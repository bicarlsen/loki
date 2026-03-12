//! Workspace.
use super::dataset;
use crate::icon;
use iced::advanced::graphics::core::window;
use jpk_reader as jpk;
use std::path::{Path, PathBuf};

pub enum DatasetState {
    Ok,
    Err(String),
    Loading,
}

#[derive(Default)]
struct DatasetChildren {
    data_table: Option<iced::window::Id>,
    pipeline: Option<iced::window::Id>,
    file_browser: Option<iced::window::Id>,
}

impl DatasetChildren {
    pub fn take_child(&mut self, kind: dataset::ChildWindowType) -> Option<iced::window::Id> {
        match kind {
            dataset::ChildWindowType::DataTable => self.data_table.take(),
            dataset::ChildWindowType::Pipeline => self.pipeline.take(),
            dataset::ChildWindowType::FileBrowser => self.file_browser.take(),
        }
    }
}

struct Dataset {
    path: PathBuf,
    label: Option<String>,
    data: DatasetState,
    kind: Option<jpk::dataset::DatasetType>,
    window: Option<iced::window::Id>,
    children: DatasetChildren,
}

#[derive(Debug, Clone)]
pub enum Message {
    /// The workspace was opened.
    WorkspaceOpened(iced::window::Id),
    WorkspaceClosed(iced::window::Id),
    /// Open a dataset file chooser.
    PromptOpenDatasetFile,
    /// Open a dataset directory chooser.
    PromptOpenDatasetDir,
    /// The user selected a datset file path to try to open.
    DatasetFilePathSelected(PathBuf),
    /// The user selected a datset directory path to try to open.
    DatasetDirPathSelected(PathBuf),
    /// A dataset is loading.
    DatasetLoading {
        path: PathBuf,
    },
    /// A dataset loaded successfully.
    DatasetLoaded {
        path: PathBuf,
        kind: jpk::dataset::DatasetType,
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
}

pub(crate) struct Workspace {
    _title: String,
    datasets: Vec<Dataset>,
}

impl Default for Workspace {
    fn default() -> Self {
        Self {
            _title: "jpk reader".into(),
            datasets: Default::default(),
        }
    }
}

impl Workspace {
    pub fn title(&self, window: iced::window::Id) -> String {
        self._title.clone()
    }
}

impl Workspace {
    pub fn new() -> (Self, iced::Task<Message>) {
        let (_, open) = iced::window::open(window::Settings::default());

        (Self::default(), open.map(Message::WorkspaceOpened))
    }

    pub fn update(&mut self, message: Message) -> iced::Task<Message> {
        match message {
            Message::WorkspaceOpened(id) => iced::Task::none(),
            Message::WorkspaceClosed(id) => self.workspace_closed(id),
            Message::PromptOpenDatasetFile => self.prompt_open_dataset_file(),
            Message::PromptOpenDatasetDir => self.prompt_open_dataset_dir(),
            Message::DatasetFilePathSelected(_) => iced::Task::done(message),
            Message::DatasetDirPathSelected(_) => iced::Task::done(message),
            Message::DatasetLoading { path } => self.dataset_loading(path),
            Message::DatasetLoaded { path, kind } => self.dataset_loaded(path, kind),
            Message::DatasetWindowOpened { path, window } => {
                self.dataset_window_opened(path, window)
            }
            Message::DatasetChildWindowOpened { path, window, kind } => {
                self.dataset_child_window_opened(path, window, kind)
            }
            Message::DatasetChildWindowClosed { path, kind } => {
                self.dataset_child_window_closed(path, kind)
            }
            Message::DatasetError { path, error } => self.dataset_error(path, error),
            Message::DatasetClosed { path } => self.dataset_closed(path),
            Message::OpenOrFocusDataset(dataset) => {
                todo!()
            }
            Message::FocusWindow(id) => iced::window::gain_focus::<Message>(id).discard(),
        }
    }

    pub fn view(&self) -> iced::Element<'_, Message> {
        let btn_open_dataset_file =
            iced::widget::button(icon::file()).on_press(Message::PromptOpenDatasetFile);
        let btn_open_dataset_dir =
            iced::widget::button(icon::opendir()).on_press(Message::PromptOpenDatasetDir);
        let dataset_commands = iced::widget::column![iced::widget::row![
            btn_open_dataset_file,
            btn_open_dataset_dir
        ]];

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
                iced::widget::column![child_data_table, child_pipelines, child_file_browser,]
            ];
            iced::widget::column![btn_dataset, children].into()
        }));

        iced::widget::column![dataset_commands, dataset_list].into()
    }

    pub fn subscription(&self) -> iced::Subscription<Message> {
        iced::window::close_events().map(Message::WorkspaceClosed)
    }
}

impl Workspace {
    fn workspace_closed(&mut self, window: iced::window::Id) -> iced::Task<Message> {
        iced::Task::none()
    }

    fn prompt_open_dataset_file(&mut self) -> iced::Task<Message> {
        rfd::FileDialog::new()
            .set_title("Open dataset file")
            .pick_file()
            .map(|path| iced::Task::done(Message::DatasetFilePathSelected(path)))
            .unwrap_or(iced::Task::none())
    }

    fn prompt_open_dataset_dir(&mut self) -> iced::Task<Message> {
        rfd::FileDialog::new()
            .set_title("Open dataset folder")
            .pick_folder()
            .map(|path| iced::Task::done(Message::DatasetDirPathSelected(path)))
            .unwrap_or(iced::Task::none())
    }

    fn dataset_loading(&mut self, path: PathBuf) -> iced::Task<Message> {
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

        iced::Task::none()
    }

    fn dataset_loaded(
        &mut self,
        path: PathBuf,
        kind: jpk::dataset::DatasetType,
    ) -> iced::Task<Message> {
        let Some(dataset) = self
            .datasets
            .iter_mut()
            .find(|dataset| dataset.path == path)
        else {
            todo!("dataset loaded, but doesn't exist");
        };

        dataset.data = DatasetState::Ok;
        let _ = dataset.kind.insert(kind);
        iced::Task::none()
    }

    fn dataset_window_opened(
        &mut self,
        path: PathBuf,
        window: iced::window::Id,
    ) -> iced::Task<Message> {
        let Some(dataset) = self
            .datasets
            .iter_mut()
            .find(|dataset| dataset.path == path)
        else {
            todo!("dataset loaded, but doesn't exist");
        };

        let _ = dataset.window.insert(window);
        iced::Task::none()
    }

    fn dataset_child_window_opened(
        &mut self,
        path: PathBuf,
        window: iced::window::Id,
        kind: dataset::ChildWindowType,
    ) -> iced::Task<Message> {
        let dataset = self
            .datasets
            .iter_mut()
            .find(|dataset| dataset.path == path)
            .expect("dataset should exist");

        match kind {
            dataset::ChildWindowType::DataTable => {
                assert!(
                    dataset.children.data_table.is_none(),
                    "data table already exists"
                );
                let _ = dataset.children.data_table.insert(window);
            }
            dataset::ChildWindowType::Pipeline => {
                assert!(
                    dataset.children.pipeline.is_none(),
                    "pipeline already exists"
                );
                let _ = dataset.children.pipeline.insert(window);
            }
            dataset::ChildWindowType::FileBrowser => {
                assert!(
                    dataset.children.file_browser.is_none(),
                    "file browser already exists"
                );
                let _ = dataset.children.file_browser.insert(window);
            }
        }
        iced::Task::none()
    }

    fn dataset_child_window_closed(
        &mut self,
        path: PathBuf,
        kind: dataset::ChildWindowType,
    ) -> iced::Task<Message> {
        let dataset = self
            .datasets
            .iter_mut()
            .find(|dataset| dataset.path == path)
            .expect("dataset should exist");

        dataset
            .children
            .take_child(kind)
            .expect("window should exist");

        iced::Task::none()
    }

    fn dataset_error(&mut self, path: PathBuf, error: String) -> iced::Task<Message> {
        todo!("dataset error: {error}")
    }

    fn dataset_closed(&mut self, path: PathBuf) -> iced::Task<Message> {
        self.datasets.retain(|dataset| dataset.path != path);
        iced::Task::none()
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
