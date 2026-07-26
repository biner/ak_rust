//! WaveTrend 通道计算。
//!
//! 与 czsc 原版完全一致的算法：
//!   1. hlc3 = (high + low + close) / 3
//!   2. esa   = EMA(hlc3, n1)
//!   3. d     = EMA(|hlc3 - esa|, n1)
//!   4. ci    = (hlc3 - esa) / (0.015 * d)
//!   5. wt1   = EMA(ci, n2)
//!   6. wt2   = SMA(wt1, n3)

/// 辅助：计算 typical price = HLC/3
pub fn typical_price(h: f64, l: f64, c: f64) -> f64 {
    (h + l + c) / 3.0
}

/// EMA（nan-安全：跳过前导 NaN）
fn ema(values: &[f64], period: usize) -> Vec<f64> {
    if period == 0 || values.is_empty() { return values.to_vec(); }
    let k = 2.0 / (period as f64 + 1.0);
    let n = values.len();
    let mut result = vec![f64::NAN; n];
    let start = values.iter().position(|v| !v.is_nan()).unwrap_or(n);
    if start + period > n { return result; }
    let mut sum = 0.0;
    for i in start..start + period {
        sum += if values[i].is_nan() { 0.0 } else { values[i] };
    }
    result[start + period - 1] = sum / period as f64;
    for i in (start + period)..n {
        if values[i].is_nan() { continue; }
        result[i] = result[i - 1] + k * (values[i] - result[i - 1]);
    }
    let fv = result.iter().find(|&&v| !v.is_nan()).copied().unwrap_or(0.0);
    for v in &mut result { if v.is_nan() { *v = fv; } }
    result
}

/// SMA（跳过 NaN）
fn sma_skip_nan(values: &[f64], period: usize) -> Vec<f64> {
    let n = values.len();
    let mut result = vec![f64::NAN; n];
    for i in 0..n {
        let mut sum = 0.0; let mut cnt = 0;
        for j in (i + 1).saturating_sub(period)..=i {
            if !values[j].is_nan() { sum += values[j]; cnt += 1; }
        }
        if cnt >= period { result[i] = sum / period as f64; }
    }
    let fv = result.iter().find(|&&v| !v.is_nan()).copied().unwrap_or(0.0);
    for v in &mut result { if v.is_nan() { *v = fv; } }
    result
}

#[derive(Debug, Clone, Copy)]
pub struct WtPoint {
    pub wt1: f64,
    pub wt2: f64,
}

/// 计算 WaveTrend 通道（czsc 原版算法：HLC/3 + 双层 EMA）。
pub fn compute_wavetrend(high: &[f64], low: &[f64], close: &[f64], n1: usize, n2: usize, n3: usize) -> Vec<WtPoint> {
    let n = close.len();
    if n < n1 + n2 + n3 + 5 {
        return (0..n).map(|_| WtPoint { wt1: f64::NAN, wt2: f64::NAN }).collect();
    }
    let hlc3: Vec<f64> = (0..n).map(|i| typical_price(high[i], low[i], close[i])).collect();
    let esa = ema(&hlc3, n1);
    let d = ema(&hlc3.iter().zip(esa.iter()).map(|(h, e)| (h - e).abs()).collect::<Vec<_>>(), n1);
    let mut ci = vec![f64::NAN; n];
    for i in 0..n {
        if d[i].is_nan() { continue; }
        let denom = 0.015 * d[i];
        ci[i] = if denom.abs() < 1e-10 { 0.0 } else { (hlc3[i] - esa[i]) / denom };
    }
    let wt1 = ema(&ci, n2);
    let wt2 = sma_skip_nan(&wt1, n3);
    (0..n).map(|i| WtPoint { wt1: wt1[i], wt2: wt2[i] }).collect()
}

#[derive(Debug, Clone)]
pub struct WtSignal {
    pub zone: &'static str,
    pub direction: &'static str,
    pub cross: &'static str,
}

pub fn get_wt_signal(wt: &[WtPoint], di: usize) -> Option<WtSignal> {
    if wt.len() < di + 2 { return None; }
    let len = wt.len(); let cur = len - di; let prev = cur - 1;
    let (w1, w2, pw1, pw2) = (wt[cur].wt1, wt[cur].wt2, wt[prev].wt1, wt[prev].wt2);
    if w1.is_nan() || w2.is_nan() || pw1.is_nan() || pw2.is_nan() { return None; }
    let zone = classify_wt_zone(w1);
    let wtcd_cur = w1 - w2; let wtcd_prev = pw1 - pw2;
    let direction = if wtcd_cur > wtcd_prev { "增大" } else if wtcd_cur < wtcd_prev { "缩小" } else { "平稳" };
    let golden_cross = pw1 <= pw2 && w1 > w2;
    let death_cross = pw1 >= pw2 && w1 < w2;
    let cross = if golden_cross { "金叉" } else if death_cross { "死叉" } else if w1 > w2 { "多头" } else { "空头" };
    Some(WtSignal { zone, direction, cross })
}

pub fn classify_wt_zone(wt1: f64) -> &'static str {
    if wt1 > 100.0 { "超涨" } else if wt1 > 80.0 { "多头极强" } else if wt1 > 60.0 { "多头强势" }
    else if wt1 > 20.0 { "多头" } else if wt1 > 0.0 { "弱多" } else if wt1 > -20.0 { "弱空" }
    else if wt1 > -60.0 { "空头" } else if wt1 > -80.0 { "空头强势" } else if wt1 > -100.0 { "空头极强" }
    else { "超跌" }
}

/// ak_wavetrend_zone_V260724：区位（6级）+ 长期趋势 + 方向三维信号
/// v1: 极度超卖/超卖/中性0轴下/中性0轴上/超买/极度超买
/// v2: 长期看多/长期看空/其他（状态机）
/// v3: 多头/空头
pub fn classify_zone_v1(wt1: f64) -> &'static str {
    if wt1 > 60.0 { "极度超买" } else if wt1 > 53.0 { "超买" }
    else if wt1 > 0.0 { "中性0轴上" } else if wt1 > -53.0 { "中性0轴下" }
    else if wt1 > -60.0 { "超卖" } else { "极度超卖" }
}

/// 长期趋势状态机
pub fn compute_long_term_state(prev_state: i32, wt1_prev: f64, wt1_cur: f64) -> i32 {
    let falling_below_53 = wt1_prev > 53.0 && wt1_cur <= 53.0;
    let rising_above_neg53 = wt1_prev < -53.0 && wt1_cur >= -53.0;
    let below_neg53 = wt1_cur < -53.0;
    let above_53 = wt1_cur > 53.0;
    match prev_state {
        1 => { if below_neg53 { 0 } else if falling_below_53 { -1 } else { 1 } }
        -1 => { if above_53 { 0 } else if rising_above_neg53 { 1 } else { -1 } }
        _ => { if rising_above_neg53 { 1 } else if falling_below_53 { -1 } else { 0 } }
    }
}

/// 长趋势状态转字符串
pub fn long_term_state_str(state: i32) -> &'static str {
    match state { 1 => "长期看多", -1 => "长期看空", _ => "其他" }
}

/// Zone 信号：需要外部传入前一根状态
#[allow(clippy::too_many_arguments)]
pub fn compute_wt_zone_signal(
    wt1_cur: f64, wt2_cur: f64, wt1_prev: f64, prev_state: i32,
) -> (&'static str, &'static str, &'static str, i32, i32) {
    let v1 = classify_zone_v1(wt1_cur);
    let state = compute_long_term_state(prev_state, wt1_prev, wt1_cur);
    let v2 = long_term_state_str(state);
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
        ("极度超买", "长期看空", "空头") => 92, ("超买", "长期看空", "空头") => 85,
        ("中性0轴上", "长期看空", "空头") => 72, ("中性0轴下", "长期看空", "空头") => 70,
        ("超卖", "长期看空", "空头") => 60, ("极度超卖", "其他", "多头") => 50,
        ("超卖", "其他", "多头") => 45, ("极度超买", "其他", "空头") => 45,
        ("超买", "其他", "空头") => 40, ("极度超买", "其他", "多头") => 25,
        ("中性0轴下", "其他", "多头") => 57, ("中性0轴上", "其他", "多头") => 55,
        ("中性0轴下", "其他", "空头") => 37, ("中性0轴上", "其他", "空头") => 35,
        _ => 5,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_wavetrend_basic() {
        let p: Vec<f64> = (0..100).map(|i| 100.0 + i as f64 * 0.5).collect();
        let wt = compute_wavetrend(&p, &p, &p, 10, 21, 4);
        assert_eq!(wt.len(), p.len());
        assert!(!wt.last().unwrap().wt1.is_nan());
        assert!(!wt.last().unwrap().wt2.is_nan());
    }

    #[test]
    fn test_insufficient_data() {
        let p = vec![100.0; 5];
        assert!(compute_wavetrend(&p, &p, &p, 10, 21, 4).last().unwrap().wt1.is_nan());
    }
}
