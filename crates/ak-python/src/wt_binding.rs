use pyo3::prelude::*;
use pyo3::types::PyDict;

use ak_signals::{compute_wavetrend, get_wt_signal, classify_wt_zone};

#[pyfunction(name = "compute_wavetrend")]
#[pyo3(signature = (prices, n1=10, n2=21, n3=4))]
fn compute_wavetrend_py(prices: Vec<f64>, n1: usize, n2: usize, n3: usize) -> Vec<Vec<f64>> {
    let wt = compute_wavetrend(&prices, n1, n2, n3);
    wt.into_iter().map(|p| vec![p.wt1, p.wt2]).collect()
}

#[pyfunction(name = "wt_classify_zone")]
fn wt_classify_zone_py(wt1: f64) -> &'static str {
    classify_wt_zone(wt1)
}

/// 信号级函数：传入价格序列和参数，返回信号字典。
/// 相当于 czsc 中 ak_wavetrend_V260625 的纯 Python 可用版。
#[pyfunction(name = "wt_signal")]
#[pyo3(signature = (prices, di=1, n1=10, n2=21, n3=4))]
fn wt_signal_py(py: Python<'_>, prices: Vec<f64>, di: usize, n1: usize, n2: usize, n3: usize) -> Py<PyAny> {
    let d = PyDict::new(py);
    let wt = compute_wavetrend(&prices, n1, n2, n3);
    match get_wt_signal(&wt, di) {
        Some(s) => {
            d.set_item("zone", s.zone).unwrap();
            d.set_item("direction", s.direction).unwrap();
            d.set_item("cross", s.cross).unwrap();
        }
        None => {
            d.set_item("zone", "其他").unwrap();
            d.set_item("direction", "任意").unwrap();
            d.set_item("cross", "其他").unwrap();
        }
    }
    d.into_any().into()
}

#[pyfunction(name = "wt_signal_batch")]
#[pyo3(signature = (prices, n1=10, n2=21, n3=4))]
fn wt_signal_batch_py(py: Python<'_>, prices: Vec<f64>, n1: usize, n2: usize, n3: usize) -> Vec<Py<PyAny>> {
    let wt = compute_wavetrend(&prices, n1, n2, n3);
    (0..wt.len())
        .map(|i| {
            let d = PyDict::new(py);
            if i == 0 {
                d.set_item("zone", "其他").unwrap();
                d.set_item("direction", "任意").unwrap();
                d.set_item("cross", "其他").unwrap();
            } else {
                let w1 = wt[i].wt1;
                let w2 = wt[i].wt2;
                let pw1 = wt[i - 1].wt1;
                let pw2 = wt[i - 1].wt2;

                if w1.is_nan() || w2.is_nan() || pw1.is_nan() || pw2.is_nan() {
                    d.set_item("zone", "其他").unwrap();
                    d.set_item("direction", "任意").unwrap();
                    d.set_item("cross", "其他").unwrap();
                } else {
                    d.set_item("zone", classify_wt_zone(w1)).unwrap();
                    let dir = if (w1 - w2) > (pw1 - pw2) { "增大" } else if (w1 - w2) < (pw1 - pw2) { "缩小" } else { "平稳" };
                    d.set_item("direction", dir).unwrap();
                    let cross = if pw1 <= pw2 && w1 > w2 { "金叉" } else if pw1 >= pw2 && w1 < w2 { "死叉" } else if w1 > w2 { "多头" } else { "空头" };
                    d.set_item("cross", cross).unwrap();
                }
            }
            d.into_any().into()
        })
        .collect()
}

pub fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(compute_wavetrend_py, m)?)?;
    m.add_function(wrap_pyfunction!(wt_classify_zone_py, m)?)?;
    m.add_function(wrap_pyfunction!(wt_signal_py, m)?)?;
    m.add_function(wrap_pyfunction!(wt_signal_batch_py, m)?)?;
    Ok(())
}
