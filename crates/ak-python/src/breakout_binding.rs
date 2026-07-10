use pyo3::prelude::*;
use pyo3::types::{PyAny, PyDict, PyList};
use ak_core::breakout_analysis::{analyze_breakout as ak_analyze, TransactionPoint};

#[pyfunction]
#[pyo3(signature = (data, breakout_price))]
fn analyze_breakout(
    py: Python<'_>,
    data: &Bound<'_, PyList>,
    breakout_price: f64,
) -> PyResult<Py<PyAny>> {
    let mut txns: Vec<TransactionPoint> = Vec::with_capacity(data.len());
    for item in data.iter() {
        let d = item.cast::<PyDict>()?;
        let time_obj = d.get_item("time")?.ok_or_else(|| pyo3::exceptions::PyValueError::new_err("Missing 'time'"))?;
        let time: chrono::NaiveTime = if let Ok(s) = time_obj.extract::<String>() {
            chrono::NaiveTime::parse_from_str(&s, "%H:%M:%S").map_err(|e| pyo3::exceptions::PyValueError::new_err(format!("Invalid time '{s}': {e}")))?
        } else if let Ok(t) = time_obj.extract::<chrono::NaiveTime>() { t }
        else { return Err(pyo3::exceptions::PyTypeError::new_err("'time' must be str or datetime.time")); };
        let price: f64 = d.get_item("price")?.ok_or_else(|| pyo3::exceptions::PyValueError::new_err("Missing 'price'"))?.extract()?;
        let vol: i64 = d.get_item("vol")?.ok_or_else(|| pyo3::exceptions::PyValueError::new_err("Missing 'vol'"))?.extract()?;
        let trade_count: i32 = d.get_item("trade_count")?.ok_or_else(|| pyo3::exceptions::PyValueError::new_err("Missing 'trade_count'"))?.extract()?;
        let bs_flag: i32 = d.get_item("bs_flag")?.ok_or_else(|| pyo3::exceptions::PyValueError::new_err("Missing 'bs_flag'"))?.extract()?;
        txns.push(TransactionPoint { time, price, vol, trade_count, bs_flag });
    }

    match ak_analyze(&txns, breakout_price) {
        None => Ok(py.None()),
        Some(r) => {
            let d = PyDict::new(py);
            d.set_item("total_n", r.total_n)?;
            d.set_item("breakout_n", r.breakout_n)?;
            d.set_item("breakout_vol_ratio", r.breakout_vol_ratio)?;
            d.set_item("breakout_buy_ratio", r.breakout_buy_ratio)?;
            d.set_item("breakout_large_buy", r.breakout_large_buy)?;
            d.set_item("breakout_large_sell", r.breakout_large_sell)?;
            d.set_item("price_sustain_ratio", r.price_sustain_ratio)?;
            d.set_item("is_strong", r.is_strong)?;
            d.set_item("classification", &r.classification)?;
            d.set_item("confidence", r.confidence)?;
            d.set_item("details", &r.details)?;
            Ok(d.into_any().into())
        }
    }
}

pub fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(analyze_breakout, m)?)?;
    Ok(())
}
