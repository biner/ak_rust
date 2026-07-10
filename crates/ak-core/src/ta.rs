//! 基础技术分析算子（独立于 czsc）。

/// 指数移动平均（span = 2 / (n + 1)）
pub fn ema(values: &[f64], n: usize) -> Vec<f64> {
    if values.is_empty() || n == 0 {
        return values.to_vec();
    }
    let alpha = 2.0 / (n as f64 + 1.0);
    let mut result = Vec::with_capacity(values.len());
    for _ in 0..n.saturating_sub(1) {
        result.push(f64::NAN);
    }
    if values.len() >= n {
        let sma: f64 = values[..n].iter().sum::<f64>() / n as f64;
        result.push(sma);
    }
    for i in n..values.len() {
        let prev = result[i - 1];
        result.push(prev + alpha * (values[i] - prev));
    }
    // 补齐头部 NaN
    let fv = result.iter().find(|&&x| !x.is_nan()).copied().unwrap_or(0.0);
    let limit = n.saturating_sub(1).min(result.len());
    for v in &mut result[..limit] {
        if v.is_nan() {
            *v = fv;
        }
    }
    result
}

/// 简单移动平均
pub fn sma(values: &[f64], n: usize) -> Vec<f64> {
    if values.is_empty() || n == 0 {
        return values.to_vec();
    }
    let mut result = Vec::with_capacity(values.len());
    for i in 0..values.len() {
        if i + 1 < n {
            result.push(f64::NAN);
        } else {
            let sum: f64 = values[i + 1 - n..=i].iter().sum();
            result.push(sum / n as f64);
        }
    }
    let fv = result.iter().find(|&&x| !x.is_nan()).copied().unwrap_or(0.0);
    let limit = n.saturating_sub(1).min(result.len());
    for v in &mut result[..limit] {
        if v.is_nan() {
            *v = fv;
        }
    }
    result
}
