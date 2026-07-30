use pyo3::prelude::*;
use pyo3::types::{PyDict, PyList};
use std::collections::HashMap;

use ak_signals::{dispatch_signals, register_builtins, has_signal, BarPoint, SignalConfig};

fn czsc_short_name(name: &str) -> String {
    let base = name
        .strip_prefix("ak_")
        .unwrap_or(name)
        .split("_V")
        .next()
        .unwrap_or(name);
    match base {
        "wavetrend" => "WT".to_string(),
        "wavetrend_zone" => "WT".to_string(),
        "dmi_state" => "DMI".to_string(),
        "ic_state" => "IC".to_string(),
        "ttm_squeeze" => "TTM".to_string(),
        "osc_state" => "OSC".to_string(),
        "vol_ratio" => "VOL".to_string(),
        _ => base.to_string(),
    }
}

fn ak_col_name(name: &str, freq: &str, params: &HashMap<String, String>) -> String {
    let short = czsc_short_name(name);
    let mut param_parts: Vec<&str> = Vec::new();
    for i in 1..=6 {
        let key = format!("n{i}");
        if let Some(v) = params.get(&key) {
            param_parts.push(v.as_str());
        }
    }
    if param_parts.is_empty() {
        for key in ["n", "m", "scale"] {
            if let Some(v) = params.get(key) {
                param_parts.push(v.as_str());
            }
        }
    }
    let params_str = param_parts.join("#");
    let version = name.split("_V").nth(1).unwrap_or(name);
    let is_zone = name.contains("zone");
    let label = if is_zone {
        format!("区位V{}", version)
    } else {
        format!("V{}", version)
    };
    format!("{}_D1{}#{}_{}", freq, short, params_str, label)
}

fn parse_bars(bars: &Bound<'_, PyList>) -> PyResult<Vec<BarPoint>> {
    let mut bar_vec: Vec<BarPoint> = Vec::with_capacity(bars.len());
    for item in bars.iter() {
        if let Ok(d) = item.cast::<PyDict>() {
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
        } else {
            let bar = &item;
            bar_vec.push(BarPoint {
                dt: bar.getattr("dt").ok().and_then(|v| v.extract::<String>().ok()).unwrap_or_default(),
                open: bar.getattr("open").ok().and_then(|v| v.extract::<f64>().ok()).unwrap_or(0.0),
                high: bar.getattr("high").ok().and_then(|v| v.extract::<f64>().ok()).unwrap_or(0.0),
                low: bar.getattr("low").ok().and_then(|v| v.extract::<f64>().ok()).unwrap_or(0.0),
                close: bar.getattr("close").ok().and_then(|v| v.extract::<f64>().ok()).unwrap_or(0.0),
                vol: bar.getattr("vol").ok().and_then(|v| v.extract::<f64>().ok()).unwrap_or(0.0),
                amount: bar.getattr("amount").ok().and_then(|v| v.extract::<f64>().ok()).unwrap_or(0.0),
                freq: bar.getattr("freq").ok().and_then(|v| v.extract::<String>().ok()).unwrap_or_default(),
            });
        }
    }
    Ok(bar_vec)
}

fn parse_config(item: &Bound<'_, PyAny>) -> PyResult<SignalConfig> {
    let d = item.cast::<PyDict>()?;
    let name: String = d
        .get_item("name")?
        .ok_or_else(|| pyo3::exceptions::PyValueError::new_err("Missing 'name'"))?
        .extract()?;
    let freq: String = d
        .get_item("freq")
        .unwrap_or(None)
        .and_then(|v| v.extract::<String>().ok())
        .unwrap_or_default();
    let mut params = HashMap::new();
    if let Ok(Some(p)) = d.get_item("params") {
        if let Ok(py_params) = p.extract::<HashMap<String, String>>() {
            params = py_params;
        }
    }
    if params.is_empty() {
        for (key, val) in d.iter() {
            let k: String = key.extract::<String>().unwrap_or_default();
            if k != "name" && k != "freq" && k != "params" {
                let v_str = if let Ok(s) = val.extract::<String>() {
                    s
                } else if let Ok(n) = val.extract::<i64>() {
                    n.to_string()
                } else if let Ok(f) = val.extract::<f64>() {
                    f.to_string()
                } else {
                    continue;
                };
                params.insert(k, v_str);
            }
        }
    }
    Ok(SignalConfig { name, freq, params })
}

#[pyfunction]
#[pyo3(signature = (bars, signals_config, sdt="20170101", init_n=500, df=false))]
fn generate_all_signals(
    py: Python<'_>,
    bars: &Bound<'_, PyList>,
    signals_config: &Bound<'_, PyList>,
    sdt: &str,
    init_n: usize,
    df: bool,
) -> PyResult<Py<PyAny>> {
    if bars.is_empty() {
        return Err(pyo3::exceptions::PyValueError::new_err("bars 不能为空"));
    }

    register_builtins();

    let mut ak_indices: Vec<usize> = Vec::new();
    let mut czsc_indices: Vec<usize> = Vec::new();
    let mut ak_configs: Vec<SignalConfig> = Vec::new();

    for i in 0..signals_config.len() {
        let item = signals_config.get_item(i)?;
        let d = item.cast::<PyDict>()?;
        let name: String = d
            .get_item("name")?
            .ok_or_else(|| pyo3::exceptions::PyValueError::new_err("Missing 'name'"))?
            .extract()?;

        if has_signal(&name) {
            ak_indices.push(i);
            ak_configs.push(parse_config(&item)?);
        } else {
            czsc_indices.push(i);
        }
    }

    let czsc_cfg = PyList::empty(py);
    for &i in &czsc_indices {
        let item = signals_config.get_item(i)?;
        czsc_cfg.append(item)?;
    }

    let czsc_mod = py.import("czsc")?;
    let czsc_result = czsc_mod.call_method1(
        "generate_czsc_signals",
        (bars, czsc_cfg, sdt, init_n, df),
    )?;

    if ak_indices.is_empty() {
        return Ok(czsc_result.into_any().unbind());
    }

    let bar_vec = parse_bars(bars)?;
    for cfg in &mut ak_configs {
        cfg.params.insert("_init_n".into(), init_n.to_string());
    }
    let ak_results = dispatch_signals(&ak_configs, &bar_vec);

    let pandas = py.import("pandas")?;

    let sdf = if czsc_indices.is_empty() {
        let records = PyList::empty(py);
        for bar in &bar_vec {
            let rec = PyDict::new(py);
            rec.set_item("dt", &bar.dt)?;
            rec.set_item("close", bar.close)?;
            records.append(rec.into_any())?;
        }
        pandas.call_method1("DataFrame", (records,))?
    } else if df {
        czsc_result.into_any().cast_into::<PyAny>()?
    } else {
        pandas.call_method1("DataFrame", (czsc_result,))?
    };

    if !ak_configs.is_empty() {
        let nrows = sdf.call_method0("__len__")?.extract::<usize>()?;
        let nbars = ak_results.len() / ak_configs.len();
        let offset = nbars.saturating_sub(nrows);
        for (cfg_idx, cfg) in ak_configs.iter().enumerate() {
            let col_name = ak_col_name(&cfg.name, &cfg.freq, &cfg.params);
            let start = cfg_idx * nbars + offset;
            let end = start + nrows;
            let values: Vec<String> = ak_results[start..end]
                .iter()
                .map(|r| format!("{}_{}_{}_{}", r.v1, r.v2, r.v3, r.score))
                .collect();
            let series = pandas.call_method1("Series", (values,))?;
            sdf.set_item(col_name, series)?;
        }
    }

    if df {
        Ok(sdf.into_any().unbind())
    } else {
        let records = sdf.call_method0("to_dict")?;
        let records = records.call_method1("orient", ("records",))?;
        Ok(records.into_any().unbind())
    }
}

pub fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(generate_all_signals, m)?)?;
    Ok(())
}
