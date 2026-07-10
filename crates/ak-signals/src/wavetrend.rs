//! WaveTrend 通道计算（无 czsc 依赖）。
//!
//! 输入：收盘价序列
//! 输出：(wt1, wt2) 序列，每个元素对应一根 K 线
//!
//! Python 侧根据 wt1/wt2 做金叉/死叉/区位判断后格式化为信号字符串。

use talib_rs::overlap::{ema as talib_ema, sma as talib_sma};

/// WT 分析结果单条
#[derive(Debug, Clone, Copy)]
pub struct WtPoint {
    pub wt1: f64,
    pub wt2: f64,
}

/// 计算 WaveTrend 通道。
///
/// # 参数
/// - `prices`: 收盘价序列（按时间升序）
/// - `n1`: Channel EMA 周期（默认 10）
/// - `n2`: Average EMA 周期（默认 21）
/// - `n3`: Signal SMA 周期（默认 4）
///
/// # 返回
/// `Vec<WtPoint>`，长度等于 `prices.len()`，不足周期时返回 `WtPoint { wt1: NaN, wt2: NaN }`
pub fn compute_wavetrend(prices: &[f64], n1: usize, n2: usize, n3: usize) -> Vec<WtPoint> {
    if prices.len() < n1 + n2 + n3 + 5 {
        return prices.iter().map(|_| WtPoint { wt1: f64::NAN, wt2: f64::NAN }).collect();
    }

    // EMA of typical price (用 close 近似)
    let ema1 = match talib_ema(prices, n1) {
        Ok(v) => v,
        Err(_) => return prices.iter().map(|_| WtPoint { wt1: f64::NAN, wt2: f64::NAN }).collect(),
    };

    // WT1 = (price - ema1) / (0.015 * mean_deviation)
    let n = prices.len();
    let mut wt1_raw = Vec::with_capacity(n);
    for i in 0..n {
        if i < n1 || ema1[i].is_nan() {
            wt1_raw.push(f64::NAN);
            continue;
        }
        // mean deviation of (price - ema1)
        let mut sum_abs = 0.0_f64;
        for j in (i + 1 - n1)..=i {
            let dev = (prices[j] - ema1[j]).abs();
            sum_abs += dev;
        }
        let mean_dev = sum_abs / n1 as f64;
        let denom = 0.015 * mean_dev;
        if denom.abs() < 1e-10 {
            wt1_raw.push(0.0);
        } else {
            wt1_raw.push((prices[i] - ema1[i]) / denom);
        }
    }

    // WT2 = SMA of WT1
    // talib_sma 不跳过 NaN 输入，WT1 前 n1 根为 NaN，
    // 用跳过 NaN 的实现保持与原 ak_core 行为一致
    let mut wt2 = vec![f64::NAN; n];
    let mut valid_count = 0usize;
    let mut sum_wt1: f64 = 0.0;
    let mut first_valid_wt2 = 0.0_f64;
    let mut first_set = false;
    for i in 0..n {
        if !wt1_raw[i].is_nan() {
            valid_count += 1;
            sum_wt1 += wt1_raw[i];
        }
        if valid_count >= n3 {
            // 找 n3 个连续有效值中的最后一个
            let mut window_sum = 0.0;
            let mut cnt = 0;
            for j in (i + 1 - n3)..=i {
                if !wt1_raw[j].is_nan() { window_sum += wt1_raw[j]; cnt += 1; }
            }
            if cnt >= n3 {
                wt2[i] = window_sum / n3 as f64;
            }
        } else if valid_count >= n3 {
            // 检查前 n3 个有效值的滑动窗口
            let mut window_sum = 0.0;
            let mut cnt = 0;
            let start = i.saturating_sub(n3 - 1);
            for j in start..=i {
                if !wt1_raw[j].is_nan() { window_sum += wt1_raw[j]; cnt += 1; }
            }
            if cnt >= n3 {
                wt2[i] = window_sum / n3 as f64;
            }
        }
        if !wt2[i].is_nan() && !first_set { first_valid_wt2 = wt2[i]; first_set = true; }
    }
    // 填充 wt2 前导 NaN
    if first_set {
        for v in &mut wt2 {
            if v.is_nan() { *v = first_valid_wt2; }
        }
    }

    let mut result = Vec::with_capacity(n);
    for i in 0..prices.len() {
        result.push(WtPoint {
            wt1: wt1_raw[i],
            wt2: wt2[i],
        });
    }
    result
}

/// WT 信号值（单根 K 线的完整信号判定）
#[derive(Debug, Clone)]
pub struct WtSignal {
    /// 8 级区位：超涨/多头极强/多头强势/多头/弱多/弱空/空头/空头强势/空头极强/超跌
    pub zone: &'static str,
    /// WTCD 方向：增大/缩小/平稳
    pub direction: &'static str,
    /// 金叉/死叉/多头/空头
    pub cross: &'static str,
}

/// 从 WT 序列和索引生成单根 K 线的信号值。
///
/// # 参数
/// - `wt`: `compute_wavetrend` 的输出
/// - `di`: 从倒数第几根取（1 = 最新一根）
pub fn get_wt_signal(wt: &[WtPoint], di: usize) -> Option<WtSignal> {
    if wt.len() < di + 2 {
        return None;
    }
    let len = wt.len();
    let cur = len - di;
    let prev = cur - 1;

    let w1 = wt[cur].wt1;
    let w2 = wt[cur].wt2;
    let pw1 = wt[prev].wt1;
    let pw2 = wt[prev].wt2;

    if w1.is_nan() || w2.is_nan() || pw1.is_nan() || pw2.is_nan() {
        return None;
    }

    let zone = classify_wt_zone(w1);

    let wtcd_cur = w1 - w2;
    let wtcd_prev = pw1 - pw2;
    let direction = if wtcd_cur > wtcd_prev {
        "增大"
    } else if wtcd_cur < wtcd_prev {
        "缩小"
    } else {
        "平稳"
    };

    let golden_cross = pw1 <= pw2 && w1 > w2;
    let death_cross = pw1 >= pw2 && w1 < w2;
    let cross = if golden_cross {
        "金叉"
    } else if death_cross {
        "死叉"
    } else if w1 > w2 {
        "多头"
    } else {
        "空头"
    };

    Some(WtSignal { zone, direction, cross })
}
pub fn classify_wt_zone(wt1: f64) -> &'static str {
    if wt1 > 100.0 {
        "超涨"
    } else if wt1 > 80.0 {
        "多头极强"
    } else if wt1 > 60.0 {
        "多头强势"
    } else if wt1 > 20.0 {
        "多头"
    } else if wt1 > 0.0 {
        "弱多"
    } else if wt1 > -20.0 {
        "弱空"
    } else if wt1 > -60.0 {
        "空头"
    } else if wt1 > -80.0 {
        "空头强势"
    } else if wt1 > -100.0 {
        "空头极强"
    } else {
        "超跌"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_wavetrend_basic() {
        // 构建一个简单的趋势序列
        let prices: Vec<f64> = (0..100).map(|i| 100.0 + (i as f64 * 0.5)).collect();
        let wt = compute_wavetrend(&prices, 10, 21, 4);
        assert_eq!(wt.len(), prices.len());
        // 最后几个不应该是 NaN
        let last = wt.last().unwrap();
        assert!(!last.wt1.is_nan());
        assert!(!last.wt2.is_nan());
    }

    #[test]
    fn test_wavetrend_insufficient_data() {
        let prices = vec![100.0; 5];
        let wt = compute_wavetrend(&prices, 10, 21, 4);
        assert!(wt.last().unwrap().wt1.is_nan());
    }

    #[test]
    fn test_classify_zone() {
        assert_eq!(classify_wt_zone(150.0), "超涨");
        assert_eq!(classify_wt_zone(90.0), "多头极强");
        assert_eq!(classify_wt_zone(50.0), "多头");
        assert_eq!(classify_wt_zone(10.0), "弱多");
        assert_eq!(classify_wt_zone(-5.0), "弱空");
        assert_eq!(classify_wt_zone(-50.0), "空头");
        assert_eq!(classify_wt_zone(-150.0), "超跌");
    }
}
