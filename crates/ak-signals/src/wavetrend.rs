use crate::{BarPoint, SignalOutput};
use std::collections::HashMap;

pub struct WtPoint {
    pub wt1: f64,
    pub wt2: f64,
}

/// EMA（CZSC 版）：seed = series[0]，每步四舍五入到 4 位小数
fn calc_ema(series: &[f64], n: usize) -> Vec<f64> {
    let len = series.len();
    let mut out = vec![f64::NAN; len];
    if len == 0 || n == 0 {
        return out;
    }
    out[0] = series[0];
    for i in 1..len {
        let ema = (2.0 * series[i] + out[i - 1] * (n as f64 - 1.0)) / (n as f64 + 1.0);
        out[i] = (ema * 10_000.0).round() / 10_000.0;
    }
    out
}

/// SMA（CZSC 版）：不足窗口期用 expanding mean，每步四舍五入到 4 位小数
fn calc_sma(series: &[f64], n: usize) -> Vec<f64> {
    let len = series.len();
    let mut out = vec![f64::NAN; len];
    if len == 0 || n == 0 {
        return out;
    }
    for i in 0..len {
        let start = if i < n { 0 } else { i + 1 - n };
        let window = &series[start..=i];
        let mean = window.iter().sum::<f64>() / window.len() as f64;
        out[i] = (mean * 10_000.0).round() / 10_000.0;
    }
    out
}

/// WaveTrend 全量计算（CZSC 原版算法）
pub fn compute_wavetrend(high: &[f64], low: &[f64], close: &[f64], n1: usize, n2: usize, n3: usize) -> Vec<WtPoint> {
    let n = close.len();
    let hlc3: Vec<f64> = (0..n).map(|i| (high[i] + low[i] + close[i]) / 3.0).collect();
    let esa = calc_ema(&hlc3, n1);
    let diff: Vec<f64> = hlc3.iter().zip(esa.iter()).map(|(h, e)| (h - e).abs()).collect();
    let d = calc_ema(&diff, n1);
    let ci: Vec<f64> = hlc3.iter().zip(esa.iter().zip(d.iter())).map(|(h, (e, dv))| {
        let denom = 0.015 * dv;
        if denom.abs() < 1e-10 { 0.0 } else { (h - e) / denom }
    }).collect();
    let wt1 = calc_ema(&ci, n2);
    let wt2 = calc_sma(&wt1, n3);
    (0..n).map(|i| WtPoint { wt1: wt1[i], wt2: wt2[i] }).collect()
}

/// WaveTrend 8 级区位分类（CZSC 原版阈值）
pub fn classify_wt_zone(wt1: f64) -> &'static str {
    if wt1 > 60.0 { "极度超买" }
    else if wt1 > 53.0 { "超买" }
    else if wt1 > 30.0 { "偏多" }
    else if wt1 >= 0.0 { "弱多" }
    else if wt1 >= -30.0 { "弱空" }
    else if wt1 >= -53.0 { "偏空" }
    else if wt1 >= -60.0 { "超卖" }
    else { "极度超卖" }
}

pub struct WtSignal {
    pub zone: &'static str,
    pub direction: &'static str,
    pub cross: &'static str,
}

pub fn get_wt_signal(wt: &[WtPoint], di: usize) -> Option<WtSignal> {
    if wt.len() < di + 2 { return None; }
    let len = wt.len();
    let cur = len - di;
    let prev = cur - 1;
    let (w1, w2, pw1, pw2) = (wt[cur].wt1, wt[cur].wt2, wt[prev].wt1, wt[prev].wt2);
    if w1.is_nan() || w2.is_nan() || pw1.is_nan() || pw2.is_nan() { return None; }
    let zone = classify_wt_zone(w1);
    let wtcd_cur = w1 - w2;
    let wtcd_prev = pw1 - pw2;
    let direction = if wtcd_cur > wtcd_prev { "增大" } else if wtcd_cur < wtcd_prev { "缩小" } else { "平稳" };
    let golden_cross = pw1 <= pw2 && w1 > w2;
    let death_cross = pw1 >= pw2 && w1 < w2;
    let cross = if golden_cross { "金叉" } else if death_cross { "死叉" } else if w1 > w2 { "多头" } else { "空头" };
    Some(WtSignal { zone, direction, cross })
}

/// 长期趋势状态机（CZSC 原版）
pub fn compute_long_term_state(prev_state: i32, wt1_prev: f64, wt1_cur: f64) -> i32 {
    let is_falling_below_53 = wt1_prev > 53.0 && wt1_cur <= 53.0;
    let is_rising_above_neg53 = wt1_prev < -53.0 && wt1_cur >= -53.0;
    let is_below_neg53 = wt1_cur < -53.0;
    let is_above_53 = wt1_cur > 53.0;
    match prev_state {
        1 => { if is_below_neg53 { 0 } else if is_falling_below_53 { -1 } else { 1 } }
        -1 => { if is_above_53 { 0 } else if is_rising_above_neg53 { 1 } else { -1 } }
        _ => { if is_rising_above_neg53 { 1 } else if is_falling_below_53 { -1 } else { 0 } }
    }
}

/// Zone 信号计算（CZSC 原版）
#[allow(clippy::too_many_arguments)]
pub fn compute_wt_zone_signal(
    wt1_cur: f64, wt2_cur: f64, wt1_prev: f64, prev_state: i32,
) -> (&'static str, &'static str, &'static str, i32, i32) {
    let v1 = classify_wt_zone(wt1_cur);
    let state = compute_long_term_state(prev_state, wt1_prev, wt1_cur);
    let v2 = match state { 1 => "长期看多", -1 => "长期看空", _ => "其他" };
    let v3 = if wt1_cur > wt2_cur { "多头" } else { "空头" };
    let score = wt_zone_score(v1, v2, v3);
    (v1, v2, v3, state, score)
}

fn wt_zone_score(v1: &str, v2: &str, v3: &str) -> i32 {
    match (v1, v2, v3) {
        ("极度超卖", "长期看多", "多头") => 95, ("超卖", "长期看多", "多头") => 90,
        ("中性0轴下", "长期看多", "多头") => 80, ("中性0轴上", "长期看多", "多头") => 75,
        ("超买", "长期看多", "多头") => 65, ("极度超卖", "长期看多", "空头") => 50,
        ("超卖", "长期看多", "空头") => 45, ("中性0轴下", "长期看多", "空头") => 42,
        ("中性0轴上", "长期看多", "空头") => 40,
        ("极度超买", "长期看空", "空头") => 92, ("超买", "长期看空", "空头") => 85,
        ("中性0轴上", "长期看空", "空头") => 72, ("中性0轴下", "长期看空", "空头") => 70,
        ("超卖", "长期看空", "空头") => 60, ("极度超买", "长期看空", "多头") => 45,
        ("超买", "长期看空", "多头") => 35, ("中性0轴上", "长期看空", "多头") => 32,
        ("中性0轴下", "长期看空", "多头") => 30,
        ("极度超卖", "其他", "多头") => 50, ("超卖", "其他", "多头") => 45,
        ("极度超卖", "其他", "空头") => 35, ("超卖", "其他", "空头") => 30,
        ("极度超买", "其他", "空头") => 45, ("超买", "其他", "空头") => 40,
        ("极度超买", "其他", "多头") => 25, ("超买", "其他", "多头") => 20,
        ("中性0轴下", "其他", "多头") => 57, ("中性0轴上", "其他", "多头") => 55,
        ("中性0轴下", "其他", "空头") => 37, ("中性0轴上", "其他", "空头") => 35,
        _ => 5,
    }
}

pub(crate) fn fill_one_missing(n: usize) -> Vec<SignalOutput> {
    (0..n).map(|_| SignalOutput::with_score("", "", "其他", "任意", "任意", 1)).collect()
}

fn wt_signal_at(wt: &[WtPoint], freq: &str, i: usize, min_bars: usize, name: &str) -> SignalOutput {
    if i + 1 < min_bars {
        return SignalOutput::with_score(name, freq, "其他", "任意", "任意", 1);
    }
    match get_wt_signal(&wt[..=i], 1) {
        Some(s) => SignalOutput::new(name, freq, s.zone, s.direction, s.cross),
        None => SignalOutput::with_score(name, freq, "其他", "任意", "任意", 1),
    }
}

pub fn compute_wt_signal(bars: &[BarPoint], params: &HashMap<String, String>) -> Vec<SignalOutput> {
    let n1 = params.get("n1").and_then(|s| s.parse::<usize>().ok()).unwrap_or(10);
    let n2 = params.get("n2").and_then(|s| s.parse::<usize>().ok()).unwrap_or(21);
    let n3 = params.get("n3").and_then(|s| s.parse::<usize>().ok()).unwrap_or(4);
    let freq = bars.first().map(|b| b.freq.clone()).unwrap_or_default();
    let name = "ak_wavetrend_V260625";
    let min_bars = n1 + n2 + n3 + 5;
    let high: Vec<f64> = bars.iter().map(|b| b.high).collect();
    let low: Vec<f64> = bars.iter().map(|b| b.low).collect();
    let close: Vec<f64> = bars.iter().map(|b| b.close).collect();
    if close.len() < min_bars { return fill_one_missing(close.len()); }
    let wt = compute_wavetrend(&high, &low, &close, n1, n2, n3);
    (0..close.len()).map(|i| wt_signal_at(&wt, &freq, i, min_bars, name)).collect()
}

pub fn compute_wt_zone_signal_full(bars: &[BarPoint], params: &HashMap<String, String>) -> Vec<SignalOutput> {
    let n1 = params.get("n1").and_then(|s| s.parse::<usize>().ok()).unwrap_or(10);
    let n2 = params.get("n2").and_then(|s| s.parse::<usize>().ok()).unwrap_or(21);
    let n3 = params.get("n3").and_then(|s| s.parse::<usize>().ok()).unwrap_or(4);
    let init_n = params.get("_init_n").and_then(|s| s.parse::<usize>().ok()).unwrap_or(0);
    let min_bars = n1 + n2 + n3 + 5;
    let freq = bars.first().map(|b| b.freq.clone()).unwrap_or_default();
    let name = "ak_wavetrend_zone_V260724";
    let high: Vec<f64> = bars.iter().map(|b| b.high).collect();
    let low: Vec<f64> = bars.iter().map(|b| b.low).collect();
    let close: Vec<f64> = bars.iter().map(|b| b.close).collect();
    if close.len() < min_bars { return fill_one_missing(close.len()); }
    let wt = compute_wavetrend(&high, &low, &close, n1, n2, n3);

    // CZSC 在 init_n 处重置状态机：state 从 0 开始，bar init_n 是第 1 根输出
    let start = init_n.max(1);
    let mut st = 0i32;
    let mut out = Vec::with_capacity(close.len());
    for _i in 0..start {
        out.push(SignalOutput::with_score(name, &freq, "其他", "任意", "任意", 1));
    }
    for i in start..close.len() {
        if wt[i].wt1.is_nan() || wt[i].wt2.is_nan() || wt[i-1].wt1.is_nan() {
            out.push(SignalOutput::with_score(name, &freq, "其他", "任意", "任意", 1));
            continue;
        }
        let (v1, v2, v3, new_state, score) = compute_wt_zone_signal(wt[i].wt1, wt[i].wt2, wt[i-1].wt1, st);
        st = new_state;
        out.push(SignalOutput::with_score(name, &freq, v1, v2, v3, score));
    }
    out
}
