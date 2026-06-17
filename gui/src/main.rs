//! JPK reader GUI.
use iced::advanced::graphics::core::window;
use jpk_reader as jpk;
use polars::prelude as pl;
use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
    time,
};

mod data_server;
mod dataset;
mod icon;
mod reader;
mod settings;
mod workspace;

const TOOLTIP_DELAY: time::Duration = time::Duration::from_millis(300);

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
    #[from]
    AppSettings(settings::Message),
    /// A workspace message.
    #[from]
    Workspace(workspace::Message),
    /// A dataset message.
    Dataset {
        id: PathBuf,
        message: dataset::Message,
    },
    #[from]
    DataServer(data_server::Message),
    #[from]
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
    /// A dataset loaded successfully.
    DatasetLoaded {
        path: PathBuf,
        reader: reader::Reader,
        df: pl::DataFrame,
    },
    /// An error occurred while loading a dataframe.
    DatasetLoadError {
        path: PathBuf,
        error: String,
    },
    DatasetLastWindowClosed(PathBuf),
    /// Clear all datasets.
    ClearDatasets,
    AppClosed,
}

#[derive(Clone, Debug)]
enum WindowKind {
    Workspace,
    AppSettings,
    Dataset(PathBuf),
    DatasetChild {
        dataset: PathBuf,
        kind: dataset::ChildWindowType,
    },
}

struct App {
    settings: settings::AppSettings,
    workspace: workspace::Workspace,
    datasets: HashMap<PathBuf, dataset::Dataset>,
    windows: HashMap<iced::window::Id, WindowKind>,
    data_server: Option<data_server::DataServer>,
}

impl App {
    // TODO: Allow per window theming for datasets.
    pub fn theme(&self, _window: iced::window::Id) -> iced::Theme {
        self.settings.theme.clone()
    }
}

impl App {
    fn new() -> (Self, iced::Task<Message>) {
        let (workspace, workspace_open) = workspace::Workspace::new();
        let app = Self {
            settings: Default::default(),
            workspace,
            datasets: Default::default(),
            windows: Default::default(),
            data_server: Default::default(),
        };

        (
            app,
            workspace_open.map(|window| Message::WindowOpened {
                window,
                kind: WindowKind::Workspace,
            }),
        )
    }

    pub fn view(&self, window: window::Id) -> iced::Element<'_, Message> {
        let Some(kind) = self.windows.get(&window) else {
            #[cfg(feature = "tracing")]
            ::tracing::debug!("window {window} does not exist");

            return iced::widget::space().into();
        };
        match kind {
            WindowKind::AppSettings => self.settings.view().map(Message::AppSettings),
            WindowKind::Workspace => self.workspace.view().map(Message::Workspace),
            WindowKind::Dataset(path) | WindowKind::DatasetChild { dataset: path, .. } => {
                let dataset = self.datasets.get(path).expect("dataset should exist");
                dataset
                    .view(&self.settings.theme, &window)
                    .map(move |msg| Message::Dataset {
                        id: path.clone(),
                        message: msg,
                    })
            }
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
    #[must_use]
    fn update(&mut self, message: Message) -> iced::Task<Message> {
        #[cfg(feature = "tracing")]
        ::tracing::trace!(message=?message);

        match message {
            Message::AppClosed => self.exit(),
            Message::AppSettings(message) => self.settings_update(message),
            Message::Workspace(message) => self.workspace_message(message),
            Message::Dataset { id, message } => self.dataset_message(id, message),
            Message::DataServer(message) => self.data_server(message),
            Message::DataServerUpdate(update) => self.data_server_update(update),
            Message::WindowOpened { window, kind } => self.window_opened(window, kind),
            Message::WindowClosed(id) => self.window_closed(id),
            Message::OpenDatasetFilePath(path) => self.try_load_dataset_file(path),
            Message::OpenDatasetDirPath(path) => self.try_load_dataset_dir(path),
            Message::DatasetLoaded { path, reader, df } => self.open_dataset(path, reader, df),
            Message::DatasetLoadError { path, error } => {
                let action = self
                    .workspace
                    .update(workspace::Message::DatasetError { path, error });
                assert!(matches!(action, workspace::Action::None));

                iced::Task::none()
            }
            Message::DatasetLastWindowClosed(dataset) => {
                self.datasets
                    .remove(&dataset)
                    .expect("dataset should exist");
                iced::Task::done(workspace::Message::DatasetClosed { path: dataset }.into())
            }
            Message::ClearDatasets => self.clear_datasets(),
        }
    }

    fn settings_update(&mut self, message: settings::Message) -> iced::Task<Message> {
        match self.settings.update(message) {
            settings::Action::None => iced::Task::none(),
            settings::Action::Run(task) => task.map(Into::into),
        }
    }

    fn dataset_message(&mut self, id: PathBuf, message: dataset::Message) -> iced::Task<Message> {
        let dataset = self.datasets.get_mut(&id).expect("dataset should exist");
        match dataset.update(message) {
            dataset::Action::None => iced::Task::none(),
            dataset::Action::Run(task) => task.map(move |message| Message::Dataset {
                id: id.clone(),
                message,
            }),
            dataset::Action::ChildWindowOpened { window, kind } => {
                let path = dataset.path().clone();
                self.windows.insert(
                    window,
                    WindowKind::DatasetChild {
                        dataset: path.clone(),
                        kind,
                    },
                );
                let action = self
                    .workspace
                    .update(workspace::Message::DatasetChildWindowOpened {
                        path,
                        window: window,
                        kind: kind.clone(),
                    });
                assert!(matches!(action, workspace::Action::None));

                iced::Task::none()
            }
            dataset::Action::RegisterTransformScript { dataset, transform } => {
                if let Some(server) = &self.data_server {
                    server
                        .update(data_server::Update::TransformAdded(
                            data_server::TransformUri { dataset, transform },
                        ))
                        .expect("could not send update to data server");
                }

                iced::Task::none()
            }
        }
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
                let dataset = self
                    .datasets
                    .get_mut(&dataset)
                    .expect("dataset should exist");
                let action = dataset.update(
                    dataset::pipeline::Message::IpcDataframeRequest { transform, tx }.into(),
                );

                match action {
                    dataset::Action::None => iced::Task::none(),
                    dataset::Action::Run(task) => task.map({
                        let id = dataset.path().clone();
                        move |message| Message::Dataset {
                            id: id.clone(),
                            message,
                        }
                    }),
                    dataset::Action::ChildWindowOpened { .. }
                    | dataset::Action::RegisterTransformScript { .. } => {
                        panic!("unexpected action")
                    }
                }
            }
            data_server::Message::DataProduced {
                transform,
                dataframe,
            } => {
                let data_server::TransformUri { dataset, transform } = transform;
                let dataset = self
                    .datasets
                    .get_mut(&dataset)
                    .expect("dataset should exist");
                let action = dataset.update(
                    dataset::pipeline::Message::IpcDataframeProdcued {
                        transform,
                        dataframe,
                    }
                    .into(),
                );

                match action {
                    dataset::Action::None => iced::Task::none(),
                    dataset::Action::Run(task) => task.map({
                        let id = dataset.path().clone();
                        move |message| Message::Dataset {
                            id: id.clone(),
                            message,
                        }
                    }),
                    dataset::Action::ChildWindowOpened { .. }
                    | dataset::Action::RegisterTransformScript { .. } => {
                        panic!("unexpected action")
                    }
                }
            }
        }
    }

    fn data_server_update(&mut self, update: data_server::Update) -> iced::Task<Message> {
        if let Some(data_server) = &mut self.data_server {
            data_server
                .update(update)
                .expect("could not send data server update");
        }

        iced::Task::none()
    }

    /// # Notes
    /// Focuses the window.
    fn window_opened(&mut self, id: iced::window::Id, kind: WindowKind) -> iced::Task<Message> {
        self.windows.insert(id, kind.clone());
        let focus = iced::window::gain_focus(id.clone());
        let task = match &kind {
            WindowKind::Workspace => focus,
            WindowKind::AppSettings => focus,
            WindowKind::Dataset(path) => {
                let _ = self
                    .windows
                    .insert(id.clone(), WindowKind::Dataset(path.clone()));

                let action = self
                    .workspace
                    .update(workspace::Message::DatasetWindowOpened {
                        path: path.clone(),
                        window: id.clone(),
                    });
                assert!(matches!(action, workspace::Action::None));

                iced::Task::none()
            }
            WindowKind::DatasetChild { dataset, kind } => {
                let action = self
                    .workspace
                    .update(workspace::Message::DatasetChildWindowOpened {
                        path: dataset.clone(),
                        window: id.clone(),
                        kind: kind.clone(),
                    });
                assert!(matches!(action, workspace::Action::None));

                iced::Task::none()
            }
        };
        task
    }

    fn window_closed(&mut self, id: iced::window::Id) -> iced::Task<Message> {
        if self.windows.len() == 1 {
            return self.exit();
        }

        let window = self.windows.remove(&id).expect("window should exist");
        match window {
            WindowKind::Workspace => iced::Task::none(),
            WindowKind::AppSettings => iced::Task::none(),
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
                    WindowKind::AppSettings => false,
                    WindowKind::Dataset(other) => *other == dataset,
                    WindowKind::DatasetChild { dataset: other, .. } => *other == dataset,
                });

                if last_dataset_window {
                    task_window = task_window.chain(iced::Task::done(
                        Message::DatasetLastWindowClosed(dataset.clone()),
                    ));
                }

                let dataset_msg = match kind {
                    dataset::ChildWindowType::Settings => dataset::Message::SettingsClosed,
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

    fn exit(&mut self) -> iced::Task<Message> {
        // TODO: Kill data server
        // self.data_server
        //     .kill
        //     .send(data_server::Kill)
        //     .expect("kill message sent");

        iced::exit()
    }

    fn clear_datasets(&mut self) -> iced::Task<Message> {
        let mut tasks = Vec::with_capacity(self.windows.len());
        for (id, kind) in self.windows.iter() {
            if matches!(kind, WindowKind::Workspace) {
                continue;
            }

            tasks.push(iced::window::close(id.clone()))
        }

        iced::Task::batch(tasks)
    }
}

impl App {
    fn workspace_message(&mut self, message: workspace::Message) -> iced::Task<Message> {
        let action = self.workspace.update(message);
        #[cfg(feature = "tracing")]
        ::tracing::trace!(?action);

        match action {
            workspace::Action::None => iced::Task::none(),
            workspace::Action::Run(task) => task.map(Message::Workspace),
            workspace::Action::AppSettingsWindowOpened(id) => {
                self.windows.insert(id, WindowKind::AppSettings);
                iced::Task::none()
            }
            workspace::Action::LoadDatasetFile(path) => self.try_load_dataset_file(path),
            workspace::Action::LoadDatasetDir(path) => self.try_load_dataset_dir(path),
            #[cfg(feature = "project")]
            workspace::Action::SaveProject(path) => self.save_project(path),
            #[cfg(feature = "project")]
            workspace::Action::OpenProject(path) => self.open_project(path),
        }
    }

    #[inline]
    fn try_load_dataset_file(&mut self, path: impl Into<PathBuf>) -> iced::Task<Message> {
        self.try_load_dataset(path, Self::load_dataset_file)
    }

    #[inline]
    fn try_load_dataset_dir(&mut self, path: impl Into<PathBuf>) -> iced::Task<Message> {
        self.try_load_dataset(path, Self::load_dataset_dir)
    }

    fn try_load_dataset<L>(&mut self, path: impl Into<PathBuf>, loader: L) -> iced::Task<Message>
    where
        L: FnOnce(PathBuf) -> Result<(reader::Reader, pl::DataFrame), error::OpenDataset>
            + Send
            + 'static,
    {
        let path = path.into();
        self.workspace.dataset_set_loading(path.clone());

        iced::Task::perform(
            tokio::task::spawn_blocking({
                let path = path.clone();
                move || loader(path)
            }),
            move |result| match result {
                Ok(dataset) => match dataset {
                    Ok((reader, df)) => Message::DatasetLoaded {
                        path: path.clone(),
                        reader,
                        df,
                    },
                    Err(err) => {
                        #[cfg(feature = "tracing")]
                        ::tracing::error!("tokio task failed while loading dataset: {err:?}");

                        Message::DatasetLoadError {
                            path: path.clone(),
                            error: format!("{err:?}"),
                        }
                    }
                },
                Err(err) => {
                    #[cfg(feature = "tracing")]
                    ::tracing::error!("tokio task failed while loading dataset: {err}");

                    Message::DatasetLoadError {
                        path: path.clone(),
                        error: err.to_string(),
                    }
                }
            },
        )
    }

    fn load_dataset_file(
        path: impl AsRef<Path>,
    ) -> Result<(reader::Reader, pl::DataFrame), error::OpenDataset> {
        match jpk::dataset::DatasetType::from_fs(&path) {
            Ok(Some(dataset_type)) => match dataset_type {
                jpk::dataset::DatasetType::VoltageSpectroscopy => {
                    let mut reader =
                        jpk::voltage_spectroscopy::v2_0::FileReader::new(path.as_ref())?;
                    let df = reader.load_data()?;
                    return Ok((reader::Reader::JpkVoltageSpectroscopy(reader.into()), df));
                }
                jpk::dataset::DatasetType::QIMap => todo!(),
                jpk::dataset::DatasetType::VoltageSpectroscopyCollection => {
                    panic!("file should not be identified as a dataset collection type")
                }
            },
            Ok(None) => {
                #[cfg(feature = "tracing")]
                ::tracing::debug!("file not readable as jpk");
            }
            Err(err) => {
                #[cfg(feature = "tracing")]
                ::tracing::debug!(?err);
            }
        }

        return Err(error::OpenDataset::UnknownDatasetType);
    }

    fn load_dataset_dir(
        path: impl AsRef<Path>,
    ) -> Result<(reader::Reader, pl::DataFrame), error::OpenDataset> {
        match jpk::dataset::DatasetType::from_fs(&path) {
            Ok(Some(dataset_type)) => match dataset_type {
                jpk_reader::dataset::DatasetType::VoltageSpectroscopyCollection => {
                    let dir_walker = fs::read_dir(&path).unwrap();
                    let paths = dir_walker
                        .into_iter()
                        .filter_map(|entry| entry.ok())
                        .filter_map(|entry| {
                            let path = entry.path();
                            let ext = path.extension()?.to_str()?;
                            (path.is_file() && ext == jpk_reader::voltage_spectroscopy::VOLTAGE_SPECTROSCOPY_FILE_EXT).then_some(path)
                        })
                        .collect::<Vec<_>>();

                    let mut reader = jpk::voltage_spectroscopy::v2_0::DirReader::new(paths)?;
                    let df = reader.load_data()?;
                    return Ok((
                        reader::Reader::JpkVoltageSpectroscopyCollection(reader.into()),
                        df,
                    ));
                }
                jpk_reader::dataset::DatasetType::VoltageSpectroscopy
                | jpk_reader::dataset::DatasetType::QIMap => {
                    panic!("directory should not be identified as single dataset type")
                }
            },
            Ok(None) => {
                #[cfg(feature = "tracing")]
                ::tracing::debug!("directory not readable as jpk");
            }
            Err(err) => {
                #[cfg(feature = "tracing")]
                ::tracing::debug!(?err);
            }
        }

        return Err(error::OpenDataset::UnknownDatasetType);
    }

    fn open_dataset(
        &mut self,
        path: impl Into<PathBuf>,
        reader: reader::Reader,
        df: pl::DataFrame,
    ) -> iced::Task<Message> {
        let path = path.into();
        let (dataset, open) = dataset::Dataset::new(path.clone(), reader, df);
        self.workspace.dataset_loaded(path.clone());
        self.datasets.insert(path.clone(), dataset);

        open.then(move |window| {
            iced::Task::done(
                Message::WindowOpened {
                    window,
                    kind: WindowKind::Dataset(path.clone()),
                }
                .into(),
            )
        })
    }

    // fn open_dataset_file_path(&mut self, path: impl AsRef<Path>) -> iced::Task<Message> {
    //     let path = path.as_ref();
    //     if let Some(dataset) = self.datasets.get(path) {
    //         return iced::window::gain_focus(dataset.window_id().clone());
    //     }

    //     self.workspace.dataset_set_loading(path.to_path_buf());
    //     iced::Task::future({
    //         let path = path.to_path_buf();
    //         async move {
    //             let result = tokio::task::spawn_blocking({
    //                 let path = path.clone();
    //                 move || match Self::open_dataset_file(&path) {
    //                     Ok((reader, dataframe)) => Message::DatasetLoaded {
    //                         path: path.clone(),
    //                         reader,
    //                         dataframe,
    //                     },
    //                     Err(err) => workspace::Message::DatasetError {
    //                         path: path.clone(),
    //                         error: format!("{err:?}"),
    //                     }
    //                     .into(),
    //                 }
    //             })
    //             .await;
    //             match result {
    //                 Ok(msg) => msg.into(),
    //                 Err(err) => workspace::Message::DatasetError {
    //                     path: path.clone(),
    //                     error: format!("Could not load dataset: {err:?}"),
    //                 }
    //                 .into(),
    //             }
    //         }
    //     })
    // }

    // fn open_dataset_dir_path(&mut self, path: impl AsRef<Path>) -> iced::Task<Message> {
    //     let path = path.as_ref();
    //     if let Some(dataset) = self.datasets.get(path) {
    //         todo!("focus dataset");
    //     }

    //     self.workspace.dataset_set_loading(path.to_path_buf());
    //     iced::Task::future({
    //         let path = path.to_path_buf();
    //         async move {
    //             let result = tokio::task::spawn_blocking({
    //                 let path = path.clone();
    //                 move || match Self::open_dataset_dir(&path) {
    //                     Ok((reader, df)) => Message::DatasetLoaded {
    //                         path: path.clone(),
    //                         reader,
    //                         df,
    //                     },
    //                     Err(err) => workspace::Message::DatasetError {
    //                         path: path.clone(),
    //                         error: format!("{err:?}"),
    //                     }
    //                     .into(),
    //                 }
    //             })
    //             .await;
    //             match result {
    //                 Ok(msg) => msg.into(),
    //                 Err(err) => workspace::Message::DatasetError {
    //                     path: path.clone(),
    //                     error: format!("Could not load dataset: {err:?}"),
    //                 }
    //                 .into(),
    //             }
    //         }
    //     })
    // }
}

#[cfg(feature = "project")]
impl App {
    fn save_project(&self, path: PathBuf) -> iced::Task<Message> {
        let state = project::State::new(self);
        match state.save(&path) {
            Ok(_) => iced::Task::none(),
            Err(err) => {
                #[cfg(feature = "tracing")]
                ::tracing::error!(?err);

                todo!("could not save project: {err:?}")
            }
        }
    }

    fn open_project(&mut self, path: PathBuf) -> iced::Task<Message> {
        match project::State::from_file(&path) {
            Ok(project) => self.clear_datasets().chain(self.load_project(&project)),
            Err(err) => {
                #[cfg(feature = "tracing")]
                ::tracing::error!(?err);

                todo!("could not load project: {err:?}");
            }
        }
    }

    fn load_project(&mut self, project: &project::State) -> iced::Task<Message> {
        let tasks = project.datasets.iter().map(|dataset| {
            if dataset.path.is_file() {
                self.try_load_dataset_file(&dataset.path)
            } else if dataset.path.is_dir() {
                self.try_load_dataset_dir(&dataset.path)
            } else if !dataset.path.exists() {
                iced::Task::done(
                    workspace::Message::DatasetError {
                        path: dataset.path.clone(),
                        error: "does not exist".to_string(),
                    }
                    .into(),
                )
            } else {
                iced::Task::done(
                    workspace::Message::DatasetError {
                        path: dataset.path.clone(),
                        error: "unknown".to_string(),
                    }
                    .into(),
                )
            }
        });

        iced::Task::batch(tasks)
    }
}

#[cfg(feature = "project")]
impl Into<project::State> for &App {
    fn into(self) -> project::State {
        let datasets = self
            .datasets
            .iter()
            .map(|(path, dataset)| {
                let workspace_dataset = self.workspace.get(path).expect("dataset should exist");
                let children = dataset.children();
                let windows = project::Windows {
                    main: self.windows.get(dataset.window_id()).is_some(),
                    data_table: children.data_table.is_some(),
                    pipeline: children.pipeline.is_some(),
                    file_browser: children.files_browser.is_some(),
                };

                project::Dataset {
                    path: path.clone(),
                    label: workspace_dataset.label().cloned(),
                    windows,
                }
            })
            .collect();

        project::State { datasets }
    }
}

#[cfg(feature = "project")]
mod project {
    use std::{
        fs,
        io::{self, Read, Write},
        path::{Path, PathBuf},
    };

    pub const FILE_EXT: &str = "loki";

    #[derive(Default, serde::Serialize, serde::Deserialize)]
    pub(crate) struct Windows {
        pub(crate) main: bool,
        pub(crate) data_table: bool,
        pub(crate) pipeline: bool,
        pub(crate) file_browser: bool,
    }

    #[derive(serde::Serialize, serde::Deserialize)]
    pub(crate) struct Dataset {
        pub(crate) path: PathBuf,
        pub(crate) label: Option<String>,
        pub(crate) windows: Windows,
    }

    #[derive(serde::Serialize, serde::Deserialize)]
    pub(crate) struct State {
        pub(crate) datasets: Vec<Dataset>,
    }

    impl State {
        pub fn new(app: &super::App) -> Self {
            app.into()
        }

        pub fn from_file(path: impl AsRef<Path>) -> Result<Self, LoadError> {
            let mut file = fs::File::open(path)?;
            let mut content = String::new();
            file.read_to_string(&mut content)?;
            toml::from_str(&content).map_err(|err| err.into())
        }

        pub fn save(&self, path: impl AsRef<Path>) -> Result<(), io::Error> {
            let mut file = fs::File::create(path)?;

            let content = toml::to_string(&self).expect("valid toml");
            file.write_all(content.as_bytes())
        }
    }

    #[derive(Debug, derive_more::From)]
    pub enum LoadError {
        Io(io::Error),
        Serde(toml::de::Error),
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
        let env_filter = EnvFilter::try_from_default_env().unwrap_or(EnvFilter::default());
        tracing_subscriber::registry()
            .with(fmt::layer())
            .with(env_filter)
            .init();
    }
}
