//! Dataset.
use crate::icon;
use iced::widget;
use jpk_reader as jpk;
use polars::prelude as pl;
use std::{borrow::Cow, path::PathBuf};

mod data_table;
pub mod pipeline;
mod plot;
mod settings;
mod voltage_spectroscopy;
mod voltage_spectroscopy_collection;

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

pub enum Action {
    None,
    Run(iced::Task<Message>),
    ChildWindowOpened {
        window: iced::window::Id,
        kind: ChildWindowType,
    },
}

#[derive(derive_more::Debug, derive_more::From)]
pub enum Reader {
    VoltageSpectroscopy(#[debug(skip)] jpk::voltage_spectroscopy::v2_0::FileReader),
    VoltageSpectroscopyCollection(#[debug(skip)] jpk::voltage_spectroscopy::v2_0::DirReader),
}

#[derive(derive_more::From)]
enum DatasetKind {
    VoltageSpectroscopy(voltage_spectroscopy::State),
    VoltageSpectroscopyCollection(voltage_spectroscopy_collection::State),
}

#[derive(Default)]
pub struct Children {
    pub(crate) settings: Option<(iced::window::Id, settings::Settings)>,
    pub(crate) data_table: Option<(iced::window::Id, data_table::DataTable)>,
    pub(crate) pipeline: Option<iced::window::Id>,
    pub(crate) files_browser: Option<(iced::window::Id, ())>,
}

type SharedDataframe = std::sync::Arc<std::sync::RwLock<pl::DataFrame>>;

pub struct Dataset {
    path: PathBuf,
    window_id: iced::window::Id,
    df: SharedDataframe,
    reader: Reader,
    pipeline: pipeline::Pipeline,
    state: DatasetKind,
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
    //// # Returns
    /// Open window task.
    pub fn new(
        path: impl Into<PathBuf>,
        reader: Reader,
        df: pl::DataFrame,
    ) -> (Self, iced::Task<iced::window::Id>) {
        let df_pipeline = df.clone();
        let df = std::sync::Arc::new(std::sync::RwLock::new(df));
        let (state, plot) = match &reader {
            Reader::VoltageSpectroscopy(_) => {
                let state = voltage_spectroscopy::State::new();
                let plot =
                    plot::State::new(df.clone(), voltage_spectroscopy::State::default_options());
                (state.into(), plot.into())
            }
            Reader::VoltageSpectroscopyCollection(_) => {
                let state = voltage_spectroscopy_collection::State::new(df.clone());
                let plot = plot::State::new(
                    df.clone(),
                    voltage_spectroscopy_collection::State::default_options(),
                );
                (state.into(), plot.into())
            }
        };

        let (window_id, open) = iced::window::open(iced::window::Settings::default());
        (
            Self {
                path: path.into(),
                window_id,
                df,
                reader,
                pipeline: pipeline::Pipeline::new(df_pipeline),
                state,
                plot: plot,
                children: Children::default(),
            },
            open,
        )
    }

    #[must_use]
    pub fn update(&mut self, message: Message) -> Action {
        match message {
            Message::OpenSettings => {
                if let Some((id, _)) = &self.children.settings {
                    Action::Run(iced::window::gain_focus(id.clone()))
                } else {
                    let (_, open) = iced::window::open(iced::window::Settings::default());
                    Action::Run(open.map(Message::SettingsOpened))
                }
            }
            Message::SettingsOpened(id) => {
                assert!(self.children.settings.is_none(), "settings already exist");
                let settings = settings::Settings::new();
                self.children.settings = Some((id.clone(), settings));
                Action::ChildWindowOpened {
                    window: id.clone(),
                    kind: ChildWindowType::Settings,
                }
            }
            Message::SettingsClosed => {
                assert!(self.children.settings.is_some(), "settings should exist");
                self.children.settings = None;
                Action::None
            }
            Message::OpenDataTable => {
                if let Some((id, _)) = &self.children.data_table {
                    Action::Run(iced::window::gain_focus(id.clone()))
                } else {
                    let (_, open) = iced::window::open(iced::window::Settings::default());
                    Action::Run(open.map(Message::DataTableOpened))
                }
            }
            Message::DataTableOpened(id) => {
                assert!(
                    self.children.data_table.is_none(),
                    "data table already exists"
                );
                let data_table = data_table::DataTable::new(self.df.clone());
                self.children.data_table = Some((id.clone(), data_table));

                Action::ChildWindowOpened {
                    window: id.clone(),
                    kind: ChildWindowType::DataTable,
                }
            }
            Message::DataTableClosed => {
                assert!(
                    self.children.data_table.is_some(),
                    "data table should exist"
                );
                self.children.data_table = None;
                Action::None
            }
            Message::OpenPipeline => {
                if let Some(id) = self.children.pipeline {
                    Action::Run(iced::window::gain_focus(id.clone()))
                } else {
                    let settings = iced::window::Settings {
                        size: iced::Size {
                            width: 200.0,
                            height: 600.0,
                        },
                        ..Default::default()
                    };
                    let (_, open) = iced::window::open(settings);
                    Action::Run(open.map(Message::PipelineOpened))
                }
            }
            Message::PipelineOpened(id) => {
                assert!(self.children.pipeline.is_none(), "pipeline already exists");
                self.children.pipeline = Some(id.clone());
                Action::ChildWindowOpened {
                    window: id,
                    kind: ChildWindowType::Pipeline,
                }
            }
            Message::PipelineClosed => {
                assert!(self.children.pipeline.is_some());
                self.children.pipeline = None;
                Action::None
            }
            Message::OpenFilesBrowser => {
                todo!()
            }
            Message::FilesBrowserOpened(id) => todo!(),
            Message::FilesBrowserClosed => {
                assert!(self.children.files_browser.is_some());
                self.children.files_browser = None;
                Action::None
            }
            Message::Plot(message) => match self.plot.update(message) {
                plot::Action::None => Action::None,
                plot::Action::DataHovered(idx) => {
                    if let Some((_, data_table)) = self.children.data_table.as_mut() {
                        let message = if let Some(idx) = idx {
                            data_table::Message::HighlightRecord(idx)
                        } else {
                            data_table::Message::ClearHighlight
                        };
                        match data_table.update(message) {
                            data_table::Action::None => Action::None,
                        }
                    } else {
                        Action::None
                    }
                }
            },
            Message::Settings(message) => {
                if let settings::Message::SetMode(mode) = &message {
                    // self.plot.mode(mode);
                    todo!()
                }

                if let Some((_, settings)) = self.children.settings.as_mut() {
                    match settings.update(message) {
                        settings::Action::None => Action::None,
                    }
                } else {
                    Action::None
                }
            }
            Message::DataTable(message) => {
                if let Some((_, data_table)) = self.children.data_table.as_mut() {
                    match data_table.update(message) {
                        data_table::Action::None => Action::None,
                    }
                } else {
                    Action::None
                }
            }
            Message::Pipeline(message) => self.pipeline_update(message),
            Message::VoltageSpectroscopy(message) => {
                let DatasetKind::VoltageSpectroscopy(state) = &mut self.state else {
                    panic!("invalid message state")
                };

                match state.update(message) {
                    voltage_spectroscopy::Action::None => Action::None,
                }
            }
            Message::VoltageSpectroscopyCollection(message) => {
                let DatasetKind::VoltageSpectroscopyCollection(state) = &mut self.state else {
                    panic!("invalid message state")
                };

                match state.update(message) {
                    voltage_spectroscopy_collection::Action::None => Action::None,
                }
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
            // let btn_file_browser = self
            //     .state
            //     .is_file_collection()
            //     .then_some(widget::button("Files browser").on_press(Message::OpenFilesBrowser));

            let controls = widget::column![
                btn_settings,
                btn_open_data_table,
                btn_pipeline,
                // btn_file_browser,
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
    fn pipeline_update(&mut self, message: pipeline::Message) -> Action {
        // match message {
        //     pipeline::Message::OutputUpdated => iced::Task::batch([
        //         self.pipeline.update(message).map(Message::Pipeline),
        //         iced::Task::done(
        //             data_table::Message::DataframeUpdated(self.pipeline.output().clone()).into(),
        //         ),
        //         iced::Task::done(
        //             plot::Message::DataframeChange(self.pipeline.output().clone()).into(),
        //         ),
        //     ]),
        //     _ => self.pipeline.update(message).map(Message::Pipeline),
        // }
        match self.pipeline.update(message) {
            pipeline::Action::None => Action::None,
            pipeline::Action::Run(task) => Action::Run(task.map(Message::Pipeline)),
        }
    }
}
