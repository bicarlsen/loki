//! Heatmap.
use polars::prelude as pl;

#[derive(Debug, Clone)]
pub struct Options {}

#[derive(Default, Debug, Clone)]
pub struct Axis {
    scale: super::axis::AxisScale,
    values: super::axis::IndexValues,
}

#[derive(Clone, Debug, Default)]
pub struct Index {
    x: Axis,
    y: Axis,
}

#[derive(Debug, Clone, derive_more::From)]
pub enum Message {}

#[derive(Debug)]
pub(super) struct State {}

impl State {
    pub fn new(df: pl::DataFrame, options: Options) -> Self {
        Self {}
    }

    pub fn view(&self) -> iced::Element<'_, Message> {
        iced::widget::column![].into()
    }
}
