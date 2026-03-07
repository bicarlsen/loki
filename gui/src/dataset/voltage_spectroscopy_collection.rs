//! Element for a voltage spectroscopy dataset collection (multiple `.jpk-voltage-ramp`).

use polars::prelude::{self as pl, *};

const DEFAULT_X_COL: &str = "x";
const DEFAULT_Y_COL: &str = "y";
const DEFAULT_SEGMENT_COL: &str = "segment";
const DEFAULT_COLOR_COL: &str = "ff_rel";
// TODO: expected columns, with validation

pub struct State {
    df: pl::DataFrame,
    color_col: &'static str,
}

#[derive(Debug, Clone)]
pub enum Message {
    SetColorCol(&'static str),
}

impl State {
    pub fn new(df: pl::DataFrame) -> Self {
        let df = photodiode_fit(&df);
        let df = df.select(["x", "y", "ff"]).unwrap();
        let df = df
            .lazy()
            .with_column((pl::col("ff") / pl::col("ff").max()).alias("ff_rel"))
            .collect()
            .unwrap();

        Self { df, color_col: "" }
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
        true
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

fn photodiode_fit(df: &pl::DataFrame) -> pl::DataFrame {
    let groups = df
        .group_by([DEFAULT_X_COL, DEFAULT_Y_COL, DEFAULT_SEGMENT_COL])
        .unwrap();
    groups
        .apply(|df| {
            let x = df
                .column("cafmBias")
                .unwrap()
                .f64()
                .unwrap()
                .into_no_null_iter()
                .collect::<Vec<_>>();
            let y = df
                .column("cafmCurrent")
                .unwrap()
                .f64()
                .unwrap()
                .into_no_null_iter()
                .collect::<Vec<_>>();

            let (a, b, c) = analysis::voltage_spectroscopy::exponential(&x, &y).unwrap();
            let jsc = a + c;
            let voc = (-c / a).ln() / b;
            let v_mpp = -(1.0 + (c / a).sqrt()) / b;
            let i_mpp = a * (b * v_mpp).exp() + c;
            let p_mpp = v_mpp * i_mpp;
            let ff = p_mpp / (jsc * voc);

            let x_pos = df
                .head(Some(1))
                .column("x")
                .unwrap()
                .f64()
                .unwrap()
                .into_no_null_iter()
                .collect::<Vec<_>>();
            let y_pos = df
                .head(Some(1))
                .column("y")
                .unwrap()
                .f64()
                .unwrap()
                .into_no_null_iter()
                .collect::<Vec<_>>();
            let segment = df.head(Some(1));
            let segment = segment
                .column("segment")
                .unwrap()
                .u8()
                .unwrap()
                .into_no_null_iter()
                .collect::<Vec<_>>();
            let x_pos = pl::Column::new("x".into(), x_pos);
            let y_pos = pl::Column::new("y".into(), y_pos);
            let segment = pl::Column::new("segment".into(), segment);
            let jsc = pl::Column::new("jsc".into(), [jsc]);
            let voc = pl::Column::new("voc".into(), [voc]);
            let v_mpp = pl::Column::new("v_mpp".into(), [v_mpp]);
            let i_mpp = pl::Column::new("i_mpp".into(), [i_mpp]);
            let p_mpp = pl::Column::new("p_mpp".into(), [p_mpp]);
            let ff = pl::Column::new("ff".into(), [ff]);
            let fits = pl::DataFrame::new(vec![
                x_pos, y_pos, segment, jsc, voc, v_mpp, i_mpp, p_mpp, ff,
            ])
            .unwrap();
            Ok(fits)
        })
        .unwrap()
}
