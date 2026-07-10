use pyo3::prelude::*;
use pyo3::types::{PyAny, PyDict, PyList};
use ak_core::tick_analysis::{analyze_tick_chart as ak_analyze_tick, TickPoint};

#[pyfunction]
#[pyo3(signature = (data, pre_close, low_price=None, close_pos_threshold=0.3))]
fn analyze_tick_chart(
    py: Python<'_>,
    data: &Bound<'_, PyList>,
    pre_close: f64,
    low_price: Option<f64>,
    close_pos_threshold: f64,
) -> PyResult<Py<PyAny>> {
    let mut ticks: Vec<TickPoint> = Vec::with_capacity(data.len());
    for item in data.iter() {
        let d = item.cast::<PyDict>()?;
        let time_obj = d.get_item("time")?.ok_or_else(|| {
            pyo3::exceptions::PyValueError::new_err("Missing 'time' field")
        })?;
        let time: chrono::NaiveTime = if let Ok(s) = time_obj.extract::<String>() {
            chrono::NaiveTime::parse_from_str(&s, "%H:%M:%S").map_err(|e| {
                pyo3::exceptions::PyValueError::new_err(format!("Invalid time string '{s}': {e}"))
            })?
        } else if let Ok(t) = time_obj.extract::<chrono::NaiveTime>() { t }
        else { return Err(pyo3::exceptions::PyTypeError::new_err("'time' must be str or datetime.time")); };
        let price: f64 = d.get_item("price")?.ok_or_else(|| pyo3::exceptions::PyValueError::new_err("Missing 'price'"))?.extract()?;
        let avg: f64 = d.get_item("avg").unwrap_or(None).map(|v| v.extract::<f64>()).transpose()?.unwrap_or(price);
        let vol: i64 = d.get_item("vol")?.ok_or_else(|| pyo3::exceptions::PyValueError::new_err("Missing 'vol'"))?.extract()?;
        let momentum: f64 = d.get_item("momentum").unwrap_or(None).map(|v| v.extract::<f64>()).transpose()?.unwrap_or(0.0);
        ticks.push(TickPoint { time, price, avg, vol, momentum });
    }

    match ak_analyze_tick(&ticks, pre_close, low_price, close_pos_threshold) {
        None => Ok(py.None()),
        Some(r) => {
            let d = PyDict::new(py);
            d.set_item("close_pos", r.close_pos)?;
            d.set_item("day_chg_pct", r.day_chg_pct)?;
            d.set_item("amplitude_pct", r.amplitude_pct)?;
            d.set_item("total_vol", r.total_vol)?;
            d.set_item("n", r.n)?;
            d.set_item("large_trade_count", r.large_trade_count)?;
            d.set_item("large_trade_vol_ratio", r.large_trade_vol_ratio)?;
            d.set_item("quality_score", r.quality_score)?;
            d.set_item("should_cancel", r.should_cancel)?;
            d.set_item("cancel_reason", &r.cancel_reason)?;
            Ok(d.into_any().into())
        }
    }
}

pub fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(analyze_tick_chart, m)?)?;
    Ok(())
}
