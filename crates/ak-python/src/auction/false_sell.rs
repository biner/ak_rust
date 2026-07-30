use pyo3::prelude::*;
use pyo3::types::{PyAny, PyDict, PyList};
use ak_core::auction::scan::AuctionTick;
use ak_core::auction::false_sell::FalseSellDetector;

const VERSION: &str = env!("CARGO_PKG_VERSION");

#[pyfunction]
fn false_sell_version() -> String { VERSION.to_string() }

#[pyfunction(name = "auction_false_sell")]
#[pyo3(signature = (data, threshold=0.5))]
fn analyze_false_sell(
    py: Python<'_>,
    data: &Bound<'_, PyList>,
    threshold: f64,
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

    let detector = FalseSellDetector::new(threshold);
    match detector.analyze_inner(&ticks) {
        None => Ok(py.None()),
        Some(r) => {
            let d = PyDict::new(py);
            d.set_item("score", r.score)?;
            d.set_item("match", r.match_)?;
            d.set_item("neg_ratio", r.neg_ratio)?;
            d.set_item("price_up", r.price_up)?;
            d.set_item("vr", r.vr)?;
            d.set_item("last_um", r.last_um)?;
            d.set_item("neg_total", r.neg_total)?;
            d.set_item("late_peak", r.late_peak)?;
            d.set_item("recovery_score", r.recovery_score)?;
            d.set_item("n", r.n)?;
            d.set_item("max_match", r.max_match)?;
            d.set_item("price_change_pct", r.price_change_pct)?;
            d.set_item("net_um", r.net_um)?;
            d.set_item("total_amt", r.total_amt)?;
            d.set_item("big_order_ratio", r.big_order_ratio)?;
            Ok(d.into_any().into())
        }
    }
}

pub fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(false_sell_version, m)?)?;
    m.add_function(wrap_pyfunction!(analyze_false_sell, m)?)?;
    Ok(())
}
