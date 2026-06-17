//! Data readers

pub(crate) mod jpk_voltage_spectroscopy;
pub(crate) mod jpk_voltage_spectroscopy_collection;

#[derive(derive_more::Debug, derive_more::From)]
pub enum Reader {
    JpkVoltageSpectroscopy(#[debug(skip)] jpk_voltage_spectroscopy::Reader),
    JpkVoltageSpectroscopyCollection(#[debug(skip)] jpk_voltage_spectroscopy_collection::Reader),
}
