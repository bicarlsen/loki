//! # Datacube
//! An n-dimensional data array whose projections are visualized.
//! A projection is a function whose domain is data grouped across m-dimensions.
//! Clicking on a point in a projection opens a new window containing the entire data group.

use super::{SharedDataframe, chart::ValueType};
use iced_aksel as aksel;

#[derive(Debug, Clone)]
pub enum Index {
    D1(axis::Axis),
    D2 { x: axis::Axis, y: axis::Axis },
}

#[derive(Debug, Clone)]
pub struct Options {
    index: Index,
}

impl Options {
    pub fn new(index: Index) -> Self {
        Self { index }
    }
}

#[derive(Debug, Clone)]
pub enum PlotInteraction {
    Dragged(aksel::Delta),
    Scrolled(aksel::ScrollEvent<iced::Point>),
    ShapeEnter {
        point: aksel::interaction::Id,
        event: aksel::EnterEvent,
    },
    ShapeExit,
    ShapePress {
        point: aksel::interaction::Id,
        event: aksel::PressEvent<iced::Point>,
    },
}

#[derive(Debug, Clone, derive_more::From)]
pub enum Message {
    DataframeChanged,
    Plot(PlotInteraction),
}

pub enum Action {
    None,
}

pub struct State {
    chart: aksel::State<&'static str, ValueType>,
    df: SharedDataframe,
    options: Options,
}

impl State {
    pub fn new(df: SharedDataframe, options: Options) -> Self {
        Self {
            chart: aksel::State::new(),
            df,
            options,
        }
    }

    pub fn view(&self) -> iced::Element<'_, Message> {
        iced::widget::column![iced::widget::text("datacube")].into()
    }

    pub fn df(&self) -> SharedDataframe {
        self.df.clone()
    }
}

impl State {
    pub fn update(&mut self, message: Message) -> Action {
        match message {
            Message::DataframeChanged => todo!(),
            Message::Plot(interaction) => todo!(),
        }
    }
}

pub mod axis {
    use super::super::axis::{AxisScale, IndexValues};

    #[derive(Default, Debug, Clone)]
    pub struct Axis {
        scale: AxisScale,
        values: IndexValues,
    }

    impl Axis {
        pub fn new(values: IndexValues) -> Self {
            Self {
                scale: AxisScale::Linear,
                values,
            }
        }

        pub fn new_with_scale(values: IndexValues, scale: AxisScale) -> Self {
            Self { scale, values }
        }
    }
}
