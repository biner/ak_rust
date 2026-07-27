use talib_rs::momentum;

#[derive(Clone)]
pub struct DmiSignal {
    pub v1: &'static str,
    pub v2: &'static str,
    pub v3: &'static str,
}

fn dmi_at(_high: &[f64], _low: &[f64], _close: &[f64], plus_di: &[f64], minus_di: &[f64], adx: &[f64], i: usize) -> Option<DmiSignal> {
    let prev = i.saturating_sub(1);
    if prev == 0 || adx[i].is_nan() || adx[prev].is_nan()
        || plus_di[i].is_nan() || minus_di[i].is_nan()
    {
        return None;
    }
    let v1 = if adx[i] < 20.0 { "盘整" }
    else if adx[i] < 25.0 { "酝酿" }
    else if adx[i] < 40.0 { "趋势" }
    else { "强趋势" };

    let di_diff = plus_di[i] - minus_di[i];
    let v2 = if di_diff > 10.0 { "多头强势" }
    else if di_diff > 5.0 { "多头" }
    else if di_diff < -10.0 { "空头强势" }
    else if di_diff < -5.0 { "空头" }
    else { "缠绕" };

    let lb = 3.min(i.saturating_sub(1));
    let v3 = if !adx[i - lb].is_nan() {
        let slope = adx[i] - adx[i - lb];
        if slope > 0.5 { "上升" } else if slope < -0.5 { "下降" } else { "平稳" }
    } else { "其他" };

    Some(DmiSignal { v1, v2, v3 })
}

pub fn compute_dmi_series(high: &[f64], low: &[f64], close: &[f64], n1: usize, n2: usize) -> Vec<Option<DmiSignal>> {
    let min_bars = 3 * n1.max(n2) + 5;
    if close.len() < min_bars {
        return (0..close.len()).map(|_| None).collect();
    }

    let plus_di = match momentum::plus_di(high, low, close, n1) {
        Ok(v) => v, _ => return (0..close.len()).map(|_| None).collect(),
    };
    let minus_di = match momentum::minus_di(high, low, close, n1) {
        Ok(v) => v, _ => return (0..close.len()).map(|_| None).collect(),
    };

    let adx = if n1 == n2 {
        match momentum::adx(high, low, close, n2) {
            Ok(v) => v, _ => return (0..close.len()).map(|_| None).collect(),
        }
    } else {
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

    (0..close.len()).map(|i| dmi_at(high, low, close, &plus_di, &minus_di, &adx, i)).collect()
}

pub fn compute_dmi(high: &[f64], low: &[f64], close: &[f64], n1: usize, n2: usize, di: usize) -> Option<DmiSignal> {
    let series = compute_dmi_series(high, low, close, n1, n2);
    let len = series.len();
    if len < di + 1 { return None; }
    series[len - di].clone()
}
