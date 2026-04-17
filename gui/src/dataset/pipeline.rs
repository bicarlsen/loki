//! Data pipeline.

use iced::widget;
use polars::prelude as pl;
use polars_io::SerWriter;
use std::{
    borrow::Cow,
    collections::HashMap,
    path::{Path, PathBuf},
};

#[derive(Clone, Debug)]
pub enum Message {
    TransformPushed(Transform),
    PromptTransformScript,
    PushTransformScript(PathBuf),
    TransformOutputUpdated(TransformId),
    OutputUpdated,
    IpcDataframeRequest {
        transform: TransformId,
        tx: crate::data_server::DataRequestTx,
    },
    IpcDataframeProdcued {
        transform: TransformId,
        dataframe: pl::DataFrame,
    },
}

pub enum Action {
    None,
}

pub type TransformId = u8;

#[derive(Clone, Debug)]
pub struct Transform {
    id: TransformId,
    kind: TransformKind,
}

impl Transform {
    pub fn id(&self) -> TransformId {
        self.id
    }

    pub fn kind(&self) -> &TransformKind {
        &self.kind
    }
}

impl<'a> iced::advanced::text::IntoFragment<'a> for &'a Transform {
    fn into_fragment(self) -> widget::text::Fragment<'a> {
        self.kind.into_fragment()
    }
}

#[derive(Clone, Debug)]
pub enum TransformKind {
    Script {
        file: PathBuf,
        runner: TransformScriptRunner,
    },
}

impl<'a> iced::advanced::text::IntoFragment<'a> for &'a TransformKind {
    fn into_fragment(self) -> widget::text::Fragment<'a> {
        match self {
            TransformKind::Script { file, runner: _ } => Cow::Owned(format!(
                "Script ({})",
                file.file_name()
                    .expect("file name should exist")
                    .to_string_lossy()
            )),
        }
    }
}

#[derive(Clone, Debug)]
pub struct TransformScriptRunner {
    cmd: String,
}

#[derive(Clone)]
pub(super) struct Pipeline {
    /// Original dataframe.
    raw: pl::DataFrame,
    /// Output of `raw` after being passed through `transforms`.
    output: pl::DataFrame,
    transforms: Vec<Transform>,
    cache: HashMap<TransformId, pl::DataFrame>,
}

impl Pipeline {
    pub(super) fn new(df: pl::DataFrame) -> Self {
        Self {
            raw: df.clone(),
            output: df.clone(),
            transforms: Default::default(),
            cache: Default::default(),
        }
    }

    pub(super) fn output(&self) -> &pl::DataFrame {
        &self.output
    }

    fn push(&mut self, transform: TransformKind) -> TransformId {
        let id = if let Some(id) = self.transforms.iter().map(|transform| transform.id).max() {
            id + 1
        } else {
            0
        };

        let transform = Transform {
            id,
            kind: transform,
        };

        self.transforms.push(transform);
        id
    }

    fn get_transform(&self, id: TransformId) -> Option<&Transform> {
        self.transforms.iter().find(|transform| transform.id == id)
    }
}

impl Pipeline {
    pub(super) fn update(&mut self, message: Message) -> iced::Task<Message> {
        match message {
            Message::TransformPushed(_) => iced::Task::none(),
            Message::PromptTransformScript => self.prompt_new_transform_script(),
            Message::PushTransformScript(path) => {
                let id = self.push(TransformKind::Script {
                    file: path,
                    runner: TransformScriptRunner {
                        cmd: "python".to_string(),
                    },
                });

                let transform = self.get_transform(id).unwrap();
                iced::Task::done(Message::TransformPushed(transform.clone()))
            }
            Message::TransformOutputUpdated(transform) => self.transform_output_updated(transform),
            Message::OutputUpdated => iced::Task::none(),
            Message::IpcDataframeRequest { transform, tx } => {
                self.ipc_dataframe_request(transform, tx)
            }
            Message::IpcDataframeProdcued {
                transform,
                dataframe,
            } => self.ipc_dataframe_produced(transform, dataframe),
        }
    }

    pub(super) fn view(&self, dataset: impl AsRef<Path>) -> iced::Element<'_, Message> {
        let btn_ctrl_add_script = widget::button("Script").on_press(Message::PromptTransformScript);
        let controls_r1 = widget::row![btn_ctrl_add_script];
        let controls = widget::column![controls_r1];

        let stages = std::iter::once(widget::text("raw").into())
            .chain(self.transforms.iter().map(|transform| {
                widget::tooltip(
                    widget::text(transform),
                    widget::text(crate::data_server::TransformUri::key_of(
                        &dataset,
                        transform.id,
                    )),
                    widget::tooltip::Position::Right,
                )
                .delay(std::time::Duration::from_millis(300))
                .into()
            }))
            .collect::<Vec<iced::Element<'_, Message>>>();
        let pipeline = widget::column(stages);

        widget::column![controls, pipeline].into()
    }
}

impl Pipeline {
    fn prompt_new_transform_script(&mut self) -> iced::Task<Message> {
        rfd::FileDialog::new()
            .set_title("Select script file")
            .add_filter("Python", &["py"])
            .pick_file()
            .map(|path| iced::Task::done(Message::PushTransformScript(path)))
            .unwrap_or(iced::Task::none())
    }

    fn transform_output_updated(&mut self, transform: TransformId) -> iced::Task<Message> {
        let transform_idx = self
            .transforms
            .iter()
            .position(|t| t.id == transform)
            .expect("invalid transorm id");
        let next_idx = transform_idx + 1;
        if next_idx == self.transforms.len() {
            self.output = self
                .cache
                .get(&transform)
                .expect("dataframe should be cached")
                .clone();

            return iced::Task::done(Message::OutputUpdated);
        }

        let next = &self.transforms[next_idx];
        todo!("recompute");
        iced::Task::done(Message::TransformOutputUpdated(next.id))
    }

    fn ipc_dataframe_request(
        &mut self,
        transform: TransformId,
        tx: crate::data_server::DataRequestTx,
    ) -> iced::Task<Message> {
        let mut tx = tx.lock().expect("could not get ipc response channel");
        let tx = tx.take().expect("ipc response channel already taken");

        // TODO: Get dataframe from transofrm instead of current output.
        // let transform = self.get_transform(transform).unwrap();

        let mut ipc_file = match tempfile::NamedTempFile::new() {
            Ok(file) => file,
            Err(err) => {
                #[cfg(feature = "tracing")]
                tracing::error!("could not create ipc file: {err:?}");

                tx.send(Err(err.into()))
                    .expect("could not send ipc response");
                return iced::Task::none();
            }
        };
        let mut writer = pl::IpcWriter::new(ipc_file.as_file_mut());
        if let Err(err) = writer.finish(&mut self.output) {
            #[cfg(feature = "tracing")]
            tracing::error!("could not write dataframe to ipc file: {err:?}");

            tx.send(Err(err.into()))
                .expect("could not send ipc response");
            return iced::Task::none();
        }

        tx.send(Ok(ipc_file)).expect("could not send ipc response");
        iced::Task::none()
    }

    fn ipc_dataframe_produced(
        &mut self,
        transform: TransformId,
        dataframe: pl::DataFrame,
    ) -> iced::Task<Message> {
        self.cache.insert(transform, dataframe);
        iced::Task::done(Message::TransformOutputUpdated(transform))
    }
}
