use crate::{BarPoint, SignalOutput};
use crate::wavetrend::fill_one_missing;
use std::collections::HashMap;

/// ak_vol_ratio_V250721：量比信号
pub fn compute_vol_ratio(bars: &[BarPoint], params: &HashMap<String, String>) -> Vec<SignalOutput> {
    let n = params.get("n").and_then(|s| s.parse::<usize>().ok()).unwrap_or(20);
    let freq = bars.first().map(|b| b.freq.clone()).unwrap_or_default();
    let name = "ak_vol_ratio_V260721";
    let vol: Vec<f64> = bars.iter().map(|b| b.vol).collect();
    let len = vol.len();
    let min_bars = n + 5;
    if len < min_bars { return fill_one_missing(len); }

    let mut out = Vec::with_capacity(len);
    for i in 0..len {
        if i < n - 1 {
            out.push(SignalOutput::with_score(name, &freq, "其他", "任意", "任意", 0));
            continue;
        }
        let cur_vol = vol[i];
        let vol_sum: f64 = vol[(i + 1 - n)..=i].iter().sum();
        let vol_ma = vol_sum / n as f64;
        if vol_ma == 0.0 {
            out.push(SignalOutput::with_score(name, &freq, "其他", "任意", "任意", 0));
            continue;
        }
        let ratio = cur_vol / vol_ma;
        let v1 = if ratio > 1.2 { "放量" } else if ratio < 0.8 { "缩量" } else { "平量" };
        out.push(SignalOutput::new(name, &freq, v1, "任意", "任意"));
    }
    out
}
