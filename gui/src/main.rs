//! JPK reader GUI.

use iced::{Task, advanced::graphics::core::window};
use jpk_reader as jpk;
use polars::prelude as pl;
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};

mod data_server;
mod dataset;
mod icon;
mod workspace;

fn main() -> iced::Result {
    #[cfg(feature = "tracing")]
    tracing::enable();

    iced::daemon(App::new, App::update, App::view)
        .subscription(App::subscription)
        .theme(App::theme)
        .run()
}

#[derive(Debug, derive_more::From)]
enum Message {
    /// A workspace message.
    #[from]
    Workspace(workspace::Message),
    /// A dataset message.
    Dataset {
        id: PathBuf,
        message: dataset::Message,
    },
    DataServer(data_server::Message),
    DataServerUpdate(data_server::Update),
    /// A new window opened.
    WindowOpened {
        window: iced::window::Id,
        kind: WindowKind,
    },
    /// A window closed.
    WindowClosed(iced::window::Id),
    /// Try to open a dataset file at the given path.
    OpenDatasetFilePath(PathBuf),
    /// Try to open a dataset directory at the given path.
    OpenDatasetDirPath(PathBuf),
    /// A dataset is beign loaded.
    DatasetLoading {
        path: PathBuf,
    },
    /// A dataset loaded successfully.
    DatasetLoaded {
        path: PathBuf,
        reader: dataset::Reader,
        dataframe: pl::DataFrame,
    },
    /// An error occurred while loading a dataframe.
    DatasetError {
        path: PathBuf,
        error: String,
    },
    DatasetLastWindowClosed(PathBuf),
    AppClosed,
}

#[derive(Debug)]
enum WindowKind {
    Workspace,
    Dataset(PathBuf),
    DatasetChild {
        dataset: PathBuf,
        kind: dataset::ChildWindowType,
    },
}

#[derive(Debug)]
struct DataServer {
    update_tx: tokio::sync::mpsc::UnboundedSender<data_server::Update>,
    kill: tokio::sync::oneshot::Sender<data_server::Kill>,
}

struct App {
    theme: iced::Theme,
    workspace: workspace::Workspace,
    datasets: HashMap<PathBuf, dataset::Dataset>,
    windows: HashMap<iced::window::Id, WindowKind>,
    data_server: Option<DataServer>,
}

impl App {
    pub fn theme(&self, window: iced::window::Id) -> iced::Theme {
        self.theme.clone()
    }
}

impl App {
    fn new() -> (Self, iced::Task<Message>) {
        let app = Self {
            theme: iced::Theme::CatppuccinFrappe,
            workspace: Default::default(),
            datasets: Default::default(),
            windows: Default::default(),
            data_server: Default::default(),
        };

        let (workspace, open) = workspace::Workspace::new();
        (
            app,
            open.map(|message| match message {
                workspace::Message::WorkspaceOpened(id) => Message::WindowOpened {
                    window: id,
                    kind: WindowKind::Workspace,
                },
                _ => panic!("unexpected message received"),
            }),
        )
    }

    fn update(&mut self, message: Message) -> iced::Task<Message> {
        #[cfg(feature = "tracing")]
        ::tracing::trace!(message=?message);

        match message {
            Message::AppClosed => self.app_closed(),
            Message::Workspace(message) => self.workspace_message(message),
            Message::Dataset { id, message } => self.dataset_message(id, message),
            Message::DataServer(message) => self.data_server(message),
            Message::DataServerUpdate(update) => self.data_server_update(update),
            Message::WindowOpened { window, kind } => self.window_opened(window, kind),
            Message::WindowClosed(id) => self.window_closed(id),
            Message::OpenDatasetFilePath(path) => self.open_dataset_file_path(path),
            Message::OpenDatasetDirPath(path) => self.open_dataset_dir_path(path),
            Message::DatasetLoading { path } => self
                .workspace
                .update(workspace::Message::DatasetLoading { path })
                .map(Into::into),
            Message::DatasetLoaded {
                path,
                reader,
                dataframe,
            } => self.dataset_loaded(path, reader, dataframe),
            Message::DatasetError { path, error } => self
                .workspace
                .update(workspace::Message::DatasetError { path, error })
                .map(Into::into),
            Message::DatasetLastWindowClosed(dataset) => {
                self.datasets
                    .remove(&dataset)
                    .expect("dataset should exist");
                iced::Task::done(workspace::Message::DatasetClosed { path: dataset }.into())
            }
        }
    }

    pub fn view(&self, window: window::Id) -> iced::Element<'_, Message> {
        match self.windows.get(&window) {
            Some(WindowKind::Workspace) => self.workspace.view().map(Message::Workspace),
            Some(WindowKind::Dataset(path))
            | Some(WindowKind::DatasetChild { dataset: path, .. }) => {
                let dataset = self.datasets.get(path).expect("dataset should exist");
                dataset
                    .view(&self.theme, &window)
                    .map(move |msg| Message::Dataset {
                        id: path.clone(),
                        message: msg,
                    })
            }
            None => iced::widget::container(iced::widget::Space::new()).into(),
        }
    }

    pub fn subscription(&self) -> iced::Subscription<Message> {
        iced::Subscription::batch([
            iced::window::close_events().map(Message::WindowClosed),
            iced::Subscription::run(data_server::start).map(Message::DataServer),
        ])
    }
}

impl App {
    fn workspace_message(&mut self, message: workspace::Message) -> iced::Task<Message> {
        match message {
            workspace::Message::DatasetFilePathSelected(path) => {
                iced::Task::done(Message::OpenDatasetFilePath(path))
            }
            workspace::Message::DatasetDirPathSelected(path) => {
                iced::Task::done(Message::OpenDatasetDirPath(path))
            }

            _ => self.workspace.update(message).map(Message::Workspace),
        }
    }

    fn dataset_message(&mut self, id: PathBuf, message: dataset::Message) -> iced::Task<Message> {
        self.datasets
            .get_mut(&id)
            .map(|dataset| match message {
                dataset::Message::WindowOpened(window_id) => {
                    let _ = self
                        .windows
                        .insert(window_id.clone(), WindowKind::Dataset(id.clone()));
                    iced::Task::none()
                }
                dataset::Message::ChildWindowOpened { window, kind } => {
                    let _ = self.windows.insert(
                        window.clone(),
                        WindowKind::DatasetChild {
                            dataset: dataset.path().clone(),
                            kind,
                        },
                    );

                    iced::Task::done(
                        workspace::Message::DatasetChildWindowOpened {
                            path: id.clone(),
                            window,
                            kind,
                        }
                        .into(),
                    )
                }
                dataset::Message::Pipeline(dataset::pipeline::Message::TransformPushed(
                    ref transform,
                )) => {
                    if let dataset::pipeline::TransformKind::Script { file, .. } = transform.kind()
                    {
                        if let Some(data_server) = &self.data_server {
                            data_server
                                .update_tx
                                .send(data_server::Update::TransformAdded(
                                    data_server::TransformUri::new(id.clone(), transform.id()),
                                ))
                                .expect("could not send update");
                        }
                    }

                    dataset
                        .update(message)
                        .map(move |message| Message::Dataset {
                            id: id.clone(),
                            message,
                        })
                }
                _ => dataset
                    .update(message)
                    .map(move |message| Message::Dataset {
                        id: id.clone(),
                        message,
                    }),
            })
            .expect("dataset should exist")
    }

    fn data_server(&mut self, message: data_server::Message) -> iced::Task<Message> {
        match message {
            data_server::Message::ServerStarted(data_server) => {
                assert!(self.data_server.is_none(), "data server already exists");
                let mut data_server = data_server
                    .lock()
                    .expect("could not lock data server start message");
                let data_server = data_server.take().expect("data server shoudl exist");
                let _ = self.data_server.insert(data_server);
                iced::Task::none()
            }
            data_server::Message::DataRequest { transform, tx } => {
                let data_server::TransformUri { dataset, transform } = transform;
                iced::Task::done(Message::Dataset {
                    id: dataset,
                    message: dataset::pipeline::Message::IpcDataframeRequest { transform, tx }
                        .into(),
                })
            }
            data_server::Message::DataProduced {
                transform,
                dataframe,
            } => {
                let data_server::TransformUri { dataset, transform } = transform;
                iced::Task::done(Message::Dataset {
                    id: dataset,
                    message: dataset::pipeline::Message::IpcDataframeProdcued {
                        transform,
                        dataframe,
                    }
                    .into(),
                })
            }
        }
    }

    fn data_server_update(&mut self, update: data_server::Update) -> iced::Task<Message> {
        if let Some(data_server) = &mut self.data_server {
            data_server
                .update_tx
                .send(update)
                .expect("could not send data server update");
        }

        iced::Task::none()
    }

    fn window_opened(&mut self, id: iced::window::Id, kind: WindowKind) -> iced::Task<Message> {
        let focus = iced::window::gain_focus(id);
        let task = match &kind {
            WindowKind::Workspace => focus,
            WindowKind::Dataset(path) => focus.map({
                let path = path.clone();
                let window = id.clone();
                move |msg| {
                    Message::Workspace(workspace::Message::DatasetWindowOpened {
                        path: path.clone(),
                        window,
                    })
                }
            }),
            WindowKind::DatasetChild { dataset, kind } => focus.map({
                let path = dataset.clone();
                let window = id.clone();
                let kind = kind.clone();
                move |msg| {
                    Message::Workspace(workspace::Message::DatasetChildWindowOpened {
                        path: path.clone(),
                        window,
                        kind: kind,
                    })
                }
            }),
        };
        self.windows.insert(id, kind);
        task
    }

    fn window_closed(&mut self, id: iced::window::Id) -> iced::Task<Message> {
        if self.windows.len() == 1 {
            return iced::Task::done(Message::AppClosed);
        }

        let window = self.windows.remove(&id).expect("window should exist");
        match window {
            WindowKind::Workspace => {
                iced::Task::done(workspace::Message::WorkspaceClosed(id).into())
            }
            WindowKind::Dataset(path) => {
                let last_dataset_window = !self.windows.values().any(|window| {
                    if let WindowKind::DatasetChild { dataset, .. } = window {
                        *dataset == path
                    } else {
                        false
                    }
                });
                if last_dataset_window {
                    iced::Task::done(Message::DatasetLastWindowClosed(path))
                } else {
                    iced::Task::none()
                }
            }
            WindowKind::DatasetChild { dataset, kind } => {
                let mut task_window = iced::Task::done(
                    workspace::Message::DatasetChildWindowClosed {
                        path: dataset.clone(),
                        kind,
                    }
                    .into(),
                );

                let last_dataset_window = !self.windows.values().any(|window| match window {
                    WindowKind::Workspace => false,
                    WindowKind::Dataset(other) => *other == dataset,
                    WindowKind::DatasetChild { dataset: other, .. } => *other == dataset,
                });

                if last_dataset_window {
                    task_window = task_window.chain(iced::Task::done(
                        Message::DatasetLastWindowClosed(dataset.clone()),
                    ));
                }

                let dataset_msg = match kind {
                    dataset::ChildWindowType::DataTable => dataset::Message::DataTableClosed,
                    dataset::ChildWindowType::Pipeline => dataset::Message::PipelineClosed,
                    dataset::ChildWindowType::FileBrowser => dataset::Message::FilesBrowserClosed,
                };

                iced::Task::batch([
                    iced::Task::done(Message::Dataset {
                        id: dataset.clone(),
                        message: dataset_msg,
                    }),
                    task_window,
                ])
            }
        }
    }

    fn app_closed(&mut self) -> iced::Task<Message> {
        // TODO: Kill data server
        // self.data_server
        //     .kill
        //     .send(data_server::Kill)
        //     .expect("kill message sent");

        iced::exit()
    }

    fn open_dataset_file_path(&mut self, path: impl AsRef<Path>) -> iced::Task<Message> {
        let path = path.as_ref();
        if let Some(dataset) = self.datasets.get(path) {
            todo!("focus dataset");
        }

        iced::Task::batch([
            Task::done(Message::DatasetLoading {
                path: path.to_path_buf(),
            }),
            iced::Task::future({
                let path = path.to_path_buf();
                async move {
                    let result = tokio::task::spawn_blocking({
                        let path = path.clone();
                        move || match Self::open_dataset_file(&path) {
                            Ok((reader, dataframe)) => Message::DatasetLoaded {
                                path: path.clone(),
                                reader,
                                dataframe,
                            },
                            Err(err) => workspace::Message::DatasetError {
                                path: path.clone(),
                                error: format!("{err:?}"),
                            }
                            .into(),
                        }
                    })
                    .await;
                    match result {
                        Ok(msg) => msg.into(),
                        Err(err) => workspace::Message::DatasetError {
                            path: path.clone(),
                            error: format!("Could not load dataset: {err:?}"),
                        }
                        .into(),
                    }
                }
            }),
        ])
    }

    fn open_dataset_dir_path(&mut self, path: impl AsRef<Path>) -> iced::Task<Message> {
        let path = path.as_ref();
        if let Some(dataset) = self.datasets.get(path) {
            todo!("focus dataset");
        }

        iced::Task::batch([
            Task::done(Message::DatasetLoading {
                path: path.to_path_buf(),
            }),
            iced::Task::future({
                let path = path.to_path_buf();
                async move {
                    let result = tokio::task::spawn_blocking({
                        let path = path.clone();
                        move || match Self::open_dataset_dir(&path) {
                            Ok((reader, dataframe)) => Message::DatasetLoaded {
                                path: path.clone(),
                                reader,
                                dataframe,
                            },
                            Err(err) => workspace::Message::DatasetError {
                                path: path.clone(),
                                error: format!("{err:?}"),
                            }
                            .into(),
                        }
                    })
                    .await;
                    match result {
                        Ok(msg) => msg.into(),
                        Err(err) => workspace::Message::DatasetError {
                            path: path.clone(),
                            error: format!("Could not load dataset: {err:?}"),
                        }
                        .into(),
                    }
                }
            }),
        ])
    }

    fn dataset_loaded(
        &mut self,
        path: impl Into<PathBuf>,
        reader: dataset::Reader,
        dataframe: pl::DataFrame,
    ) -> iced::Task<Message> {
        let path = path.into();

        let (dataset, open) = dataset::Dataset::new(path.clone(), reader, dataframe);
        let loaded = Task::done(
            workspace::Message::DatasetLoaded {
                path: path.clone(),
                kind: dataset.kind(),
            }
            .into(),
        );

        self.datasets.insert(path.clone(), dataset);
        let open = open.map(move |message| {
            let dataset::Message::WindowOpened(id) = message else {
                panic!("unexpected message");
            };

            Message::WindowOpened {
                window: id,
                kind: WindowKind::Dataset(path.clone()),
            }
        });

        Task::batch([open, loaded])
    }
}

impl App {
    fn open_dataset_file(
        path: impl AsRef<Path>,
    ) -> Result<(dataset::Reader, pl::DataFrame), error::OpenDataset> {
        let Some(dataset_type) = jpk::dataset::DatasetType::from_fs(&path)? else {
            return Err(error::OpenDataset::UnknownDatasetType);
        };

        match dataset_type {
            jpk::dataset::DatasetType::VoltageSpectroscopy => {
                let mut reader = jpk::voltage_spectroscopy::v2_0::FileReader::new(path.as_ref())?;
                let df = reader.load_data_all()?;
                Ok((reader.into(), df))
            }
            jpk::dataset::DatasetType::QIMap => todo!(),
            jpk::dataset::DatasetType::VoltageSpectroscopyCollection => {
                panic!("file should not be identified as a dataset collection type")
            }
        }
    }

    fn open_dataset_dir(
        path: impl AsRef<Path>,
    ) -> Result<(dataset::Reader, pl::DataFrame), error::OpenDataset> {
        let Some(dataset_type) = jpk::dataset::DatasetType::from_fs(&path)? else {
            return Err(error::OpenDataset::UnknownDatasetType);
        };

        match dataset_type {
            jpk_reader::dataset::DatasetType::VoltageSpectroscopyCollection => {
                let mut reader = jpk::voltage_spectroscopy::v2_0::DirReader::new(path.as_ref());
                let df = reader.load_data_all()?;
                Ok((reader.into(), df))
            }
            jpk_reader::dataset::DatasetType::VoltageSpectroscopy
            | jpk_reader::dataset::DatasetType::QIMap => {
                panic!("directory should not be identified as single dataset type")
            }
        }
    }
}

mod error {
    use jpk_reader as jpk;
    use std::{io, path::PathBuf};

    #[derive(Debug, derive_more::From)]
    pub enum OpenDataset {
        Io(io::Error),
        /// Could not determine the type of dataset from the path.
        UnknownDatasetType,
        /// Could not load the dataset as the determined type.
        Dataset(jpk::dataset::error::Error<jpk::dataset::error::Dataset>),
        /// Could not load data.
        DataFile {
            path: PathBuf,
            // TODO: Make generic for all data readers. Probably need a `Reader` trait in `jpk_reader` with associated errors.
            error: jpk::voltage_spectroscopy::v2_0::error::DataFile,
        },
        DataCollection {
            path: PathBuf,
            // TODO: Make generic for all data readers. Probably need a `CollectionReader` trait in `jpk_reader` with associated errors.
            error: jpk::voltage_spectroscopy::v2_0::error::DataCollection,
        },
    }

    impl From<jpk::dataset::error::Error<jpk::voltage_spectroscopy::v2_0::error::DataFile>>
        for OpenDataset
    {
        fn from(
            value: jpk::dataset::error::Error<jpk::voltage_spectroscopy::v2_0::error::DataFile>,
        ) -> Self {
            Self::DataFile {
                path: value.paths[0].clone(),
                error: value.error,
            }
        }
    }

    impl From<jpk::dataset::error::Error<jpk::voltage_spectroscopy::v2_0::error::DataCollection>>
        for OpenDataset
    {
        fn from(
            value: jpk::dataset::error::Error<
                jpk::voltage_spectroscopy::v2_0::error::DataCollection,
            >,
        ) -> Self {
            Self::DataCollection {
                path: value.paths[0].clone(),
                error: value.error,
            }
        }
    }
}

#[cfg(feature = "tracing")]
mod tracing {
    use tracing_subscriber::{EnvFilter, fmt, prelude::*};

    pub fn enable() {
        tracing_subscriber::registry()
            .with(fmt::layer())
            .with(EnvFilter::from_default_env())
            .init();
    }
}
