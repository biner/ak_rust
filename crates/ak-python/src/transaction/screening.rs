use pyo3::prelude::*;
use pyo3::types::{PyAny, PyDict, PyList};
use ak_core::transaction::{ScreeningTxPoint, ScreeningMetrics, compute_screening_metrics};

fn parse_screening_txns(data: &Bound<'_, PyList>) -> PyResult<Vec<ScreeningTxPoint>> {
    let mut txns: Vec<ScreeningTxPoint> = Vec::with_capacity(data.len());
    for item in data.iter() {
        let d = item.cast::<PyDict>()?;
        let time_obj = d.get_item("time")?.ok_or_else(|| pyo3::exceptions::PyValueError::new_err("Missing 'time'"))?;
        let time: chrono::NaiveTime = if let Ok(s) = time_obj.extract::<String>() {
            chrono::NaiveTime::parse_from_str(&s, "%H:%M:%S").map_err(|e| pyo3::exceptions::PyValueError::new_err(format!("Invalid time '{s}': {e}")))?
        } else if let Ok(t) = time_obj.extract::<chrono::NaiveTime>() { t }
        else { return Err(pyo3::exceptions::PyTypeError::new_err("'time' must be str or datetime.time")); };
        let price: f64 = d.get_item("price")?.ok_or_else(|| pyo3::exceptions::PyValueError::new_err("Missing 'price'"))?.extract()?;
        let vol: i64 = d.get_item("vol")?.ok_or_else(|| pyo3::exceptions::PyValueError::new_err("Missing 'vol'"))?.extract()?;
        let trade_count: i32 = d.get_item("trade_count").ok()
            .and_then(|v| v.and_then(|v| v.extract::<i32>().ok()))
            .unwrap_or(1);
        txns.push(ScreeningTxPoint { time, price, vol, trade_count });
    }
    Ok(txns)
}

fn screening_to_dict(py: Python<'_>, r: ScreeningMetrics) -> Py<PyAny> {
    let d = PyDict::new(py);
    d.set_item("first30_vol_pct", r.first30_vol_pct).unwrap();
    d.set_item("midday_vol_pct", r.midday_vol_pct).unwrap();
    d.set_item("afternoon_vol_pct", r.afternoon_vol_pct).unwrap();
    d.set_item("last30_vol_pct", r.last30_vol_pct).unwrap();
    d.set_item("last5_vol_pct", r.last5_vol_pct).unwrap();
    d.set_item("large_pct", r.large_pct).unwrap();
    d.set_item("xl_pct", r.xl_pct).unwrap();
    d.set_item("large_amount_pct", r.large_amount_pct).unwrap();
    d.set_item("xl_amount_pct", r.xl_amount_pct).unwrap();
    d.set_item("avg_trade_vol", r.avg_trade_vol).unwrap();
    d.set_item("vwap", r.vwap).unwrap();
    d.set_item("avg_shares_per_trade", r.avg_shares_per_trade).unwrap();
    d.set_item("single_trade_vol_pct", r.single_trade_vol_pct).unwrap();
    d.into_any().into()
}

#[pyfunction(name = "screening_metrics")]
#[pyo3(signature = (data))]
fn screening_metrics_fn(py: Python<'_>, data: &Bound<'_, PyList>) -> PyResult<Py<PyAny>> {
    let txns = parse_screening_txns(data)?;
    match compute_screening_metrics(&txns) {
        None => Ok(py.None()),
        Some(r) => Ok(screening_to_dict(py, r)),
    }
}

pub fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(screening_metrics_fn, m)?)?;
    Ok(())
}
