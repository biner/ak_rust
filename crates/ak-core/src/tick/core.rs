//! 分时图核心分析算法。

use crate::tick::types::{TickChartResult, TickPoint};

// ── 常量 ──

/// 大单阈值倍数（成交量大于均值 × 此数视为大单）
const LARGE_TRADE_MULTIPLIER: f64 = 2.0;

/// 早盘结束索引（9:30 ~ 11:30 = 120 分钟）
const AM_END_IDX: usize = 120;

/// 尾盘开始索引（14:00 = 第 210 分钟）
const LAST_30_START_IDX: usize = 210;

/// 分析一组分钟级 Tick 数据，返回量价特征和撤单建议。
///
/// # 参数
/// - `ticks`: TickPoint 切片（按时间升序，完整或部分日内数据）
/// - `pre_close`: 前收盘价（用于计算涨跌幅）
/// - `low_price`: 日线最低价（None 则用 tick 内最低价）
/// - `close_pos_threshold`: 收盘位置阈值。Python 侧根据 EMA 等条件动态决定：
///   - low 在 EMA100/EMA200 之上 → 传 0.2
///   - 否则 → 传 0.3
pub fn analyze_tick_chart(
    ticks: &[TickPoint],
    pre_close: f64,
    low_price: Option<f64>,
    close_pos_threshold: f64,
) -> Option<TickChartResult> {
    if ticks.len() < 10 {
        return None;
    }

    let n = ticks.len();
    let prices: Vec<f64> = ticks.iter().map(|t| t.price).collect();
    let vols: Vec<i64> = ticks.iter().map(|t| t.vol).collect();

    let _open = ticks[0].price;
    let close = ticks[n - 1].price;
    let high = prices.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
    let low = prices.iter().cloned().fold(f64::INFINITY, f64::min);
    let total_vol: i64 = vols.iter().sum();

    // ── 基础统计 ──
    let day_chg_pct = if pre_close > 0.0 {
        (close - pre_close) / pre_close * 100.0
    } else {
        0.0
    };
    let amplitude_pct = if pre_close > 0.0 {
        (high - low) / pre_close * 100.0
    } else {
        0.0
    };
    let price_range = high - low;
    // 收盘位置用日线low（如果传入）或tick内最低价
    let ref_low = low_price.unwrap_or(low);
    let close_pos = if price_range > 0.0 {
        (close - ref_low) / (high - ref_low).max(price_range)
    } else {
        0.5
    };

    // ── 时段分析 ──
    let am_end = AM_END_IDX.min(n);
    let total_minutes = n;

    let (am_avg_price, pm_avg_price) = if am_end > 0 && n > am_end {
        let am_sum: f64 = prices[..am_end].iter().sum();
        let pm_sum: f64 = prices[am_end..].iter().sum();
        let am_n = am_end;
        let pm_n = n - am_end;
        (am_sum / am_n as f64, pm_sum / pm_n as f64)
    } else {
        let avg = prices.iter().sum::<f64>() / n as f64;
        (avg, avg)
    };

    let am_pm_diff_pct = if am_avg_price > 0.0 {
        (pm_avg_price - am_avg_price) / am_avg_price * 100.0
    } else {
        0.0
    };

    // 尾盘 30 分钟涨跌幅
    let last_30_start = LAST_30_START_IDX.min(n.saturating_sub(1));
    let last_30min_chg_pct = if last_30_start < n && prices[last_30_start] > 0.0 {
        (close - prices[last_30_start]) / prices[last_30_start] * 100.0
    } else {
        0.0
    };

    // ── 分钟级统计 ──
    let avg_vol_per_min = if total_minutes > 0 {
        total_vol as f64 / total_minutes as f64
    } else {
        0.0
    };

    let vol_std = if n > 1 {
        let variance: f64 = vols
            .iter()
            .map(|v| {
                let d = *v as f64 - avg_vol_per_min;
                d * d
            })
            .sum::<f64>()
            / (n - 1) as f64;
        variance.sqrt()
    } else {
        0.0
    };

    // ── 大单分析 ──
    let large_threshold = avg_vol_per_min * LARGE_TRADE_MULTIPLIER;
    let large_indices: Vec<usize> = vols
        .iter()
        .enumerate()
        .filter(|&(_, v)| (*v as f64) > large_threshold)
        .map(|(i, _)| i)
        .collect();

    let large_trade_count = large_indices.len();
    let large_vol_sum: i64 = large_indices.iter().map(|&i| vols[i]).sum();
    let large_trade_vol_ratio = if total_vol > 0 {
        large_vol_sum as f64 / total_vol as f64
    } else {
        0.0
    };

    let large_am = large_indices.iter().filter(|&&i| i < AM_END_IDX).count();
    let large_pm = large_indices.iter().filter(|&&i| i >= AM_END_IDX).count();
    let large_last_30 = large_indices
        .iter()
        .filter(|&&i| i >= LAST_30_START_IDX)
        .count();
    let large_last_30_vol: i64 = large_indices
        .iter()
        .filter(|&&i| i >= LAST_30_START_IDX)
        .map(|&i| vols[i])
        .sum();

    // ── 综合评分与撤单判断 ──
    let (quality_score, should_cancel, cancel_reason) = compute_quality(
        close_pos,
        day_chg_pct,
        close_pos_threshold,
        large_am,
        large_pm,
        large_last_30,
        last_30min_chg_pct,
        am_pm_diff_pct,
    );

    Some(TickChartResult {
        close_pos,
        day_chg_pct,
        amplitude_pct,
        total_vol,
        n,
        am_avg_price,
        pm_avg_price,
        am_pm_diff_pct,
        last_30min_chg_pct,
        large_trade_count,
        large_trade_vol_ratio,
        large_am,
        large_pm,
        large_last_30,
        large_last_30_vol,
        avg_vol_per_min,
        vol_std,
        quality_score,
        should_cancel,
        cancel_reason,
    })
}

/// 根据量价特征计算综合质量和撤单建议。
fn compute_quality(
    close_pos: f64,
    day_chg_pct: f64,
    close_pos_threshold: f64,
    large_am: usize,
    large_pm: usize,
    large_last_30: usize,
    last_30min_chg_pct: f64,
    am_pm_diff_pct: f64,
) -> (f64, bool, String) {
    let (mut score, mut reasons): (f64, Vec<String>) = (50.0, Vec::new()); // 基础分 50

    // 1. 收盘位置评分（阈值由调用方决定，如 0.2 或 0.3）
    if close_pos >= 0.8 {
        score += 25.0; // 高位收盘，强信号
    } else if close_pos >= close_pos_threshold {
        score += 10.0; // 在阈值之上，中性偏强
    } else {
        score -= 25.0; // 低于阈值，弱信号
        reasons.push(format!(
            "收盘位置过低({:.2}<{:.1})",
            close_pos, close_pos_threshold
        ));
    }

    // 2. 尾盘大单（大资金持续买入）
    if large_last_30 >= 3 {
        score += 20.0;
        reasons.push(format!("尾盘大单活跃({})", large_last_30));
    } else if large_last_30 >= 1 {
        score += 5.0;
    } else {
        score -= 5.0; // 尾盘无大单
    }

    // 3. 大单分布（早盘 vs 午盘）
    if large_am > 0 && large_pm > 0 {
        let ratio = large_am as f64 / large_pm.max(1) as f64;
        if ratio > 3.0 && large_last_30 == 0 {
            // 早盘大单远多于午盘且尾盘无大单 → 拉高出货
            score -= 20.0;
            reasons.push(format!("大单集中在早盘({}:{})尾盘无支撑", large_am, large_pm));
        }
    }

    // 4. 尾盘走势强度
    if last_30min_chg_pct > 1.0 {
        score += 10.0; // 尾盘拉升
    } else if last_30min_chg_pct < -1.0 {
        score -= 10.0; // 尾盘跳水
        reasons.push(format!("尾盘跳水({:+.1}%)", last_30min_chg_pct));
    }

    // 5. 早午盘趋势
    if am_pm_diff_pct > 0.5 {
        score += 5.0; // 午盘强于早盘
    } else if am_pm_diff_pct < -0.5 {
        score -= 5.0; // 午盘弱于早盘
    }

    // 6. 当日涨幅（避免过度追高）
    if day_chg_pct > 8.0 {
        score -= 10.0;
        reasons.push(format!("当日涨幅过大({:+.1}%)", day_chg_pct));
    }

    // 归一化到 [0, 100]
    let quality_score = score.clamp(0.0, 100.0);

    // 撤单决策
    let should_cancel = quality_score < 40.0;
    let cancel_reason = if should_cancel && !reasons.is_empty() {
        reasons.join("; ")
    } else {
        String::new()
    };

    (quality_score, should_cancel, cancel_reason)
}

/// 仅计算收盘位置（轻量级，无需完整分析）。
pub fn calc_close_pos(ticks: &[TickPoint]) -> Option<f64> {
    if ticks.len() < 2 {
        return None;
    }
    let prices: Vec<f64> = ticks.iter().map(|t| t.price).collect();
    let high = prices.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
    let low = prices.iter().cloned().fold(f64::INFINITY, f64::min);
    let close = prices[prices.len() - 1];
    let range = high - low;
    if range > 0.0 {
        Some((close - low) / range)
    } else {
        Some(0.5)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveTime;

    fn t(h: u32, m: u32, s: u32) -> NaiveTime {
        NaiveTime::from_hms_opt(h, m, s).unwrap()
    }

    fn make_strong_ticks() -> Vec<TickPoint> {
        // 模拟强势信号日：早盘平稳 → 午盘放量拉升 → 尾盘高位收盘
        let mut ticks = Vec::new();
        let mut price = 100.0;
        for i in 0..240 {
            let minute = i;
            let h = 9 + minute / 60;
            let m = minute % 60;
            if h >= 15 {
                break;
            }
            let time = t(h, m, 0);

            if i < 60 {
                price += 0.02; // 早盘缓慢上涨
            } else if i < 120 {
                price += 0.01; // 10:30-11:30 横盘
            } else if i < 180 {
                price += 0.05; // 下午放量拉升
            } else {
                price += 0.03; // 尾盘继续涨
            }

            let vol = if i >= 120 { 100000 } else { 30000 };
            ticks.push(TickPoint {
                time,
                price,
                avg: price - 0.1,
                vol,
                momentum: if i >= 120 { 5000.0 } else { 500.0 },
            });
        }
        ticks
    }

    fn make_weak_ticks() -> Vec<TickPoint> {
        // 模拟弱势信号日：早盘巨量拉升 → 午盘缩量回落 → 尾盘跳水
        let mut ticks = Vec::new();
        let mut price = 100.0;
        for i in 0..240 {
            let minute = i;
            let h = 9 + minute / 60;
            let m = minute % 60;
            if h >= 15 {
                break;
            }
            let time = t(h, m, 0);

            if i < 30 {
                price += 0.30; // 开盘30分钟猛拉
            } else if i < 60 {
                price -= 0.10; // 回落
            } else if i < 120 {
                price -= 0.05; // 继续下滑
            } else if i < 180 {
                price += 0.02; // 下午小幅反弹
            } else {
                price -= 0.15; // 尾盘跳水
            }

            let vol = if i < 30 { 200000 } else { 20000 };
            ticks.push(TickPoint {
                time,
                price,
                avg: price + 0.1,
                vol,
                momentum: if i < 30 { 15000.0 } else { -2000.0 },
            });
        }
        ticks
    }

    #[test]
    fn test_analyze_strong_signal() {
        let ticks = make_strong_ticks();
        let result = analyze_tick_chart(&ticks, 99.5, None, 0.3).unwrap();
        assert!(result.close_pos > 0.7, "强势信号应高位收盘");
        assert!(result.quality_score >= 50.0, "强势信号评分应较高");
        assert!(!result.should_cancel, "强势信号不应撤单");
        println!("强势信号: close_pos={:.2} score={:.0}", result.close_pos, result.quality_score);
    }

    #[test]
    fn test_analyze_weak_signal() {
        let ticks = make_weak_ticks();
        let result = analyze_tick_chart(&ticks, 99.5, None, 0.3).unwrap();
        assert!(result.close_pos < 0.5, "弱势信号应低位收盘");
        println!(
            "弱势信号: close_pos={:.2} score={:.0} cancel={} reason={}",
            result.close_pos, result.quality_score, result.should_cancel, result.cancel_reason
        );
        // 弱势信号应触发撤单（早盘大单集中+尾盘跳水）
        assert!(result.should_cancel, "弱势信号应撤单");
    }

    #[test]
    fn test_large_trade_distribution() {
        let strong = make_strong_ticks();
        let weak = make_weak_ticks();
        let r_strong = analyze_tick_chart(&strong, 99.5, None, 0.3).unwrap();
        let r_weak = analyze_tick_chart(&weak, 99.5, None, 0.3).unwrap();
        println!("强势: large_am={} large_pm={}", r_strong.large_am, r_strong.large_pm);
        println!("弱势: large_am={} large_pm={}", r_weak.large_am, r_weak.large_pm);
    }

    #[test]
    fn test_insufficient_data() {
        let ticks = vec![TickPoint {
            time: t(9, 30, 0),
            price: 100.0,
            avg: 100.0,
            vol: 1000,
            momentum: 0.0,
        }];
        assert!(analyze_tick_chart(&ticks, 100.0, None, 0.3).is_none());
    }
}
