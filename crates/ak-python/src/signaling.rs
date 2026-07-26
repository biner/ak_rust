use pyo3::prelude::*;
use pyo3::types::{PyDict, PyList};
use std::collections::HashMap;

use ak_signals::{register_builtins, dispatch_signals, list_signal_names, BarPoint, SignalConfig};

#[pyfunction]
fn generate_ak_signals(py: Python<'_>, signals_config: &Bound<'_, PyList>, bars: &Bound<'_, PyList>) -> PyResult<Py<PyAny>> {
    // 确保信号已注册
    ak_signals::register_builtins();
    let sig_count = ak_signals::list_signal_names().len();
    eprintln!("[ak_python] dispatch: registry has {} signals", sig_count);
    // 解析 bars
    let mut bar_vec: Vec<BarPoint> = Vec::with_capacity(bars.len());
    for item in bars.iter() {
        let d = item.cast::<PyDict>()?;
        bar_vec.push(BarPoint {
            dt: d.get_item("dt").unwrap_or(None).and_then(|v| v.extract::<String>().ok()).unwrap_or_default(),
            open: d.get_item("open").unwrap_or(None).and_then(|v| v.extract::<f64>().ok()).unwrap_or(0.0),
            high: d.get_item("high").unwrap_or(None).and_then(|v| v.extract::<f64>().ok()).unwrap_or(0.0),
            low: d.get_item("low").unwrap_or(None).and_then(|v| v.extract::<f64>().ok()).unwrap_or(0.0),
            close: d.get_item("close").unwrap_or(None).and_then(|v| v.extract::<f64>().ok()).unwrap_or(0.0),
            vol: d.get_item("vol").unwrap_or(None).and_then(|v| v.extract::<f64>().ok()).unwrap_or(0.0),
            amount: d.get_item("amount").unwrap_or(None).and_then(|v| v.extract::<f64>().ok()).unwrap_or(0.0),
            freq: d.get_item("freq").unwrap_or(None).and_then(|v| v.extract::<String>().ok()).unwrap_or_default(),
        });
    }

    // 解析 signals_config
    let mut configs: Vec<SignalConfig> = Vec::with_capacity(signals_config.len());
    for item in signals_config.iter() {
        let d = item.cast::<PyDict>()?;
        let name: String = d.get_item("name")?.ok_or_else(|| pyo3::exceptions::PyValueError::new_err("Missing 'name'"))?.extract()?;
        let freq: String = d.get_item("freq").unwrap_or(None).and_then(|v| v.extract::<String>().ok()).unwrap_or_default();
        let mut params = HashMap::new();
        if let Ok(Some(p)) = d.get_item("params") {
            if let Ok(py_params) = p.extract::<HashMap<String, String>>() {
                params = py_params;
            }
        }
        configs.push(SignalConfig { name, freq, params });
    }

    let results = dispatch_signals(&configs, &bar_vec);

    // 构造 DataFrame 等价的 list[dict]
    let py_list = PyList::empty(py);
    for sig in &results {
        let d = PyDict::new(py);
        d.set_item("name", &sig.name).unwrap();
        d.set_item("freq", &sig.freq).unwrap();
        d.set_item("v1", &sig.v1).unwrap();
        d.set_item("v2", &sig.v2).unwrap();
        d.set_item("v3", &sig.v3).unwrap();
        py_list.append(d.into_any().into_any()).unwrap();
    }

    Ok(py_list.into_any().into())
}

#[pyfunction]
fn ak_signal_list() -> Vec<String> {
    register_builtins();
    list_signal_names()
}

pub fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    ak_signals::register_builtins();
    m.add_function(wrap_pyfunction!(generate_ak_signals, m)?)?;
    m.add_function(wrap_pyfunction!(ak_signal_list, m)?)?;
    Ok(())
}
