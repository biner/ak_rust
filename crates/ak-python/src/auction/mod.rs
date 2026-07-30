use pyo3::prelude::*;

pub mod scan;
pub mod false_sell;

pub use scan::register as register_scan;
pub use false_sell::register as register_false_sell;

pub fn register_all(m: &Bound<'_, PyModule>) -> PyResult<()> {
    register_scan(m)?;
    register_false_sell(m)?;
    Ok(())
}
