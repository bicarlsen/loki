use crate::icon;
use iced::widget;
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
    type YAxis;
    fn plot_options(&self) -> plot::Options<Self::YAxis>;
}

#[derive(Clone, Copy, Debug)]
pub enum ChildWindowType {
    Settings,
    DataTable,
    Pipeline,
    FileBrowser,
}

impl<'a> iced::advanced::text::IntoFragment<'a> for ChildWindowType {
    fn into_fragment(self) -> widget::text::Fragment<'a> {
        match self {
            ChildWindowType::Settings => Cow::Borrowed("Settings"),
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
    OpenSettings,
    SettingsOpened(iced::window::Id),
    SettingsClosed,
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
    Settings(settings::Message),
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
    type YAxis = Vec<plot::ValueAxis>;
    fn plot_options(&self) -> plot::Options<Self::YAxis> {
        match self {
            DatasetState::VoltageSpectroscopy(state) => state.plot_options(),
            DatasetState::VoltageSpectroscopyCollection(state) => state.plot_options(),
        }
    }
}

#[derive(Default)]
pub struct Children {
    pub(crate) settings: Option<(iced::window::Id, settings::Settings)>,
    pub(crate) data_table: Option<(iced::window::Id, data_table::DataTable)>,
    pub(crate) pipeline: Option<iced::window::Id>,
    pub(crate) files_browser: Option<(iced::window::Id, ())>,
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

    pub fn window_id(&self) -> &iced::window::Id {
        &self.window_id
    }

    pub fn children(&self) -> &Children {
        &self.children
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
            Message::OpenSettings => {
                if let Some((id, _)) = &self.children.settings {
                    iced::window::gain_focus(id.clone())
                } else {
                    let (_, open) = iced::window::open(iced::window::Settings::default());
                    open.map(Message::SettingsOpened)
                }
            }
            Message::SettingsOpened(id) => {
                assert!(self.children.settings.is_none(), "settings already exist");
                let settings = settings::Settings::new();
                self.children.settings = Some((id.clone(), settings));
                iced::Task::done(Message::ChildWindowOpened {
                    window: id.clone(),
                    kind: ChildWindowType::Settings,
                })
            }
            Message::SettingsClosed => {
                assert!(self.children.settings.is_some(), "settings should exist");
                self.children.settings = None;
                iced::Task::none()
            }
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
                self.children.data_table = Some((id.clone(), data_table));
                iced::Task::done(Message::ChildWindowOpened {
                    window: id.clone(),
                    kind: ChildWindowType::DataTable,
                })
            }
            Message::DataTableClosed => {
                assert!(
                    self.children.data_table.is_some(),
                    "data table should exist"
                );
                self.children.data_table = None;
                iced::Task::none()
            }
            Message::OpenPipeline => {
                if let Some(id) = self.children.pipeline {
                    iced::window::gain_focus(id.clone())
                } else {
                    let settings = iced::window::Settings {
                        size: iced::Size {
                            width: 200.0,
                            height: 600.0,
                        },
                        ..Default::default()
                    };
                    let (_, open) = iced::window::open(settings);
                    open.map(Message::PipelineOpened)
                }
            }
            Message::PipelineOpened(id) => {
                assert!(self.children.pipeline.is_none(), "pipeline already exists");
                self.children.pipeline = Some(id.clone());

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
                self.children.pipeline = None;
                iced::Task::none()
            }
            Message::OpenFilesBrowser => {
                todo!()
            }
            Message::FilesBrowserOpened(id) => todo!(),
            Message::FilesBrowserClosed => {
                assert!(self.children.files_browser.is_some());
                self.children.files_browser = None;
                iced::Task::none()
            }
            Message::Plot(message) => {
                let data_table_msg = if let Some((_, data_table)) =
                    self.children.data_table.as_mut()
                {
                    let data_table_msg = match &message {
                        plot::Message::Data(plot::data::Message::ShapeEnter { point, .. }) => {
                            let idx = self
                                .plot
                                .record_idx_by_point_id(point)
                                .expect("record should exist");

                            Some(data_table::Message::HighlightRecord(idx))
                        }
                        plot::Message::Data(plot::data::Message::ShapeExit) => {
                            Some(data_table::Message::ClearHighlight)
                        }
                        _ => None,
                    };

                    match data_table_msg {
                        None => iced::Task::none(),
                        Some(msg) => data_table.update(msg).map(Into::into),
                    }
                } else {
                    iced::Task::none()
                };

                iced::Task::batch([data_table_msg, self.plot.update(message).map(Into::into)])
            }
            Message::Settings(message) => {
                if let settings::Message::SetMode(mode) = &message {
                    self.plot.mode(mode);
                }

                if let Some((_, settings)) = self.children.settings.as_mut() {
                    settings.update(message).map(Message::Settings)
                } else {
                    iced::Task::none()
                }
            }
            Message::DataTable(message) => {
                if let Some((_, data_table)) = self.children.data_table.as_mut() {
                    data_table.update(message).map(Message::DataTable)
                } else {
                    iced::Task::none()
                }
            }
            Message::Pipeline(message) => self.pipeline_update(message),
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

    pub fn view(
        &self,
        theme: &iced::advanced::graphics::core::Theme,

        window: &iced::window::Id,
    ) -> iced::Element<'_, Message> {
        if self.window_id == *window {
            let plot_container = widget::container(self.plot.view().map(Message::Plot));
            let btn_settings = widget::button(icon::cog()).on_press(Message::OpenSettings);
            let btn_open_data_table = widget::button("Data table").on_press(Message::OpenDataTable);
            let btn_pipeline = widget::button("Pipeline").on_press(Message::OpenPipeline);
            let btn_file_browser = self
                .state
                .is_file_collection()
                .then_some(widget::button("Files browser").on_press(Message::OpenFilesBrowser));

            let controls = widget::column![
                btn_settings,
                btn_open_data_table,
                btn_pipeline,
                btn_file_browser,
            ];
            let controls_container = widget::container(controls);
            return widget::row![plot_container, controls_container].into();
        }
        if let Some((id, settings)) = &self.children.settings {
            if id == window {
                return settings.view().map(Message::Settings);
            }
        }
        if let Some((id, data_table)) = &self.children.data_table {
            if id == window {
                return data_table.view(theme).map(Message::DataTable);
            }
        }
        if let Some(id) = &self.children.pipeline {
            if id == window {
                return self.pipeline.view(&self.path).map(Message::Pipeline);
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

impl Dataset {
    fn pipeline_update(&mut self, message: pipeline::Message) -> iced::Task<Message> {
        match message {
            pipeline::Message::OutputUpdated => iced::Task::batch([
                self.pipeline.update(message).map(Message::Pipeline),
                iced::Task::done(
                    data_table::Message::DataframeUpdated(self.pipeline.output().clone()).into(),
                ),
                iced::Task::done(
                    plot::Message::DataframeChange(self.pipeline.output().clone()).into(),
                ),
            ]),
            _ => self.pipeline.update(message).map(Message::Pipeline),
        }
    }
}

mod settings {
    use crate::dataset::plot;

    #[derive(Debug, Clone)]
    pub enum Message {
        SetMode(plot::Mode),
    }

    #[derive(Default)]
    #[cfg_attr(feature = "project", derive(serde::Serialize, serde::Deserialize))]
    pub struct Settings {
        /// Plot mode.
        mode: super::plot::Mode,
    }

    impl Settings {
        pub fn new() -> Self {
            Default::default()
        }

        pub fn update(&mut self, message: Message) -> iced::Task<Message> {
            match message {
                Message::SetMode(mode) => {
                    self.mode = mode;
                    iced::Task::none()
                }
            }
        }

        pub fn view(&self) -> iced::Element<'_, Message> {
            let title = iced::widget::text("Settings");

            let pl_mode = iced::widget::pick_list(
                [super::plot::Mode::Scatter, super::plot::Mode::Heatmap],
                Some(self.mode),
                Message::SetMode,
            );
            let pl_mode = iced::widget::row![iced::widget::text("Plot mode"), pl_mode];

            iced::widget::column![title, pl_mode].into()
        }
    }
}

mod data_table {
    use iced::widget;
    use polars::prelude as pl;

    #[derive(Debug, Clone)]
    pub enum Message {
        DataframeUpdated(pl::DataFrame),
        /// Highlight the record at the given index.
        HighlightRecord(usize),
        /// No record should be highlighted.
        ClearHighlight,
    }

    pub struct DataTable {
        df: pl::DataFrame,
        highlight: Option<usize>,
    }

    impl DataTable {
        pub fn new(df: pl::DataFrame) -> Self {
            Self {
                df,
                highlight: Default::default(),
            }
        }

        pub fn update(&mut self, message: Message) -> iced::Task<Message> {
            match message {
                Message::DataframeUpdated(dataframe) => {
                    self.df = dataframe;
                    iced::Task::none()
                }
                Message::HighlightRecord(idx) => {
                    let _ = self.highlight.insert(idx);
                    iced::Task::none()
                }
                Message::ClearHighlight => {
                    let _ = self.highlight.take();
                    iced::Task::none()
                }
            }
        }

        pub fn view(
            &self,
            theme: &iced::advanced::graphics::core::Theme,
        ) -> iced::Element<'_, Message> {
            use widget::text;

            // TODO: Headers should be sticky
            // TODO: Columns fit to data instead of header title causing overflow
            let columns = self.df.schema().iter().map(|(name, _dtype)| {
                widget::table::column(name.as_str(), |idx: usize| {
                    let col = self.df.column(name.as_str()).unwrap();
                    let mut text = match col.get(idx).unwrap() {
                        pl::AnyValue::Null => text(""),
                        pl::AnyValue::Boolean(value) => {
                            if value {
                                text("true")
                            } else {
                                text("false")
                            }
                        }
                        pl::AnyValue::String(value) => text(value),
                        pl::AnyValue::Float64(value) => text(format!("{value:?}")),
                        pl::AnyValue::UInt8(value) => text(format!("{value:?}")),
                        pl::AnyValue::Int64(value) => text(format!("{value:?}")),
                        pl::AnyValue::Int128(value) => text(format!("{value:?}")),
                        value => todo!("display {value:?}"),
                    };
                    if let Some(highlight) = &self.highlight {
                        if idx == *highlight {
                            text = text.color(theme.palette().success);
                        }
                    }
                    text
                })
            });

            let table = widget::table::Table::new(columns, 0..self.df.height());
            widget::scrollable(table)
                .direction(iced::widget::scrollable::Direction::Both {
                    vertical: iced::widget::scrollable::Scrollbar::new(),
                    horizontal: iced::widget::scrollable::Scrollbar::new(),
                })
                .into()
        }
    }
}

pub mod pipeline {
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
                Message::TransformOutputUpdated(transform) => {
                    self.transform_output_updated(transform)
                }
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
            let btn_ctrl_add_script =
                widget::button("Script").on_press(Message::PromptTransformScript);
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
}
