//! Element for a single voltage spectroscopy dataset (`.jpk-voltage-ramp`).
use super::plot::scatter;
use polars::prelude::{self as pl, *};

const DEFAULT_X_COL: &str = "cafmBias";
const DEFAULT_Y_COL: &str = "cafmCurrent";

pub struct State {
    df: pl::DataFrame,
    x_col: &'static str,
    y_col: &'static str,
}

#[derive(Debug, Clone)]
pub enum Message {
    SetXCol(&'static str),
    SetYCol(&'static str),
}

impl State {
    pub fn new(dataframe: pl::DataFrame) -> Self {
        Self {
            df: dataframe,
            x_col: DEFAULT_X_COL,
            y_col: DEFAULT_Y_COL,
        }
    }

    pub fn update(&mut self, message: Message) -> iced::Task<Message> {
        todo!()
    }

    pub fn view(&self) -> iced::Element<'_, Message> {
        todo!()
    }

    pub fn default_options() -> scatter::Options {
        let x = scatter::axis::IndexAxis::new(super::plot::axis::IndexValues::Series(
            DEFAULT_X_COL.to_string(),
        ));
        let mut y = scatter::axis::ValueAxis::new(0);
        y.add_trace(DEFAULT_Y_COL);
        scatter::Options::new(scatter::Index::new(x, vec![y]))
    }
}
