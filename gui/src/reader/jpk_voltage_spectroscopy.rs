//! Element for a single voltage spectroscopy dataset (`.jpk-voltage-ramp`).
use crate::dataset::plot;
use std::path::PathBuf;

const DEFAULT_X_COL: &str = "cafmBias";
const DEFAULT_Y_COL: &str = "cafmCurrent";

#[derive(derive_more::Deref, derive_more::From)]
pub struct Reader {
    inner: jpk_reader::voltage_spectroscopy::v2_0::FileReader,
}

impl Reader {
    fn new(
        path: PathBuf,
    ) -> Result<Self, jpk_reader::dataset::error::Error<jpk_reader::dataset::error::Dataset>> {
        let inner = jpk_reader::voltage_spectroscopy::v2_0::FileReader::new(path)?;
        Ok(Self { inner })
    }

    pub fn default_options() -> plot::scatter::Options {
        let x = plot::scatter::axis::IndexAxis::new(plot::axis::IndexValues::Series(
            DEFAULT_X_COL.to_string(),
        ));
        let mut y = plot::scatter::axis::ValueAxis::new(0);
        y.add_trace(DEFAULT_Y_COL);
        plot::scatter::Options::new(plot::scatter::Index::new(x, vec![y]))
    }
}
