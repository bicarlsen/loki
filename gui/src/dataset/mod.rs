use iced::{Task, widget::container};
use jpk_reader as jpk;
use polars::prelude as pl;
use std::{borrow::Cow, path::PathBuf};

mod plot;
mod voltage_spectroscopy;
mod voltage_spectroscopy_collection;

trait IsFileCollection {
    fn is_file_collection(&self) -> bool;
}

trait PlotOptions {
    fn plot_options(&self) -> plot::Options;
}

#[derive(Clone, Copy, Debug)]
pub enum ChildWindowType {
    DataTable,
    Pipeline,
    FileBrowser,
}

impl<'a> iced::advanced::text::IntoFragment<'a> for ChildWindowType {
    fn into_fragment(self) -> iced::widget::text::Fragment<'a> {
        match self {
            ChildWindowType::DataTable => Cow::Borrowed("Data table"),
            ChildWindowType::Pipeline => Cow::Borrowed("Pipeline"),
            ChildWindowType::FileBrowser => Cow::Borrowed("File browser"),
        }
    }
}

#[derive(Debug, Clone, derive_more::From)]
pub enum Message {
    WindowOpened(iced::window::Id),
    ChildWindowOpened {
        window: iced::window::Id,
        kind: ChildWindowType,
    },
    OpenDataTable,
    DataTableOpened(iced::window::Id),
    DataTableClosed,
    OpenPipeline,
    PipelineOpened(iced::window::Id),
    PipelineClosed,
    OpenFilesBrowser,
    FilesBrowserOpened(iced::window::Id),
    FilesBrowserClosed,
    #[from]
    Plot(plot::Message),
    #[from]
    DataTable(data_table::Message),
    #[from]
    Pipeline(pipeline::Message),
    #[from]
    VoltageSpectroscopy(voltage_spectroscopy::Message),
    #[from]
    VoltageSpectroscopyCollection(voltage_spectroscopy_collection::Message),
}

#[derive(derive_more::Debug, derive_more::From)]
pub enum Reader {
    VoltageSpectroscopy(#[debug(skip)] jpk::voltage_spectroscopy::v2_0::FileReader),
    VoltageSpectroscopyCollection(#[debug(skip)] jpk::voltage_spectroscopy::v2_0::DirReader),
}

enum DatasetState {
    VoltageSpectroscopy(voltage_spectroscopy::State),
    VoltageSpectroscopyCollection(voltage_spectroscopy_collection::State),
}

impl IsFileCollection for DatasetState {
    fn is_file_collection(&self) -> bool {
        match self {
            DatasetState::VoltageSpectroscopy(state) => state.is_file_collection(),
            DatasetState::VoltageSpectroscopyCollection(state) => state.is_file_collection(),
        }
    }
}

impl PlotOptions for DatasetState {
    fn plot_options(&self) -> plot::Options {
        match self {
            DatasetState::VoltageSpectroscopy(state) => state.plot_options(),
            DatasetState::VoltageSpectroscopyCollection(state) => state.plot_options(),
        }
    }
}

#[derive(Default)]
struct Children {
    data_table: Option<(iced::window::Id, data_table::DataTable)>,
    pipeline: Option<(iced::window::Id, pipeline::Pipeline)>,
    files_browser: Option<(iced::window::Id, ())>,
}

pub struct Dataset {
    path: PathBuf,
    window_id: iced::window::Id,
    reader: Reader,
    pipeline: pipeline::Pipeline,
    state: DatasetState,
    plot: plot::State,
    children: Children,
}

impl Dataset {
    pub fn path(&self) -> &PathBuf {
        &self.path
    }

    pub fn kind(&self) -> jpk::dataset::DatasetType {
        match &self.reader {
            Reader::VoltageSpectroscopy(_) => jpk::dataset::DatasetType::VoltageSpectroscopy,
            Reader::VoltageSpectroscopyCollection(_) => {
                jpk::dataset::DatasetType::VoltageSpectroscopyCollection
            }
        }
    }
}

impl Dataset {
    pub fn new(
        path: impl Into<PathBuf>,
        reader: Reader,
        dataframe: pl::DataFrame,
    ) -> (Self, iced::Task<Message>) {
        let state = match &reader {
            Reader::VoltageSpectroscopy(_) => DatasetState::VoltageSpectroscopy(
                voltage_spectroscopy::State::new(dataframe.clone()),
            ),
            Reader::VoltageSpectroscopyCollection(_) => {
                DatasetState::VoltageSpectroscopyCollection(
                    voltage_spectroscopy_collection::State::new(dataframe.clone()),
                )
            }
        };

        let plot = plot::State::new(dataframe.clone(), state.plot_options()).unwrap();
        let (window_id, open) = iced::window::open(iced::window::Settings::default());
        (
            Self {
                path: path.into(),
                window_id,
                reader,
                pipeline: pipeline::Pipeline::new(dataframe),
                state,
                plot,
                children: Children::default(),
            },
            open.map(Message::WindowOpened),
        )
    }

    pub fn update(&mut self, message: Message) -> iced::Task<Message> {
        match message {
            Message::WindowOpened(_) => iced::Task::none(),
            Message::ChildWindowOpened { .. } => iced::Task::none(),
            Message::OpenDataTable => {
                if let Some((id, _)) = &self.children.data_table {
                    iced::window::gain_focus(id.clone())
                } else {
                    let (_, open) = iced::window::open(iced::window::Settings::default());
                    open.map(Message::DataTableOpened)
                }
            }
            Message::DataTableOpened(id) => {
                assert!(
                    self.children.data_table.is_none(),
                    "data table already exists"
                );
                let data_table = data_table::DataTable::new(self.pipeline.output().clone());
                let _ = self.children.data_table.insert((id.clone(), data_table));
                iced::Task::done(Message::ChildWindowOpened {
                    window: id.clone(),
                    kind: ChildWindowType::DataTable,
                })
            }
            Message::DataTableClosed => {
                assert!(self.children.data_table.is_some());
                let _ = self.children.data_table.take();
                Task::none()
            }
            Message::OpenPipeline => {
                if let Some((id, _)) = self.children.pipeline {
                    iced::window::gain_focus(id.clone())
                } else {
                    let (_, open) = iced::window::open(iced::window::Settings::default());
                    open.map(Message::PipelineOpened)
                }
            }
            Message::PipelineOpened(id) => {
                assert!(self.children.pipeline.is_none(), "pipeline already exists");
                let _ = self
                    .children
                    .pipeline
                    .insert((id.clone(), self.pipeline.clone()));

                iced::Task::batch([
                    iced::Task::done(Message::WindowOpened(id.clone())),
                    iced::Task::done(Message::ChildWindowOpened {
                        window: id.clone(),
                        kind: ChildWindowType::Pipeline,
                    }),
                ])
            }
            Message::PipelineClosed => {
                assert!(self.children.pipeline.is_some());
                let _ = self.children.pipeline.take();
                Task::none()
            }
            Message::OpenFilesBrowser => {
                todo!()
            }
            Message::FilesBrowserOpened(id) => todo!(),
            Message::FilesBrowserClosed => {
                assert!(self.children.files_browser.is_some());
                let _ = self.children.files_browser.take();
                Task::none()
            }
            Message::Plot(message) => self.plot.update(message).map(Into::into),
            Message::DataTable(message) => {
                let (_, data_table) = self
                    .children
                    .data_table
                    .as_mut()
                    .expect("data table should exist");
                data_table.update(message).map(Message::DataTable)
            }
            Message::Pipeline(message) => {
                let (_, pipeline) = self
                    .children
                    .pipeline
                    .as_mut()
                    .expect("pipeline should exist");
                pipeline.update(message).map(Message::Pipeline)
            }
            Message::VoltageSpectroscopy(message) => {
                let DatasetState::VoltageSpectroscopy(state) = &mut self.state else {
                    panic!("invalid message state")
                };

                state.update(message).map(Into::into)
            }
            Message::VoltageSpectroscopyCollection(message) => {
                let DatasetState::VoltageSpectroscopyCollection(state) = &mut self.state else {
                    panic!("invalid message state")
                };

                state.update(message).map(Into::into)
            }
        }
    }

    pub fn view(&self, window: &iced::window::Id) -> iced::Element<'_, Message> {
        if self.window_id == *window {
            let plot_container = iced::widget::container(self.plot.view().map(Message::Plot));

            let btn_open_data_table =
                iced::widget::button("Data table").on_press(Message::OpenDataTable);

            let btn_pipeline = iced::widget::button("Pipeline").on_press(Message::OpenPipeline);

            let btn_file_browser = self.state.is_file_collection().then_some(
                iced::widget::button("Files browser").on_press(Message::OpenFilesBrowser),
            );

            let controls =
                iced::widget::column![btn_open_data_table, btn_pipeline, btn_file_browser,];

            let controls_container = container(controls);
            return iced::widget::row![plot_container, controls_container].into();
        }
        if let Some((id, data_table)) = &self.children.data_table {
            if id == window {
                return data_table.view().map(Message::DataTable);
            }
        }
        if let Some((id, pipeline)) = &self.children.pipeline {
            if id == window {
                return pipeline.view().map(Message::Pipeline);
            }
        }
        if let Some((id, files_browser)) = &self.children.files_browser {
            if id == window {
                todo!();
            }
        }

        panic!("invalid window id")
    }
}

mod data_table {
    use polars::prelude as pl;

    pub type Message = ();

    pub struct DataTable {
        df: pl::DataFrame,
    }

    impl DataTable {
        pub fn new(df: pl::DataFrame) -> Self {
            Self { df }
        }

        pub fn update(&mut self, message: Message) -> iced::Task<Message> {
            iced::Task::none()
        }

        pub fn view(&self) -> iced::Element<'_, Message> {
            use iced::widget::text;

            // TODO: Headers should be sticky
            // TODO: Columns fit to data instead of header title causing overflow
            let columns = self.df.schema().iter().map(|(name, _dtype)| {
                iced::widget::table::column(name.as_str(), |idx: usize| {
                    let col = self.df.column(name.as_str()).unwrap();
                    match col.get(idx).unwrap() {
                        pl::AnyValue::Null => text(""),
                        pl::AnyValue::Boolean(value) => {
                            if value {
                                text("true")
                            } else {
                                text("false")
                            }
                        }
                        pl::AnyValue::Float64(value) => text(format!("{value:?}")),
                        pl::AnyValue::String(value) => text(value),
                        pl::AnyValue::UInt8(value) => text(format!("{value:?}")),
                        _ => todo!(),
                    }
                })
            });

            let table = iced::widget::table::Table::new(columns, 0..self.df.height());
            iced::widget::scrollable(table).into()
        }
    }
}

mod pipeline {
    use polars::prelude as pl;
    use std::{borrow::Cow, path::PathBuf};

    #[derive(Clone, Debug)]
    pub enum Message {
        TransformPushed,
        PromptTransformScript,
        PushTransformScript(PathBuf),
    }

    #[derive(Clone)]
    pub enum DataTransform {
        Script {
            file: PathBuf,
            runner: TransformScriptRunner,
        },
    }

    impl<'a> iced::advanced::text::IntoFragment<'a> for &'a DataTransform {
        fn into_fragment(self) -> iced::widget::text::Fragment<'a> {
            match self {
                DataTransform::Script { file, runner: _ } => {
                    Cow::Owned(format!("Script ({file:?})"))
                }
            }
        }
    }

    #[derive(Clone)]
    pub struct TransformScriptRunner {
        cmd: String,
    }

    #[derive(Clone)]
    pub struct Pipeline {
        /// Original dataframe.
        raw: pl::DataFrame,
        /// Output of `raw` through `transforms`.
        output: pl::DataFrame,
        transforms: Vec<DataTransform>,
    }

    impl Pipeline {
        pub fn new(df: pl::DataFrame) -> Self {
            Self {
                raw: df.clone(),
                output: df.clone(),
                transforms: vec![],
            }
        }

        pub fn output(&self) -> &pl::DataFrame {
            &self.output
        }
    }

    impl Pipeline {
        pub fn update(&mut self, message: Message) -> iced::Task<Message> {
            match message {
                Message::TransformPushed => iced::Task::none(),
                Message::PromptTransformScript => self.prompt_new_transform_script(),
                Message::PushTransformScript(path) => {
                    self.transforms.push(DataTransform::Script {
                        file: path,
                        runner: TransformScriptRunner {
                            cmd: "python".to_string(),
                        },
                    });
                    iced::Task::done(Message::TransformPushed)
                }
            }
        }

        pub fn view(&self) -> iced::Element<'_, Message> {
            let btn_ctrl_add_script =
                iced::widget::button("Script").on_press(Message::PromptTransformScript);
            let controls_r1 = iced::widget::row![btn_ctrl_add_script];
            let controls = iced::widget::column![controls_r1];

            let stages = std::iter::once(iced::widget::text("raw").into())
                .chain(
                    self.transforms
                        .iter()
                        .map(|transform| iced::widget::text(transform).into()),
                )
                .collect::<Vec<iced::Element<'_, Message>>>();
            let pipeline = iced::widget::column(stages);

            iced::widget::column![controls, pipeline].into()
        }
    }

    impl Pipeline {
        fn prompt_new_transform_script(&mut self) -> iced::Task<Message> {
            rfd::FileDialog::new()
                .set_title("Open dataset file")
                .add_filter("Python", &["py"])
                .pick_file()
                .map(|path| iced::Task::done(Message::PushTransformScript(path)))
                .unwrap_or(iced::Task::none())
        }
    }
}
