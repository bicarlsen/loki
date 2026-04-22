//! Data set plot.
// TODO: Reset button to reset plot scaling and panning.

use super::SharedDataframe;
use polars::prelude as pl;

pub mod heatmap;
pub mod scatter;

#[derive(Debug, Clone, derive_more::From)]
pub enum Message {
    DataframeChange(pl::DataFrame),
    SetMode(mode::Options),
    Mode(mode::Message),
}

impl From<scatter::Message> for Message {
    fn from(value: scatter::Message) -> Self {
        Self::Mode(value.into())
    }
}

impl From<heatmap::Message> for Message {
    fn from(value: heatmap::Message) -> Self {
        Self::Mode(value.into())
    }
}

pub enum Action {
    None,
    DataHovered(Option<usize>),
}

pub struct State {
    mode: mode::Mode,
}

impl State {
    pub fn new(df: SharedDataframe, options: impl Into<mode::Options>) -> Self {
        let options = options.into();
        let mode = match options {
            mode::Options::Scatter(options) => scatter::State::new(df, options).into(),
            mode::Options::Heatmap(options) => heatmap::State::new(df, options).into(),
        };

        Self { mode }
    }
}

impl State {
    pub fn update(&mut self, message: Message) -> Action {
        match message {
            Message::DataframeChange(df) => self.dataframe_change(df),
            Message::SetMode(mode) => todo!("{mode:?}"),
            Message::Mode(message) => match message {
                mode::Message::Scatter(message) => {
                    let mode::Mode::Scatter(state) = &mut self.mode else {
                        panic!("invalid message for state");
                    };

                    match state.update(message) {
                        scatter::Action::None => Action::None,
                        scatter::Action::DataHovered(idx) => Action::DataHovered(idx),
                    }
                }
                mode::Message::Heatmap(message) => {
                    let mode::Mode::Heatmap(state) = &mut self.mode else {
                        panic!("invalid message for state");
                    };

                    match state.update(message) {
                        heatmap::Action::None => Action::None,
                        heatmap::Action::DataHovered(idx) => Action::DataHovered(idx),
                    }
                }
            },
        }
    }

    fn dataframe_change(&mut self, df: pl::DataFrame) -> Action {
        todo!("dataframe change");
    }
}

impl State {
    pub fn view(&self) -> iced::Element<'_, Message> {
        match &self.mode {
            mode::Mode::Scatter(state) => state.view().map(Into::into),
            mode::Mode::Heatmap(state) => state.view().map(Into::into),
        }
    }
}

pub mod axis {
    #[derive(Default, Debug, Clone, Copy)]
    pub enum AxisScale {
        #[default]
        Linear,
        Log,
    }

    #[derive(Clone, Debug)]
    pub enum IndexValues {
        /// Use dataframe index as axis values.
        Index,
        /// Use a column from the dataframe as axis values.
        /// Value is the name of the column.
        Series(String),
    }

    impl IndexValues {
        pub fn take(&mut self) -> Option<String> {
            match std::mem::replace(self, Self::Index) {
                IndexValues::Index => None,
                IndexValues::Series(label) => Some(label),
            }
        }

        pub fn insert(&mut self, value: impl Into<String>) -> Option<String> {
            match std::mem::replace(self, Self::Series(value.into())) {
                IndexValues::Index => None,
                IndexValues::Series(label) => Some(label),
            }
        }
    }

    impl Default for IndexValues {
        fn default() -> Self {
            Self::Index
        }
    }
}

pub mod mode {
    use super::{heatmap, scatter};

    #[derive(Debug, Clone, derive_more::From)]
    pub enum Message {
        Scatter(scatter::Message),
        Heatmap(heatmap::Message),
    }
    #[derive(PartialEq, Eq, Default, Debug, Clone, Copy)]
    #[cfg_attr(feature = "project", derive(serde::Serialize, serde::Deserialize))]
    pub enum Kind {
        #[default]
        Scatter,
        Heatmap,
    }

    impl ToString for Kind {
        fn to_string(&self) -> String {
            match self {
                Kind::Scatter => "Scatter".to_string(),
                Kind::Heatmap => "Heatmap".to_string(),
            }
        }
    }

    #[derive(Debug, Clone, derive_more::From)]
    pub enum Options {
        Scatter(scatter::Options),
        Heatmap(heatmap::Options),
    }

    #[derive(derive_more::Debug, derive_more::From)]
    pub(super) enum Mode {
        #[debug("Scatter")]
        Scatter(scatter::State),
        #[debug("Heatmap")]
        Heatmap(heatmap::State),
    }
}

pub mod chart {
    use iced_aksel as aksel;

    pub type ValueType = f64;

    fn axis_renderer_marker(
        ctx: aksel::axis::MarkerContext<ValueType>,
    ) -> Option<aksel::axis::Marker> {
        if !ctx.cursor_on_plot && !ctx.cursor_on_axis {
            return None;
        }

        Some(ctx.marker(format!("{:.02e}", ctx.value)))
    }

    fn axis_renderer_ticks()
    -> impl Fn(aksel::axis::TickContext<ValueType>) -> aksel::axis::TickResult + 'static {
        move |ctx: aksel::axis::TickContext<ValueType>| {
            let text = format!("{:.02e}", ctx.tick.value);
            let label = ctx.label(text);

            aksel::axis::TickResult {
                label: Some(label),
                label_badge: Some(ctx.label_badge()),
                tick_line: Some(ctx.tickline()),
                grid_line: Some(ctx.gridline()),
                label_priority: None,
            }
        }
    }
}

mod widget {
    use super::axis;
    use polars::prelude as pl;

    fn pl_index_axis<'a, Message, F>(
        df: &'a pl::DataFrame,
        selected: &'a axis::IndexValues,
        message: F,
    ) -> iced::Element<'a, Message>
    where
        Message: Clone + 'a,
        F: Fn(axis::IndexValues) -> Message + 'a,
    {
        let columns = df
            .schema()
            .iter()
            .map(|(name, _)| name.to_string())
            .collect::<Vec<_>>();
        let columns = std::iter::once("".to_string())
            .chain(columns)
            .collect::<Vec<_>>();

        iced::widget::pick_list(
            columns,
            match selected {
                axis::IndexValues::Index => None,
                axis::IndexValues::Series(column) => Some(column.clone()),
            },
            move |selection| {
                let values = if selection.is_empty() {
                    axis::IndexValues::Index
                } else {
                    axis::IndexValues::Series(selection)
                };

                message(values)
            },
        )
        .placeholder("<index>")
        .into()
    }
}

mod utils {
    use palette::IntoColor;
    use polars::prelude as pl;

    #[inline]
    pub fn column_minmax_f64(column: &pl::Column) -> (f64, f64) {
        let values = column.as_series().unwrap();
        match column.dtype() {
            pl::DataType::UInt8 => {
                let min = values.min::<u8>().unwrap().unwrap();
                let max = values.max::<u8>().unwrap().unwrap();
                (min as f64, max as f64)
            }
            pl::DataType::Float64 => {
                let min = values.min::<f64>().unwrap().unwrap();
                let max = values.max::<f64>().unwrap().unwrap();
                (min, max)
            }
            pl::DataType::Int64 => {
                let min = values.min::<i64>().unwrap().unwrap();
                let max = values.max::<i64>().unwrap().unwrap();
                (min as f64, max as f64)
            }
            pl::DataType::Int128 => {
                let min = values.min::<i128>().unwrap().unwrap();
                let max = values.max::<i128>().unwrap().unwrap();
                (min as f64, max as f64)
            }
            _ => todo!(),
        }
    }

    #[inline]
    pub fn column_to_values_f64(column: &pl::Column) -> Vec<f64> {
        match column.dtype() {
            pl::DataType::Float64 => column.f64().unwrap().into_no_null_iter().collect(),
            pl::DataType::UInt8 => column
                .u8()
                .unwrap()
                .into_no_null_iter()
                .map(|v| v as f64)
                .collect(),
            pl::DataType::Int64 => column
                .i64()
                .unwrap()
                .into_no_null_iter()
                .map(|v| v as f64)
                .collect(),
            kind => todo!("{kind:?}"),
        }
    }

    #[inline]
    pub(super) fn color_to_lch(color: iced::Color) -> palette::Oklch<f32> {
        let [r, g, b, a] = color.into_linear();

        palette::rgb::Srgba::<f32>::from_linear(palette::LinSrgba::new(r, g, b, a)).into_color()
    }

    #[inline]
    pub(super) fn lch_to_color(lch: palette::Oklch<f32>) -> iced::Color {
        let color: palette::LinSrgba = lch.into_color();
        let (r, g, b, a) = color.into_components();
        iced::Color::from_linear_rgba(r, g, b, a)
    }
}
