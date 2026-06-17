//! Element for a voltage spectroscopy dataset collection (multiple `.jpk-voltage-ramp`).
use std::{fs, path::PathBuf};

use crate::dataset::plot;
use polars::prelude::{self as pl, *};

const DEFAULT_X_COL: &str = "x";
const DEFAULT_Y_COL: &str = "y";
const DEFAULT_Z_COL: &str = "y";
const DEFAULT_SEGMENT_COL: &str = "segment";
const DEFAULT_COLOR_COL: &str = "ff_rel";
// TODO: expected columns, with validation

#[derive(derive_more::Deref, derive_more::From)]
pub struct Reader {
    inner: jpk_reader::voltage_spectroscopy::v2_0::DirReader,
}

impl Reader {
    fn new(
        path: PathBuf,
    ) -> Result<
        Self,
        jpk_reader::dataset::error::Error<
            jpk_reader::voltage_spectroscopy::v2_0::error::DataCollection,
        >,
    > {
        let dir_walker = fs::read_dir(&path).unwrap();
        let paths = dir_walker
            .into_iter()
            .filter_map(|entry| entry.ok())
            .filter_map(|entry| {
                let path = entry.path();
                let ext = path.extension()?.to_str()?;
                (path.is_file()
                    && ext == jpk_reader::voltage_spectroscopy::VOLTAGE_SPECTROSCOPY_FILE_EXT)
                    .then_some(path)
            })
            .collect::<Vec<_>>();

        let inner = jpk_reader::voltage_spectroscopy::v2_0::DirReader::new(paths)?;
        Ok(Self { inner })
    }

    pub fn default_options() -> plot::heatmap::Options {
        let x = plot::heatmap::axis::Axis::new(plot::axis::IndexValues::Series(
            DEFAULT_X_COL.to_string(),
        ));
        let y = plot::heatmap::axis::Axis::new(plot::axis::IndexValues::Series(
            DEFAULT_Y_COL.to_string(),
        ));
        let z = plot::heatmap::axis::Axis::new(plot::axis::IndexValues::Series(
            DEFAULT_Z_COL.to_string(),
        ));

        plot::heatmap::Options::new(plot::heatmap::Index::new(x, y, z))
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
