use pyo3::prelude::*;

pub mod analyzer;
pub mod breakout;
pub mod screening;

pub use analyzer::register as register_analyzer;
pub use breakout::register as register_breakout;
pub use screening::register as register_screening;

pub fn register_all(m: &Bound<'_, PyModule>) -> PyResult<()> {
    register_analyzer(m)?;
    register_breakout(m)?;
    register_screening(m)?;
    Ok(())
}
