use pyo3::prelude::*;
use pyo3::types::{PyDict, PyList};

use ak_core::zs::{get_zs_seq, Bi, BiDirection};

/// 从笔列表识别中枢序列（Rust 原生实现，算法移植自 czsc）。
///
/// 参数:
///     bis: 笔字典列表，每项含:
///         - direction: "Up" / "Down"（或 "向上"/"向下"）
///         - high: 笔最高价
///         - low: 笔最低价
///         - sdt: 开始时间（字符串或 datetime，UTC）
///         - edt: 结束时间（字符串或 datetime，UTC）
///
/// 返回:
///     list[dict]: 中枢字典列表，每项含
///     zg/zd/zz/gg/dd（数值）、sdir/edir（"Up"/"Down"）、sdt/edt（时间字符串）。
#[pyfunction(name = "get_zs_seq")]
fn py_get_zs_seq(py: Python<'_>, bis: &Bound<'_, PyList>) -> PyResult<Py<PyAny>> {
    let mut bi_vec: Vec<Bi> = Vec::with_capacity(bis.len());
    for item in bis.iter() {
        let d = item.cast::<PyDict>()?;

        let dir_str: String = d
            .get_item("direction")?
            .ok_or_else(|| pyo3::exceptions::PyValueError::new_err("Missing 'direction'"))?
            .extract()?;
        let direction = match dir_str.as_str() {
            "Up" | "向上" => BiDirection::Up,
            "Down" | "向下" => BiDirection::Down,
            _ => {
                return Err(pyo3::exceptions::PyValueError::new_err(format!(
                    "Invalid direction '{dir_str}', expected Up/Down"
                )))
            }
        };
        let high: f64 = d
            .get_item("high")?
            .ok_or_else(|| pyo3::exceptions::PyValueError::new_err("Missing 'high'"))?
            .extract()?;
        let low: f64 = d
            .get_item("low")?
            .ok_or_else(|| pyo3::exceptions::PyValueError::new_err("Missing 'low'"))?
            .extract()?;

        // 时间：优先读 unix 秒（sdt_ts/edt_ts），否则尝试字符串解析
        let sdt = parse_ts(d, "sdt_ts", "sdt")?;
        let edt = parse_ts(d, "edt_ts", "edt")?;

        bi_vec.push(Bi { direction, high, low, sdt, edt });
    }

    let zs_list = get_zs_seq(&bi_vec);

    let py_list = PyList::empty(py);
    for z in &zs_list {
        let zd = PyDict::new(py);
        zd.set_item("zg", z.zg)?;
        zd.set_item("zd", z.zd)?;
        zd.set_item("zz", z.zz)?;
        zd.set_item("gg", z.gg)?;
        zd.set_item("dd", z.dd)?;
        zd.set_item("sdir", match z.sdir {
            BiDirection::Up => "Up",
            BiDirection::Down => "Down",
        })?;
        zd.set_item("edir", match z.edir {
            BiDirection::Up => "Up",
            BiDirection::Down => "Down",
        })?;
        zd.set_item("sdt", z.sdt.to_rfc3339())?;
        zd.set_item("edt", z.edt.to_rfc3339())?;
        // unix 秒（与输入 sdt_ts/edt_ts 同语义，供调用方直接使用避免时区二次转换）
        zd.set_item("sdt_ts", z.sdt.timestamp())?;
        zd.set_item("edt_ts", z.edt.timestamp())?;
        py_list.append(zd)?;
    }
    Ok(py_list.into_any().into())
}

/// 解析时间：优先 unix 秒字段（`{name}_ts`），否则解析字符串字段。
fn parse_ts(
    d: &Bound<'_, PyDict>,
    ts_key: &str,
    str_key: &str,
) -> PyResult<chrono::DateTime<chrono::Utc>> {
    use chrono::{DateTime, NaiveDateTime, Utc};

    // 1. unix 秒（i64 / f64）
    if let Ok(Some(v)) = d.get_item(ts_key) {
        if let Ok(secs) = v.extract::<i64>() {
            return Ok(DateTime::from_timestamp(secs, 0).unwrap_or_else(|| DateTime::<Utc>::UNIX_EPOCH));
        }
        if let Ok(secs_f) = v.extract::<f64>() {
            return Ok(DateTime::from_timestamp(secs_f as i64, 0)
                .unwrap_or_else(|| DateTime::<Utc>::UNIX_EPOCH));
        }
    }

    // 2. 字符串（ISO8601 / "%Y-%m-%d %H:%M:%S"）
    if let Ok(Some(v)) = d.get_item(str_key) {
        if let Ok(s) = v.extract::<String>() {
            if let Ok(naive) = NaiveDateTime::parse_from_str(&s, "%Y-%m-%d %H:%M:%S") {
                return Ok(DateTime::from_naive_utc_and_offset(naive, Utc));
            }
            if let Ok(dt) = DateTime::parse_from_rfc3339(&s) {
                return Ok(dt.with_timezone(&Utc));
            }
        }
    }

    Err(pyo3::exceptions::PyValueError::new_err(format!(
        "Missing or invalid time: '{ts_key}' (unix secs) / '{str_key}' (str)"
    )))
}

pub fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(py_get_zs_seq, m)?)?;
    Ok(())
}
