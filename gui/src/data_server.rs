//! Server for transform scripts.

use super::dataset;
use ::base64::Engine;
use base64::prelude as base64;
use polars::prelude as pl;
use polars_io::SerReader;
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt},
    net,
    sync::{mpsc, oneshot},
};

pub type DataRequestTx =
    Arc<Mutex<Option<oneshot::Sender<Result<tempfile::NamedTempFile, IpcError>>>>>;

const DATA_SERVER_URI: &str = "127.0.0.1:7041";
const TRANSFORM_DATAFRAME_REQUEST_METHOD: &str = "transform_dataframe_request";
const TRANSFORM_DATAFRAME_PRODUCED_METHOD: &str = "transform_dataframe_produced";

enum IpcMethod {
    DataFrameRequest { key: String },
    DataframeProduced { key: String, ipc_file: PathBuf },
}

#[derive(derive_more::From, Debug)]
pub enum IpcError {
    Io(std::io::Error),
    Polars(polars::error::PolarsError),
}

#[derive(Clone, Debug)]
pub struct TransformUri {
    pub(crate) dataset: PathBuf,
    pub(crate) transform: dataset::pipeline::TransformId,
}

impl TransformUri {
    pub fn new(
        dataset: impl Into<PathBuf>,
        transform: impl Into<dataset::pipeline::TransformId>,
    ) -> Self {
        Self {
            dataset: dataset.into(),
            transform: transform.into(),
        }
    }

    pub fn key(&self) -> String {
        Self::key_of(&self.dataset, self.transform)
    }

    pub fn key_of(dataset: impl AsRef<Path>, transform: dataset::pipeline::TransformId) -> String {
        let key = format!("{}.{}", dataset.as_ref().to_string_lossy(), transform);
        let key = murmur::murmurhash3_32(key.as_bytes(), 0);
        let mut bytes = [0; 4];
        bytes.copy_from_slice(&key.to_le_bytes());
        base64::BASE64_STANDARD_NO_PAD.encode(bytes)
    }
}

/// Kill signal.
#[derive(Debug)]
pub struct Kill;

#[derive(Clone, derive_more::Debug)]
pub enum Message {
    #[debug("ServerStarted")]
    ServerStarted(Arc<Mutex<Option<DataServer>>>),
    DataRequest {
        transform: TransformUri,
        /// Channel to send the IPC file.
        tx: DataRequestTx,
    },
    DataProduced {
        transform: TransformUri,
        dataframe: pl::DataFrame,
    },
}

#[derive(Clone, Debug)]
pub enum Update {
    TransformAdded(TransformUri),
    TransformRemoved(TransformUri),
}

#[derive(Debug)]
pub struct DataServer {
    update_tx: tokio::sync::mpsc::UnboundedSender<Update>,
    kill: tokio::sync::oneshot::Sender<Kill>,
}

impl DataServer {
    pub fn update(
        &self,
        update: Update,
    ) -> Result<(), tokio::sync::mpsc::error::SendError<Update>> {
        self.update_tx.send(update)
    }
}

pub fn start() -> impl iced::task::Straw<Kill, Message, std::io::Error> {
    let (update_tx, update_rx) = mpsc::unbounded_channel::<Update>();
    let (kill_tx, kill_rx) = oneshot::channel::<Kill>();

    iced::task::sipper(async move |message_tx| {
        let mut server = Server::new(DATA_SERVER_URI, message_tx, update_rx, kill_rx).await?;
        server
            .message_tx
            .send(Message::ServerStarted(Arc::new(Mutex::new(Some(
                DataServer {
                    update_tx,
                    kill: kill_tx,
                },
            )))))
            .await;
        server.run().await;
        Ok(Kill)
    })
}

pub struct Server {
    listener: net::TcpListener,
    message_tx: sipper::Sender<Message>,
    update_rx: mpsc::UnboundedReceiver<Update>,
    kill: oneshot::Receiver<Kill>,
    transform_map: HashMap<String, TransformUri>,
    /// Holds file handles of temporary files so they are not removed.
    file_cache: HashMap<String, tempfile::NamedTempFile>,
}

impl Server {
    pub async fn new(
        addr: impl net::ToSocketAddrs,
        message_tx: sipper::Sender<Message>,
        update_rx: mpsc::UnboundedReceiver<Update>,
        kill: oneshot::Receiver<Kill>,
    ) -> Result<Self, std::io::Error> {
        let listener = net::TcpListener::bind(addr).await?;

        Ok(Self {
            listener,
            message_tx,
            update_rx,
            kill,
            transform_map: Default::default(),
            file_cache: Default::default(),
        })
    }

    pub async fn run(&mut self) {
        loop {
            tokio::select! {
                socket = self.listener.accept() => match socket {
                    Ok((stream, _)) => self.handle_connection(stream).await,
                    Err(err) => todo!("{err:?}"),
                },
                msg =  self.update_rx.recv() => match msg {
                    Some(msg) => self.handle_update(msg),
                    None => todo!("empty update message")
                },
                _ = (&mut self.kill) =>  {
                    return
                }
            }
        }
    }

    async fn handle_connection(&mut self, mut stream: net::TcpStream) {
        let reader = tokio::io::BufReader::new(&mut stream);
        let mut lines = reader.lines();
        let Ok(line) = lines.next_line().await else {
            #[cfg(feature = "tracing")]
            tracing::debug!("connection terminated");
            return;
        };

        let Some(request) = line else {
            #[cfg(feature = "tracing")]
            tracing::debug!("empty request");

            return;
        };

        #[cfg(feature = "tracing")]
        tracing::trace!(?request);

        self.handle_request(&request, &mut stream).await;
    }

    async fn handle_request(&mut self, line: &String, stream: &mut net::TcpStream) {
        let method = match self.parse_line_as_ipc_method(&line) {
            Ok(method) => method,
            Err(err) => {
                #[cfg(feature = "tracing")]
                tracing::error!("invalid request {err:?}: {line}");

                return;
            }
        };

        match method {
            IpcMethod::DataFrameRequest { key } => {
                self.handle_ipc_dataframe_request(stream, &key).await
            }
            IpcMethod::DataframeProduced { key, ipc_file } => {
                self.handle_ipc_dataframe_output(stream, &key, &ipc_file)
                    .await
            }
        }
    }

    async fn handle_ipc_dataframe_request(&mut self, stream: &mut net::TcpStream, key: &String) {
        let Some(transform_uri) = self.transform_map.get(key) else {
            #[cfg(feature = "tracing")]
            tracing::error!("transform uri key `{key}` not found");

            return;
        };

        let (tx, rx) = oneshot::channel();
        self.message_tx
            .send(Message::DataRequest {
                transform: transform_uri.clone(),
                tx: Arc::new(Mutex::new(Some(tx))),
            })
            .await;

        let Ok(ipc_file) = rx.await else {
            #[cfg(feature = "tracing")]
            tracing::error!("request channel closed");

            return;
        };

        match ipc_file {
            Ok(ipc_file) => {
                stream
                    .write_all(ipc_file.path().to_string_lossy().as_bytes())
                    .await
                    .expect("could not respond to request");

                let previous = self.file_cache.insert(transform_uri.key(), ipc_file);
                #[cfg(feature = "tracing")]
                if let Some(_previous) = previous {
                    tracing::debug!("ipc file already exists");
                }
            }
            Err(err) => {
                #[cfg(feature = "tracing")]
                tracing::error!("could not get ipc file: {err:?}");

                todo!("{err:?}");
            }
        }
    }

    async fn handle_ipc_dataframe_output(
        &mut self,
        stream: &mut net::TcpStream,
        key: &String,
        ipc_file: impl AsRef<Path>,
    ) {
        let Some(transform_uri) = self.transform_map.get(key) else {
            #[cfg(feature = "tracing")]
            tracing::debug!("transform uri key `{key}` not found");

            return;
        };

        let ipc_file = match std::fs::File::open(ipc_file) {
            Ok(file) => file,
            Err(err) => {
                #[cfg(feature = "tracing")]
                tracing::error!("could not open ipc file: {err:}");

                return;
            }
        };

        let df = match pl::IpcReader::new(ipc_file).finish() {
            Ok(df) => df,
            Err(err) => {
                #[cfg(feature = "tracing")]
                tracing::error!("could not read dataframe: {err:}");

                return;
            }
        };

        self.message_tx
            .send(Message::DataProduced {
                transform: transform_uri.clone(),
                dataframe: df,
            })
            .await
    }

    fn parse_line_as_ipc_method(&self, line: impl AsRef<str>) -> Result<IpcMethod, InvalidRequest> {
        let parts = line
            .as_ref()
            .split_ascii_whitespace()
            .map(|part| part.trim())
            .collect::<Vec<_>>();

        if parts.len() == 0 {
            return Err(InvalidRequest::Malformed("empty method".to_string()));
        }
        match parts[0] {
            TRANSFORM_DATAFRAME_REQUEST_METHOD => {
                if parts.len() != 2 {
                    return Err(InvalidRequest::Malformed(format!(
                        "expected 2 parts, found {}",
                        parts.len()
                    )));
                }

                Ok(IpcMethod::DataFrameRequest {
                    key: parts[1].to_string(),
                })
            }
            TRANSFORM_DATAFRAME_PRODUCED_METHOD => {
                if parts.len() != 3 {
                    return Err(InvalidRequest::Malformed(format!(
                        "expected 3 parts, found {}",
                        parts.len()
                    )));
                }

                Ok(IpcMethod::DataframeProduced {
                    key: parts[1].to_string(),
                    ipc_file: PathBuf::from(parts[2]),
                })
            }
            _ => Err(InvalidRequest::InvalidMethod),
        }
    }

    fn handle_update(&mut self, update: Update) {
        #[cfg(feature = "tracing")]
        tracing::trace!(?update);

        match update {
            Update::TransformAdded(transform_uri) => {
                let previous = self
                    .transform_map
                    .insert(transform_uri.key(), transform_uri);

                #[cfg(feature = "tracing")]
                if let Some(previous) = previous {
                    tracing::debug!("transform already exists: {previous:?}")
                }
            }
            Update::TransformRemoved(transform_uri) => {
                let previous = self.transform_map.remove(&transform_uri.key());

                #[cfg(feature = "tracing")]
                if previous.is_none() {
                    tracing::debug!("removed transform was not present: {transform_uri:?}")
                }
            }
        }
    }
}

#[derive(Debug)]
pub enum InvalidRequest {
    /// Request could not be parsed.
    Malformed(String),
    /// Unknown method.
    InvalidMethod,
    /// Transform uri not found.
    /// Value is the transform key.
    TransformNotFound(String),
    /// Request transform uri could not be parsed.
    InvalidTransformUri,
}

mod murmur {
    //! [`murmur32` hash]()
    //! Vendored from [`simplehash`](https://crates.io/crates/simplehash) [[src](https://github.com/cmackenzie1/simplehash/blob/main/src/murmur.rs)].
    use std::hash::Hasher;

    const C1_32: u32 = 0xcc9e2d51;
    const C2_32: u32 = 0x1b873593;

    #[derive(Debug, Copy, Clone)]
    pub struct MurmurHasher32 {
        state: u32,
        length: usize,
    }

    impl MurmurHasher32 {
        #[inline]
        pub fn new(seed: u32) -> Self {
            Self {
                state: seed,
                length: 0,
            }
        }

        #[inline]
        pub fn finish_u32(&self) -> u32 {
            let mut h1 = self.state;

            // Finalization
            h1 ^= self.length as u32;
            h1 = h1 ^ (h1 >> 16);
            h1 = h1.wrapping_mul(0x85ebca6b);
            h1 = h1 ^ (h1 >> 13);
            h1 = h1.wrapping_mul(0xc2b2ae35);
            h1 = h1 ^ (h1 >> 16);

            h1
        }
    }

    impl Default for MurmurHasher32 {
        #[inline]
        fn default() -> Self {
            Self::new(0)
        }
    }

    impl Hasher for MurmurHasher32 {
        #[inline(always)]
        fn finish(&self) -> u64 {
            self.finish_u32() as u64
        }

        #[inline(always)]
        fn write(&mut self, data: &[u8]) {
            let len = data.len();
            self.length += len;

            // Local state for better optimization
            let mut h1 = self.state;

            // Process 4-byte blocks
            let nblocks = len / 4;
            let blocks_end = nblocks * 4;

            for i in (0..blocks_end).step_by(4) {
                // Use endian-agnostic byte loading (same as original algorithm)
                let k1 = (data[i] as u32)
                    | ((data[i + 1] as u32) << 8)
                    | ((data[i + 2] as u32) << 16)
                    | ((data[i + 3] as u32) << 24);

                let mut k = k1.wrapping_mul(C1_32);
                k = k.rotate_left(15);
                k = k.wrapping_mul(C2_32);

                h1 ^= k;
                h1 = h1.rotate_left(13);
                h1 = h1.wrapping_mul(5).wrapping_add(0xe6546b64);
            }

            // Process tail (remaining bytes)
            let mut k1: u32 = 0;
            let tail = &data[blocks_end..];

            match tail.len() {
                3 => {
                    k1 ^= (tail[2] as u32) << 16;
                    k1 ^= (tail[1] as u32) << 8;
                    k1 ^= tail[0] as u32;
                    k1 = k1.wrapping_mul(C1_32);
                    k1 = k1.rotate_left(15);
                    k1 = k1.wrapping_mul(C2_32);
                    h1 ^= k1;
                }
                2 => {
                    k1 ^= (tail[1] as u32) << 8;
                    k1 ^= tail[0] as u32;
                    k1 = k1.wrapping_mul(C1_32);
                    k1 = k1.rotate_left(15);
                    k1 = k1.wrapping_mul(C2_32);
                    h1 ^= k1;
                }
                1 => {
                    k1 ^= tail[0] as u32;
                    k1 = k1.wrapping_mul(C1_32);
                    k1 = k1.rotate_left(15);
                    k1 = k1.wrapping_mul(C2_32);
                    h1 ^= k1;
                }
                _ => {}
            }

            // Store state
            self.state = h1;
        }
    }

    /// Computes the MurmurHash3 32-bit hash of the provided data.
    ///
    /// MurmurHash3 is a non-cryptographic hash function created by Austin Appleby in 2008.
    /// This 32-bit implementation is optimized for x86 architectures and provides excellent
    /// distribution, avalanche behavior, and performance characteristics.
    ///
    /// # Algorithm
    ///
    /// MurmurHash3 (32-bit) works by:
    /// 1. Processing the input in 4-byte (32-bit) blocks
    /// 2. Applying carefully chosen magic constants and bit manipulation operations
    /// 3. Processing any remaining bytes (the "tail")
    /// 4. Finalizing the hash with additional mixing to improve avalanche behavior
    ///
    /// # Parameters
    ///
    /// * `data` - A slice of bytes to hash
    /// * `seed` - A 32-bit seed value that can be used to create different hash values for the same input
    ///
    /// # Returns
    ///
    /// A 32-bit unsigned integer representing the hash value
    ///
    /// # Example
    ///
    /// ```
    /// use simplehash::murmurhash3_32;
    ///
    /// let data = b"hello world";
    /// let hash = murmurhash3_32(data, 0);  // Using seed value 0
    /// println!("MurmurHash3-32 hash: 0x{:08x}", hash);
    ///
    /// // Using a different seed produces a different hash
    /// let hash2 = murmurhash3_32(data, 42);
    /// println!("MurmurHash3-32 hash (seed 42): 0x{:08x}", hash2);
    /// ```
    ///
    /// # Compatibility
    ///
    /// This implementation is compatible with other MurmurHash3 implementations including the
    /// original C++ implementation by Austin Appleby and the Python mmh3 package.
    #[inline]
    pub fn murmurhash3_32(data: &[u8], seed: u32) -> u32 {
        let mut hasher = MurmurHasher32::new(seed);
        hasher.write(data);
        hasher.finish_u32()
    }
}
