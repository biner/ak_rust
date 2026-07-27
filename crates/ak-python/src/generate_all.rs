use pyo3::prelude::*;
use pyo3::types::{PyDict, PyList};
use std::collections::HashMap;

use ak_signals::{dispatch_signals, register_builtins, has_signal, BarPoint, SignalConfig};

/// czsc 列名中的短名映射
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
        _ => base.to_string(),
    }
}

/// 为 ak_rust 信号构造 czsc 风格的列名: {freq}_D1{short}#{params}_{label}
fn ak_col_name(name: &str, freq: &str, params: &HashMap<String, String>) -> String {
    let short = czsc_short_name(name);

    let n1 = params.get("n1").map(|s| s.as_str()).unwrap_or("");
    let n2 = params.get("n2").map(|s| s.as_str()).unwrap_or("");
    let n3 = params.get("n3").map(|s| s.as_str()).unwrap_or("");

    let mut param_parts: Vec<&str> = Vec::new();
    for v in [n1, n2, n3] {
        if !v.is_empty() {
            param_parts.push(v);
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

/// 将 Python bar 列表解析为 Vec<BarPoint>（支持 RawBar 对象和 dict 两种格式）
fn parse_bars(bars: &Bound<'_, PyList>) -> PyResult<Vec<BarPoint>> {
    let mut bar_vec: Vec<BarPoint> = Vec::with_capacity(bars.len());
    for item in bars.iter() {
        // 先试 dict 格式
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
            // RawBar 对象：通过 getattr 提取字段
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

/// 从 Python dict 解析信号配置项
fn parse_config(
    item: &Bound<'_, PyAny>,
) -> PyResult<SignalConfig> {
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
    // 兼容顶层参数
    if params.is_empty() {
        for (key, val) in d.iter() {
            let k: String = key.extract::<String>().unwrap_or_default();
            if k != "name" && k != "freq" && k != "params" {
                // 值可能是 str/int/float，统一转字符串
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

/// 批量生成信号：ak_rust 负责自身信号，其余委托 czsc 处理。
///
/// 参数:
///   bars: 基础周期 K 线列表（dict 格式，需含 dt/open/high/low/close/vol/amount/freq）
///   signals_config: 信号配置列表
///   sdt: 信号开始计算日期，格式 "YYYYMMDD" 或 "YYYY-MM-DD"
///   init_n: 预热 K 线数量
///   df: 是否返回 DataFrame（默认 False 返回 list[dict]）
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

    // 注册 ak_rust 信号
    register_builtins();

    // 1. 拆分信号配置：ak_rust 信号 vs czsc 信号
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

    // 2. 构造 czsc 子配置列表
    let czsc_cfg = PyList::empty(py);
    for &i in &czsc_indices {
        let item = signals_config.get_item(i)?;
        czsc_cfg.append(item)?;
    }

    // 3. 调用 czsc 处理非 ak 信号
    //    bars 类型：dict 列表（Python 层已转 RawBar）
    let czsc_mod = py.import("czsc")?;
    let czsc_result = czsc_mod.call_method1(
        "generate_czsc_signals",
        (bars, czsc_cfg, sdt, init_n, df),
    )?;

    // 如果只有 czsc 信号，直接返回 czsc 结果
    if ak_indices.is_empty() {
        return Ok(czsc_result.into_any().unbind());
    }

    // 4. 计算 ak_rust 信号
    let bar_vec = parse_bars(bars)?;
    let ak_results = dispatch_signals(&ak_configs, &bar_vec);

    // 5. 合并结果：将 ak_rust 列追加到 czsc 结果
    let pandas = py.import("pandas")?;

    // czsc_result 可能是 DataFrame 或 list[dict]
    let sdf = if czsc_indices.is_empty() {
        // czsc 没有处理任何信号，从 bar_vec（已解析）创建基础 DataFrame
        let records = PyList::empty(py);
        for bar in &bar_vec {
            let rec = PyDict::new(py);
            rec.set_item("dt", &bar.dt)?;
            rec.set_item("close", bar.close)?;
            records.append(rec.into_any())?;
        }
        pandas.call_method1("DataFrame", (records,))?
    } else if df {
        // 已经是 DataFrame
        czsc_result.into_any().downcast_into::<PyAny>()?
    } else {
        // czsc 返回了 list[dict]，转 DataFrame（追加列后再决定是否转回 list）
        pandas.call_method1("DataFrame", (czsc_result,))?
    };

    // 6. 追加 ak_rust 列（逐 bar 信号值）
    if !ak_configs.is_empty() {
        let nbars = ak_results.len() / ak_configs.len();
        for (cfg_idx, cfg) in ak_configs.iter().enumerate() {
            let col_name = ak_col_name(&cfg.name, &cfg.freq, &cfg.params);
            let start = cfg_idx * nbars;
            let end = start + nbars;
            let values: Vec<String> = ak_results[start..end]
                .iter()
                .map(|r| format!("{}_{}_{}_{}", r.v1, r.v2, r.v3, r.score))
                .collect();
            let series = pandas.call_method1("Series", (values,))?;
            sdf.set_item(col_name, series)?;
        }
    }

    // 7. 返回
    if df {
        Ok(sdf.into_any().unbind())
    } else {
        // 转回 list[dict]
        let records = sdf.call_method0("to_dict")?;
        let records = records.call_method1("orient", ("records",))?;
        Ok(records.into_any().unbind())
    }
}

pub fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(generate_all_signals, m)?)?;
    Ok(())
}
