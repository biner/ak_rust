use crate::{BarPoint, SignalOutput};
use crate::wavetrend::fill_one_missing;
use std::collections::HashMap;

fn calc_ema(series: &[f64], n: usize) -> Vec<f64> {
    let len = series.len();
    let mut out = vec![f64::NAN; len];
    if len == 0 || n == 0 { return out; }
    out[0] = series[0];
    for i in 1..len {
        out[i] = (2.0 * series[i] + out[i - 1] * (n as f64 - 1.0)) / (n as f64 + 1.0);
    }
    for v in &mut out { *v = (*v * 10_000.0).round() / 10_000.0; }
    out
}

/// ak_osc_state_V250720：OSC 动量方向/温度/加速度三维状态信号
pub fn compute_osc_720(bars: &[BarPoint], params: &HashMap<String, String>) -> Vec<SignalOutput> {
    let n = params.get("n").and_then(|s| s.parse::<usize>().ok()).unwrap_or(20);
    let m = params.get("m").and_then(|s| s.parse::<usize>().ok()).unwrap_or(6);
    let freq = bars.first().map(|b| b.freq.clone()).unwrap_or_default();
    let name = "ak_osc_state_V260720";
    let close: Vec<f64> = bars.iter().map(|b| b.close).collect();
    let len = close.len();
    let min_bars = n.max(m) + 5;
    if len < min_bars { return fill_one_missing(len); }

    let mut osc = vec![f64::NAN; len];
    for i in (n - 1)..len {
        let sum: f64 = close[(i + 1 - n)..=i].iter().sum();
        osc[i] = (close[i] - sum / n as f64) * 100.0;
    }

    let first_valid = osc.iter().position(|v| !v.is_nan()).unwrap_or(len);
    let mut oscema = vec![f64::NAN; len];
    if first_valid + m <= len {
        let ema_part = calc_ema(&osc[first_valid..], m);
        for (i, &v) in ema_part.iter().enumerate() {
            oscema[first_valid + i] = v;
        }
    }

    let mut out = Vec::with_capacity(len);
    for i in 0..len {
        if i < n - 1 || osc[i].is_nan() || oscema[i].is_nan() {
            out.push(SignalOutput::with_score(name, &freq, "其他", "任意", "任意", 0));
            continue;
        }
        let v1 = if osc[i] > 0.0 { "上" } else if osc[i] < 0.0 { "下" } else { "零" };
        let osc_abs = osc[i].abs();
        let v2 = if osc[i] < 0.0 { "极冷" }
            else if osc_abs < 50.0 { "偏低" }
            else if osc_abs < 200.0 { "温和" }
            else if osc_abs < 350.0 { "偏高" }
            else { "过热" };
        let v3 = if osc[i] > oscema[i] { "上升" } else if osc[i] < oscema[i] { "下降" } else { "平稳" };
        out.push(SignalOutput::new(name, &freq, v1, v2, v3));
    }
    out
}

/// ak_osc_state_V250721：OSC 百分比偏离温度版
pub fn compute_osc_721(bars: &[BarPoint], params: &HashMap<String, String>) -> Vec<SignalOutput> {
    let n = params.get("n").and_then(|s| s.parse::<usize>().ok()).unwrap_or(20);
    let m = params.get("m").and_then(|s| s.parse::<usize>().ok()).unwrap_or(6);
    let scale = params.get("scale").and_then(|s| s.parse::<f64>().ok()).unwrap_or(1.0);
    let freq = bars.first().map(|b| b.freq.clone()).unwrap_or_default();
    let name = "ak_osc_state_V260721";
    let close: Vec<f64> = bars.iter().map(|b| b.close).collect();
    let len = close.len();
    let min_bars = n.max(m) + 5;
    if len < min_bars { return fill_one_missing(len); }

    let mut osc = vec![f64::NAN; len];
    let mut ma = vec![f64::NAN; len];
    for i in (n - 1)..len {
        let sum: f64 = close[(i + 1 - n)..=i].iter().sum();
        let ma_val = sum / n as f64;
        ma[i] = ma_val;
        osc[i] = (close[i] - ma_val) * 100.0;
    }

    let first_valid = osc.iter().position(|v| !v.is_nan()).unwrap_or(len);
    let mut oscema = vec![f64::NAN; len];
    if first_valid + m <= len {
        let ema_part = calc_ema(&osc[first_valid..], m);
        for (i, &v) in ema_part.iter().enumerate() {
            oscema[first_valid + i] = v;
        }
    }

    let mut out = Vec::with_capacity(len);
    for i in 0..len {
        if i < n - 1 || osc[i].is_nan() || oscema[i].is_nan() || ma[i].is_nan() {
            out.push(SignalOutput::with_score(name, &freq, "其他", "任意", "任意", 0));
            continue;
        }
        let v1 = if osc[i] > 0.0 { "上" } else if osc[i] < 0.0 { "下" } else { "零" };
        let pct = if ma[i].abs() > f64::EPSILON { (close[i] - ma[i]) / ma[i] * 100.0 } else { 0.0 };
        let t1 = 3.0 * scale;
        let t2 = 10.0 * scale;
        let v2 = if pct < -t2 { "极度冷" } else if pct < -t1 { "偏冷" } else if pct <= t1 { "适中" } else if pct <= t2 { "偏热" } else { "极度热" };
        let v3 = if osc[i] > oscema[i] { "上升" } else if osc[i] < oscema[i] { "下降" } else { "平稳" };
        out.push(SignalOutput::new(name, &freq, v1, v2, v3));
    }
    out
}
