//! ak_dmi_state_V260714 — DMI 趋势/方向/动量三维状态信号
//!
//! v1: 盘整/酝酿/趋势/强趋势（ADX）
//! v2: 多头强势/多头/缠绕/空头/空头强势（±DI差值）
//! v3: 上升/下降/平稳（ADX斜率）

use talib_rs::momentum;

pub struct DmiSignal {
    pub v1: &'static str,
    pub v2: &'static str,
    pub v3: &'static str,
}

pub fn compute_dmi(high: &[f64], low: &[f64], close: &[f64], n1: usize, n2: usize, di: usize) -> Option<DmiSignal> {
    let min_bars = 3 * n1.max(n2) + 5;
    if close.len() < min_bars || close.len() < di + 1 {
        return None;
    }

    let plus_di = momentum::plus_di(high, low, close, n1).ok()?;
    let minus_di = momentum::minus_di(high, low, close, n1).ok()?;

    let adx = if n1 == n2 {
        momentum::adx(high, low, close, n2).ok()?
    } else {
        // 自定义 ADX 计算（n1 ≠ n2 时）
        let n = close.len();
        let mut dx = vec![f64::NAN; n];
        for i in 0..n {
            let s = plus_di[i] + minus_di[i];
            if s > 0.0 && !plus_di[i].is_nan() && !minus_di[i].is_nan() {
                dx[i] = (minus_di[i] - plus_di[i]).abs() / s * 100.0;
            }
        }
        let mut adx = vec![f64::NAN; n];
        let fv = dx.iter().position(|v| !v.is_nan()).unwrap_or(n);
        if fv + n2 <= n {
            let mut s = 0.0;
            for i in fv..fv + n2 { s += dx[i]; }
            adx[fv + n2 - 1] = s / n2 as f64;
            for i in (fv + n2)..n {
                s += dx[i] - dx[i - n2];
                adx[i] = s / n2 as f64;
            }
        }
        adx
    };

    let n = close.len();
    let cur = n - di;
    let prev = cur - 1;

    if cur == 0 || adx[cur].is_nan() || adx[prev].is_nan()
        || plus_di[cur].is_nan() || minus_di[cur].is_nan()
    {
        return None;
    }

    let v1 = if adx[cur] < 20.0 { "盘整" }
    else if adx[cur] < 25.0 { "酝酿" }
    else if adx[cur] < 40.0 { "趋势" }
    else { "强趋势" };

    let di_diff = plus_di[cur] - minus_di[cur];
    let v2 = if di_diff > 10.0 { "多头强势" }
    else if di_diff > 5.0 { "多头" }
    else if di_diff < -10.0 { "空头强势" }
    else if di_diff < -5.0 { "空头" }
    else { "缠绕" };

    let lb = 3.min(cur.saturating_sub(1));
    let v3 = if !adx[cur - lb].is_nan() {
        let slope = adx[cur] - adx[cur - lb];
        if slope > 0.5 { "上升" } else if slope < -0.5 { "下降" } else { "平稳" }
    } else { "其他" };

    Some(DmiSignal { v1, v2, v3 })
}
