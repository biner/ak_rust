use pyo3::prelude::*;

mod tick_binding;
mod trade_binding;
mod breakout_binding;
mod auction_binding;
mod false_sell_binding;
mod wt_binding;
mod signaling;

#[pymodule]
fn _native(_py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    tick_binding::register(m)?;
    trade_binding::register(m)?;
    breakout_binding::register(m)?;
    auction_binding::register(m)?;
    false_sell_binding::register(m)?;
    wt_binding::register(m)?;
    signaling::register(m)?;
    Ok(())
}
