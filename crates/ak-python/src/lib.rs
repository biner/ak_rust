use pyo3::prelude::*;

mod tick;
mod transaction;
mod auction;
mod dispatch;
mod generate_all;

#[pymodule]
fn _native(_py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    tick::register(m)?;
    transaction::register_all(m)?;
    auction::register_all(m)?;
    dispatch::register(m)?;
    generate_all::register(m)?;
    Ok(())
}
