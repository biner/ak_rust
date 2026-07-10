use pyo3::prelude::*;
use pyo3::types::{PyAny, PyDict, PyList};
use ak_core::auction::scan::{AuctionScanner, AuctionTick};

const VERSION: &str = env!("CARGO_PKG_VERSION");

#[pyfunction]
fn auction_scanner_version() -> &'static str { VERSION }

#[pyfunction]
fn auction_morphology_names() -> Vec<&'static str> {
    ak_core::MORPHOLOGY_NAMES.to_vec()
}

#[pyfunction]
#[pyo3(signature = (data, threshold=0.5, open_price=None, pre_close=None, limit_up=None, limit_down=None))]
fn analyze_auction(
    py: Python<'_>,
    data: &Bound<'_, PyList>,
    threshold: f64,
    open_price: Option<f64>,
    pre_close: Option<f64>,
    limit_up: Option<f64>,
    limit_down: Option<f64>,
) -> PyResult<Py<PyAny>> {
    let mut ticks: Vec<AuctionTick> = Vec::with_capacity(data.len());
    for item in data.iter() {
        let d = item.cast::<PyDict>()?;
        let time_obj = d.get_item("time")?.ok_or_else(|| pyo3::exceptions::PyValueError::new_err("Missing 'time'"))?;
        let time: chrono::NaiveTime = if let Ok(s) = time_obj.extract::<String>() {
            chrono::NaiveTime::parse_from_str(&s, "%H:%M:%S").map_err(|e| pyo3::exceptions::PyValueError::new_err(format!("Invalid time '{s}': {e}")))?
        } else if let Ok(t) = time_obj.extract::<chrono::NaiveTime>() { t }
        else { return Err(pyo3::exceptions::PyTypeError::new_err("'time' must be str or datetime.time")); };
        let price: f64 = d.get_item("price")?.ok_or_else(|| pyo3::exceptions::PyValueError::new_err("Missing 'price'"))?.extract()?;
        let matched: i32 = d.get_item("matched")?.ok_or_else(|| pyo3::exceptions::PyValueError::new_err("Missing 'matched'"))?.extract()?;
        let unmatched: i32 = d.get_item("unmatched")?.ok_or_else(|| pyo3::exceptions::PyValueError::new_err("Missing 'unmatched'"))?.extract()?;
        ticks.push(AuctionTick { time, price, matched, unmatched });
    }

    let scanner = AuctionScanner::new(threshold);
    match scanner.analyze(&ticks, open_price, pre_close, limit_up, limit_down) {
        None => Ok(py.None()),
        Some(r) => {
            let d = PyDict::new(py);
            d.set_item("score", r.score)?;
            d.set_item("match_", r.match_)?;
            d.set_item("steps", r.steps)?;
            d.set_item("eff_steps", r.eff_steps)?;
            d.set_item("neg_pos_alt", r.neg_pos_alt)?;
            d.set_item("buy_rev", r.buy_rev)?;
            d.set_item("vr", r.vr)?;
            d.set_item("end_p", r.end_p)?;
            d.set_item("oh", r.oh)?;
            d.set_item("n", r.n)?;
            d.set_item("morphology", &r.morphology)?;
            d.set_item("morphology_conf", r.morphology_conf)?;
            d.set_item("morphology_tag", &r.morphology_tag)?;
            Ok(d.into_any().into())
        }
    }
}

pub fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(auction_scanner_version, m)?)?;
    m.add_function(wrap_pyfunction!(auction_morphology_names, m)?)?;
    m.add_function(wrap_pyfunction!(analyze_auction, m)?)?;
    Ok(())
}
