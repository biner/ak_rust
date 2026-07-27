use crate::{BarPoint, SignalOutput};
use std::collections::HashMap;

fn classify(adx: f64, plus_di: f64, minus_di: f64, prev_adx: f64) -> (&'static str, &'static str, &'static str) {
    let v1 = if adx < 20.0 { "盘整" }
        else if adx < 25.0 { "酝酿" }
        else if adx < 40.0 { "趋势" }
        else { "强趋势" };
    let di_diff = plus_di - minus_di;
    let v2 = if di_diff > 10.0 { "多头强势" }
        else if di_diff > 5.0 { "多头" }
        else if di_diff < -10.0 { "空头强势" }
        else if di_diff < -5.0 { "空头" }
        else { "缠绕" };
    let v3 = if !prev_adx.is_nan() {
        let slope = adx - prev_adx;
        if slope > 0.5 { "上升" } else if slope < -0.5 { "下降" } else { "平稳" }
    } else { "其他" };
    (v1, v2, v3)
}

pub fn compute_dmi_state(high: &[f64], low: &[f64], close: &[f64], dmi_period: usize, adxr_period: usize) -> Vec<Option<(&'static str, &'static str, &'static str)>> {
    let n = close.len();
    let min_bars = 3 * dmi_period.max(adxr_period) + 5;
    if n < min_bars { return vec![None; n]; }

    let plus_di = match talib_rs::momentum::plus_di(high, low, close, dmi_period) { Ok(v) => v, _ => return vec![None; n] };
    let minus_di = match talib_rs::momentum::minus_di(high, low, close, dmi_period) { Ok(v) => v, _ => return vec![None; n] };

    let (adx, adxr) = if dmi_period == adxr_period {
        let adx = match talib_rs::momentum::adx(high, low, close, adxr_period) { Ok(v) => v, _ => return vec![None; n] };
        let adxr = match talib_rs::momentum::adxr(high, low, close, adxr_period) { Ok(v) => v, _ => return vec![None; n] };
        (adx, adxr)
    } else {
        let mut dx = vec![f64::NAN; n];
        for i in 0..n {
            let s = plus_di[i] + minus_di[i];
            if s > 0.0 && !plus_di[i].is_nan() && !minus_di[i].is_nan() {
                dx[i] = (minus_di[i] - plus_di[i]).abs() / s * 100.0;
            }
        }
        let mut adx = vec![f64::NAN; n];
        let fv = dx.iter().position(|v| !v.is_nan()).unwrap_or(n);
        if fv + adxr_period <= n {
            let mut s = 0.0;
            for i in fv..fv + adxr_period { s += dx[i]; }
            adx[fv + adxr_period - 1] = s / adxr_period as f64;
            for i in (fv + adxr_period)..n { s += dx[i] - dx[i - adxr_period]; adx[i] = s / adxr_period as f64; }
        }
        let mut adxr = vec![f64::NAN; n];
        for i in adxr_period..n {
            if !adx[i].is_nan() && !adx[i - adxr_period].is_nan() {
                adxr[i] = (adx[i] + adx[i - adxr_period]) / 2.0;
            }
        }
        (adx, adxr)
    };

    (0..n).map(|i| {
        if i == 0 || adx[i].is_nan() || adxr[i].is_nan() || plus_di[i].is_nan() || minus_di[i].is_nan() || adx[i-1].is_nan() || adxr[i-1].is_nan() {
            None
        } else {
            let lb = 3.min(i);
            let pa = if lb > 0 { adx[i - lb] } else { f64::NAN };
            Some(classify(adx[i], plus_di[i], minus_di[i], pa))
        }
    }).collect()
}

pub fn compute_dmi_signal(bars: &[BarPoint], params: &HashMap<String, String>) -> Vec<SignalOutput> {
    let n1 = params.get("n1").and_then(|s| s.parse::<usize>().ok()).unwrap_or(14);
    let n2 = params.get("n2").and_then(|s| s.parse::<usize>().ok()).unwrap_or(6);
    let freq = bars.first().map(|b| b.freq.clone()).unwrap_or_default();
    let name = "ak_dmi_state_V260714";
    let high: Vec<f64> = bars.iter().map(|b| b.high).collect();
    let low: Vec<f64> = bars.iter().map(|b| b.low).collect();
    let close: Vec<f64> = bars.iter().map(|b| b.close).collect();
    let r = compute_dmi_state(&high, &low, &close, n1, n2);
    r.iter().map(|o| match o {
        Some((v1, v2, v3)) => SignalOutput::new(name, &freq, v1, v2, v3),
        None => SignalOutput::with_score(name, &freq, "其他", "任意", "任意", 0),
    }).collect()
}
