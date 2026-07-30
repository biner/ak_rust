use pyo3::prelude::*;
use pyo3::types::{PyAny, PyDict, PyList};
use ak_core::transaction::{analyze_transactions, TransactionPoint, TransactionAnalysisResult};

fn parse_txns(data: &Bound<'_, PyList>) -> PyResult<Vec<TransactionPoint>> {
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
    Ok(txns)
}

fn to_dict(py: Python<'_>, r: TransactionAnalysisResult) -> Py<PyAny> {
    let d = PyDict::new(py);
    d.set_item("buy_vol_ratio", r.buy_vol_ratio).unwrap();
    d.set_item("sell_vol_ratio", r.sell_vol_ratio).unwrap();
    d.set_item("neutral_vol_ratio", r.neutral_vol_ratio).unwrap();
    d.set_item("large_trade_count", r.large_trade_count).unwrap();
    d.set_item("large_trade_vol_ratio", r.large_trade_vol_ratio).unwrap();
    d.set_item("large_buy_count", r.large_buy_count).unwrap();
    d.set_item("large_sell_count", r.large_sell_count).unwrap();
    d.set_item("large_buy_vol_ratio", r.large_buy_vol_ratio).unwrap();
    d.set_item("inst_buy_vol_ratio", r.inst_buy_vol_ratio).unwrap();
    d.set_item("inst_buy_count", r.inst_buy_count).unwrap();
    d.set_item("inst_sell_count", r.inst_sell_count).unwrap();
    d.set_item("inst_net_flow_ratio", r.inst_net_flow_ratio).unwrap();
    d.set_item("split_clusters", r.split_clusters).unwrap();
    d.set_item("matched_pairs", r.matched_pairs).unwrap();
    d.set_item("price_position", r.price_position).unwrap();
    d.set_item("day_chg_pct", r.day_chg_pct).unwrap();
    d.set_item("last_30min_vol_ratio", r.last_30min_vol_ratio).unwrap();
    d.set_item("last_30min_buy_sell_ratio", r.last_30min_buy_sell_ratio).unwrap();
    d.set_item("last_30min_momentum", r.last_30min_momentum).unwrap();
    d.set_item("is_breakout", r.is_breakout).unwrap();
    d.set_item("confidence", r.confidence).unwrap();
    d.set_item("conclusion", &r.conclusion).unwrap();
    d.set_item("details", &r.details).unwrap();
    d.into_any().into()
}

macro_rules! make_tx_name_fn {
    ($name:ident, $ak_fn:path, $py_name:expr) => {
        #[pyfunction(name = $py_name)]
        #[pyo3(signature = (data, pre_close, high, low))]
        fn $name(py: Python<'_>, data: &Bound<'_, PyList>, pre_close: f64, high: f64, low: f64) -> PyResult<Py<PyAny>> {
            let txns = parse_txns(data)?;
            match $ak_fn(&txns, pre_close, high, low) {
                None => Ok(py.None()),
                Some(r) => Ok(to_dict(py, r)),
            }
        }
    };
}

make_tx_name_fn!(trans_analysis, analyze_transactions, "trans_analysis");

pub fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(trans_analysis, m)?)?;
    Ok(())
}
