//! Element for a single voltage spectroscopy dataset (`.jpk-voltage-ramp`).

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
}

impl super::IsFileCollection for State {
    fn is_file_collection(&self) -> bool {
        false
    }
}

impl super::PlotOptions for State {
    fn plot_options(&self) -> super::plot::Options {
        let mut options = super::plot::Options::new();
        options.x_axis(DEFAULT_X_COL);
        options.add_trace(0, DEFAULT_Y_COL);
        options
    }
}
