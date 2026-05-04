//! Workspace.
use super::dataset;
use crate::icon;
use jpk_reader as jpk;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub enum Message {
    /// Open app settings.
    OpenSettings,
    /// App settings opened.
    SettingsOpened(iced::window::Id),
    /// App settings closed.
    SettingsClosed,
    /// Open a dataset file chooser.
    PromptOpenDatasetFile,
    /// Open a dataset directory chooser.
    PromptOpenDatasetDir,
    /// The user selected a dataset file path to try to open.
    LoadDatasetFilePathSelected(PathBuf),
    /// The user selected a dataset directory path to try to open.
    LoadDatasetDirPathSelected(PathBuf),
    /// A dataset is loading.
    DatasetLoading {
        path: PathBuf,
    },
    /// A dataset window opened.
    DatasetWindowOpened {
        path: PathBuf,
        window: iced::window::Id,
    },
    DatasetChildWindowOpened {
        path: PathBuf,
        window: iced::window::Id,
        kind: dataset::ChildWindowType,
    },
    DatasetChildWindowClosed {
        path: PathBuf,
        kind: dataset::ChildWindowType,
    },
    /// An error occurred while loading the dataset.
    DatasetError {
        path: PathBuf,
        error: String,
    },
    DatasetClosed {
        path: PathBuf,
    },
    OpenOrFocusDataset(PathBuf),
    FocusWindow(iced::window::Id),
    #[cfg(feature = "project")]
    RequestSaveProject,
    #[cfg(feature = "project")]
    RequestOpenProject,
}

#[derive(Debug)]
pub enum Action {
    None,
    Run(iced::Task<Message>),
    /// App settings window opened.
    AppSettingsWindowOpened(iced::window::Id),
    /// Try to load the file as a dataset.
    LoadDatasetFile(PathBuf),
    /// Try to load the directory as a dataset.
    LoadDatasetDir(PathBuf),
    #[cfg(feature = "project")]
    SaveProject(PathBuf),
    #[cfg(feature = "project")]
    OpenProject(PathBuf),
}

pub enum DatasetState {
    Ok,
    Err(String),
    Loading,
}

#[derive(Default)]
struct DatasetChildren {
    settings: Option<iced::window::Id>,
    data_table: Option<iced::window::Id>,
    pipeline: Option<iced::window::Id>,
    file_browser: Option<iced::window::Id>,
}

impl DatasetChildren {
    pub fn take_child(&mut self, kind: dataset::ChildWindowType) -> Option<iced::window::Id> {
        match kind {
            dataset::ChildWindowType::Settings => self.settings.take(),
            dataset::ChildWindowType::DataTable => self.data_table.take(),
            dataset::ChildWindowType::Pipeline => self.pipeline.take(),
            dataset::ChildWindowType::FileBrowser => self.file_browser.take(),
        }
    }
}

pub struct Dataset {
    path: PathBuf,
    label: Option<String>,
    data: DatasetState,
    kind: Option<jpk::dataset::DatasetType>,
    window: Option<iced::window::Id>,
    children: DatasetChildren,
}

impl Dataset {
    pub fn label(&self) -> Option<&String> {
        self.label.as_ref()
    }
}

pub(crate) struct Workspace {
    _title: String,
    app_settings: Option<iced::window::Id>,
    datasets: Vec<Dataset>,
}

impl Default for Workspace {
    fn default() -> Self {
        Self {
            _title: "jpk reader".into(),
            app_settings: Default::default(),
            datasets: Default::default(),
        }
    }
}

impl Workspace {
    pub fn title(&self, window: iced::window::Id) -> String {
        self._title.clone()
    }

    pub fn get(&self, dataset: impl AsRef<Path>) -> Option<&Dataset> {
        self.datasets.iter().find(|ds| ds.path == dataset.as_ref())
    }

    fn clear(&mut self) {
        self.datasets.clear();
    }
}

impl Workspace {
    /// # Returns
    /// Window open task.
    pub fn new() -> (Self, iced::Task<iced::window::Id>) {
        let settings = iced::window::Settings {
            size: iced::Size {
                width: 300.0,
                height: 600.0,
            },
            ..Default::default()
        };

        let (_, open) = iced::window::open(settings);
        (Self::default(), open)
    }

    pub fn update(&mut self, message: Message) -> Action {
        match message {
            Message::OpenSettings => {
                let (_, open) = iced::window::open(Default::default());
                Action::Run(open.map(Message::SettingsOpened))
            }
            Message::SettingsOpened(id) => {
                let _ = self.app_settings.insert(id.clone());
                Action::AppSettingsWindowOpened(id)
            }
            Message::SettingsClosed => {
                let _ = self.app_settings.take();
                Action::None
            }
            Message::PromptOpenDatasetFile => Action::Run(self.maybe_open_dataset_file()),
            Message::PromptOpenDatasetDir => Action::Run(self.maybe_open_dataset_dir()),
            Message::LoadDatasetFilePathSelected(path) => Action::LoadDatasetFile(path),
            Message::LoadDatasetDirPathSelected(path) => Action::LoadDatasetDir(path),
            Message::DatasetLoading { path } => {
                self.dataset_set_loading(path);
                Action::None
            }
            Message::DatasetWindowOpened { path, window } => {
                self.insert_dataset_window(path, window);
                Action::None
            }
            Message::DatasetChildWindowOpened { path, window, kind } => {
                self.dataset_child_window_opened(path, window, kind);
                Action::None
            }
            Message::DatasetChildWindowClosed { path, kind } => {
                self.dataset_child_window_closed(path, kind);
                Action::None
            }
            Message::DatasetError { path, error } => {
                todo!("dataset error: {error}")
            }
            Message::DatasetClosed { path } => {
                self.datasets.retain(|dataset| dataset.path != path);
                Action::None
            }
            Message::OpenOrFocusDataset(dataset) => {
                if let Some(dataset) = self.get(dataset) {
                    if let Some(id) = dataset.window.as_ref() {
                        Action::Run(iced::window::gain_focus(id.clone()))
                    } else {
                        todo!("dataset window not found");
                    }
                } else {
                    todo!("dataset not found");
                }
            }
            Message::FocusWindow(id) => {
                Action::Run(iced::window::gain_focus::<Message>(id).discard())
            }
            #[cfg(feature = "project")]
            Message::RequestSaveProject => self.request_save_project(),
            #[cfg(feature = "project")]
            Message::RequestOpenProject => self.request_open_project(),
        }
    }

    pub fn view(&self) -> iced::Element<'_, Message> {
        let menu = iced_aw::menu_bar!((
            iced::widget::button("File"),
            #[cfg(feature = "project")]
            iced_aw::Menu::new(iced_aw::menu_items!(
                (iced::widget::button("Save project").on_press(Message::RequestSaveProject)),
                (iced::widget::button("Open project").on_press(Message::RequestOpenProject))
            ))
        ));

        let btn_open_dataset_file =
            iced::widget::button(icon::file()).on_press(Message::PromptOpenDatasetFile);
        let btn_open_dataset_dir =
            iced::widget::button(icon::folder()).on_press(Message::PromptOpenDatasetDir);
        let btn_open_settings = iced::widget::button(icon::cog()).on_press(Message::OpenSettings);
        let dataset_commands = iced::widget::row![
            btn_open_dataset_file,
            btn_open_dataset_dir,
            iced::widget::space::horizontal(),
            btn_open_settings,
        ];

        let common_base_path =
            common_path_all(self.datasets.iter().map(|dataset| dataset.path.as_path()))
                .unwrap_or(PathBuf::new());
        let dataset_list = iced::widget::column(self.datasets.iter().map(|dataset| {
            let label = if let Some(label) = &dataset.label {
                label.clone()
            } else {
                let label = dataset
                    .path
                    .strip_prefix(&common_base_path)
                    .unwrap()
                    .to_string_lossy()
                    .to_string();

                if label.is_empty() {
                    dataset
                        .path
                        .file_name()
                        .unwrap()
                        .to_string_lossy()
                        .to_string()
                } else {
                    label
                }
            };

            let btn_dataset = iced::widget::button(iced::widget::text(label))
                .on_press(Message::OpenOrFocusDataset(dataset.path.clone()));

            let child_settings = dataset.children.settings.as_ref().map(|window| {
                iced::widget::button(iced::widget::text("Settings"))
                    .on_press(Message::FocusWindow(window.clone()))
            });

            let child_data_table = dataset.children.data_table.as_ref().map(|window| {
                iced::widget::button(iced::widget::text("Data table"))
                    .on_press(Message::FocusWindow(window.clone()))
            });

            let child_pipelines = dataset.children.pipeline.as_ref().map(|window| {
                iced::widget::button(iced::widget::text("Pipeline"))
                    .on_press(Message::FocusWindow(window.clone()))
            });

            let child_file_browser = dataset.children.file_browser.as_ref().map(|window| {
                iced::widget::button(iced::widget::text("File browser"))
                    .on_press(Message::FocusWindow(window.clone()))
            });

            let children = iced::widget::row![
                iced::widget::space().width(iced::Length::Fixed(20.0)),
                iced::widget::column![
                    child_settings,
                    child_data_table,
                    child_pipelines,
                    child_file_browser,
                ]
            ];
            iced::widget::column![btn_dataset, children].into()
        }));

        iced::widget::column![menu, dataset_commands, dataset_list].into()
    }
}

impl Workspace {
    fn maybe_open_dataset_file(&mut self) -> iced::Task<Message> {
        iced::Task::future(
            rfd::AsyncFileDialog::new()
                .set_title("Open dataset file")
                .pick_file(),
        )
        .then(|path| match path {
            None => iced::Task::none(),
            Some(fh) => iced::Task::done(Message::LoadDatasetFilePathSelected(
                fh.path().to_path_buf(),
            )),
        })
    }

    fn maybe_open_dataset_dir(&mut self) -> iced::Task<Message> {
        iced::Task::future(
            rfd::AsyncFileDialog::new()
                .set_title("Open dataset folder")
                .pick_folder(),
        )
        .then(|path| match path {
            None => iced::Task::none(),
            Some(fh) => {
                iced::Task::done(Message::LoadDatasetDirPathSelected(fh.path().to_path_buf()))
            }
        })
    }

    pub fn dataset_set_loading(&mut self, path: PathBuf) {
        if let Some(dataset) = self
            .datasets
            .iter_mut()
            .find(|dataset| dataset.path == path)
        {
            dataset.data = DatasetState::Loading;
        } else {
            self.datasets.push(Dataset {
                path,
                label: None,
                data: DatasetState::Loading,
                kind: None,
                window: None,
                children: Default::default(),
            });
        }
    }

    pub fn dataset_loaded(&mut self, path: PathBuf, kind: jpk::dataset::DatasetType) {
        let dataset = self
            .datasets
            .iter_mut()
            .find(|dataset| dataset.path == path)
            .expect("dataset loaded, but doesn't exist");

        dataset.data = DatasetState::Ok;
        let _ = dataset.kind.insert(kind);
    }

    fn insert_dataset_window(&mut self, path: PathBuf, window: iced::window::Id) {
        let dataset = self
            .datasets
            .iter_mut()
            .find(|dataset| dataset.path == path)
            .expect("dataset loaded, but doesn't exist");

        let _ = dataset.window.insert(window);
    }

    fn dataset_child_window_opened(
        &mut self,
        path: PathBuf,
        window: iced::window::Id,
        kind: dataset::ChildWindowType,
    ) {
        let dataset = self
            .datasets
            .iter_mut()
            .find(|dataset| dataset.path == path)
            .expect("dataset should exist");

        match kind {
            dataset::ChildWindowType::Settings => {
                assert!(
                    dataset.children.settings.is_none(),
                    "settings should not exist"
                );
                dataset.children.settings = Some(window);
            }
            dataset::ChildWindowType::DataTable => {
                assert!(
                    dataset.children.data_table.is_none(),
                    "data table should not exist"
                );
                dataset.children.data_table = Some(window);
            }
            dataset::ChildWindowType::Pipeline => {
                assert!(
                    dataset.children.pipeline.is_none(),
                    "pipeline should not exist"
                );
                dataset.children.pipeline = Some(window);
            }
            dataset::ChildWindowType::FileBrowser => {
                assert!(
                    dataset.children.file_browser.is_none(),
                    "file browser should not exist"
                );
                dataset.children.file_browser = Some(window);
            }
        }
    }

    fn dataset_child_window_closed(&mut self, path: PathBuf, kind: dataset::ChildWindowType) {
        let dataset = self
            .datasets
            .iter_mut()
            .find(|dataset| dataset.path == path)
            .expect("dataset should exist");

        dataset
            .children
            .take_child(kind)
            .expect("window should exist");
    }

    #[cfg(feature = "project")]
    fn request_save_project(&self) -> Action {
        match rfd::FileDialog::new()
            .add_filter("loki", &[crate::project::FILE_EXT])
            .save_file()
        {
            Some(path) => Action::SaveProject(path),
            None => Action::None,
        }
    }

    #[cfg(feature = "project")]
    fn request_open_project(&mut self) -> Action {
        match rfd::FileDialog::new()
            .add_filter("loki", &[crate::project::FILE_EXT])
            .pick_file()
        {
            Some(path) => Action::OpenProject(path),
            None => Action::None,
        }
    }
}

/// Find the common prefix, if any, between any number of paths
///
/// # Example
///
/// ```rust
/// # extern crate common_path;
/// use std::path::{PathBuf, Path};
/// use common_path::common_path_all;
///
/// # fn main() {
/// let baz = Path::new("/foo/bar/baz");
/// let quux = Path::new("/foo/bar/quux");
/// let foo = Path::new("/foo/bar/foo");
/// let prefix = common_path_all(vec![baz, quux, foo]).unwrap();
/// assert_eq!(prefix, Path::new("/foo/bar").to_path_buf());
/// # }
/// ```
///
/// # Notes
///
/// Vendored from [`common-path`](https://crates.io/crates/common-path) crate.
pub fn common_path_all<'a>(paths: impl IntoIterator<Item = &'a Path>) -> Option<PathBuf> {
    let mut path_iter = paths.into_iter();
    let mut result = path_iter.next()?.to_path_buf();
    for path in path_iter {
        if let Some(r) = common_path(&result, &path) {
            result = r;
        } else {
            return None;
        }
    }
    Some(result.to_path_buf())
}

/// Find the common prefix, if any, between 2 paths
///
/// # Example
///
/// ```rust
/// # extern crate common_path;
/// use std::path::{PathBuf, Path};
/// use common_path::common_path;
///
/// # fn main() {
/// let baz = Path::new("/foo/bar/baz");
/// let quux = Path::new("/foo/bar/quux");
/// let prefix = common_path(baz, quux).unwrap();
/// assert_eq!(prefix, Path::new("/foo/bar").to_path_buf());
/// # }
/// ```
///
/// # Notes
///
/// Vendored from [`common-path`](https://crates.io/crates/common-path) crate.
pub fn common_path<P, Q>(one: P, two: Q) -> Option<PathBuf>
where
    P: AsRef<Path>,
    Q: AsRef<Path>,
{
    let one = one.as_ref();
    let two = two.as_ref();
    let one = one.components();
    let two = two.components();
    let mut final_path = PathBuf::new();
    let mut found = false;
    let paths = one.zip(two);
    for (l, r) in paths {
        if l == r {
            final_path.push(l.as_os_str());
            found = true;
        } else {
            break;
        }
    }
    if found { Some(final_path) } else { None }
}
