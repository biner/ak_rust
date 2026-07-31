//! 卖盘假压制检测器 V2.0 (False Sell Pressure Detector — T2 模式)
//!
//! 识别竞价阶段主力用大额卖单压制价格、实际在暗中吸筹的"假摔"行为。
//!
//! 核心假设：
//!   真出货 = 卖盘多 + 价格下跌
//!   假压制 = 卖盘多 + 价格不跌/上涨 (主力压制价格以便吸筹)
//!
//! **T2 越高，假压制证据越充分**。与 T1 配合使用时：
//!   - T2 ≥ 0.60: 强假压制信号——卖盘汹涌但价格坚挺，确认主力压盘吸筹
//!   - T2 0.30~0.60: 有假压制迹象但不强烈
//!   - T2 < 0.30: 卖压不足，无法判断
//!
//! 推荐：T1 ≥ 阈值 AND T2 ≥ 0.40 → 确认主力吸筹且存在假压制（强化信号）
//! 原版 "T2 < 0.60 才排除" 的逻辑存在问题——T2 越高越像假压制，不应排除。

use chrono::Timelike;
use crate::auction::AuctionTick;

/// 卖盘假压制检测结果
#[derive(Debug, Clone, PartialEq)]
pub struct FalseSellResult {
    /// 综合 T2 得分 [0, 100] 百分制
    pub score: i32,
    /// score >= threshold
    pub match_: bool,
    // ── 原始因子 ──
    pub neg_ratio: f64,
    pub price_up: bool,
    pub vr: f64,
    pub last_um: i32,
    pub neg_total: i64,
    pub late_peak: bool,
    pub recovery_score: f64,
    // ── 辅助指标 ──
    pub n: usize,
    pub max_match: i32,
    pub price_change_pct: f64,
    pub net_um: i64,
    pub total_amt: f64,
    pub big_order_ratio: f64,
}

/// 卖盘假压制检测器 trait。
pub trait FalseSellAnalyzer {
    /// 分析集合竞价数据，检测是否存在虚假卖出（压单后撤单）行为。
    fn analyze(&self, data: &[AuctionTick]) -> Option<FalseSellResult>;
}

/// 卖盘假压制检测器 (T2 模式)
#[derive(Debug, Clone)]
pub struct FalseSellDetector {
    /// 百分制阈值 [0, 100]
    threshold: i32,
}

impl FalseSellAnalyzer for FalseSellDetector {
    fn analyze(&self, data: &[AuctionTick]) -> Option<FalseSellResult> {
        self.analyze_inner(data)
    }
}

impl FalseSellDetector {
    /// `threshold`: 百分制阈值 [0, 100]，默认 50
    pub fn new(threshold: i32) -> Self {
        Self { threshold }
    }

    /// 分析集合竞价数据，返回 T2 评分。
    /// 仅处理 09:15-09:25 时段内的 tick。
    /// 注：通过 `FalseSellAnalyzer` trait 调用 `analyze`，此方法改名避免冲突。
    pub fn analyze_inner(&self, data: &[AuctionTick]) -> Option<FalseSellResult> {
        if data.len() < 15 {
            return None;
        }

        // 只保留 09:15-09:25 时段
        let recs: Vec<&AuctionTick> = data
            .iter()
            .filter(|t| {
                let h = t.time.hour();
                let m = t.time.minute();
                h == 9 && (15..=25).contains(&m)
            })
            .collect();

        if recs.len() < 15 {
            return None;
        }

        let n = recs.len();
        let prices: Vec<f64> = recs.iter().map(|t| t.price).collect();
        let matches: Vec<i32> = recs.iter().map(|t| t.matched).collect();
        let ums: Vec<i32> = recs.iter().map(|t| t.unmatched).collect();

        // ── 1. 卖盘时间占比 sell_ratio ──
        let neg_count = ums.iter().filter(|&&u| u < 0).count();
        let neg_ratio = neg_count as f64 / n as f64;
        let sr_score = if neg_ratio > 0.6 {
            1.0
        } else if neg_ratio > 0.4 {
            0.6
        } else if neg_ratio > 0.25 {
            0.3
        } else {
            0.0
        };

        // ── 2. 价格韧性 price_resilience ──
        let price_up = prices.last() >= prices.first();
        let pr_score = if price_up {
            let mut sell_then_recover = 0_usize;
            for i in 1..n {
                if ums[i - 1] < 0 && ums[i] >= 0 && prices[i] >= prices[i - 1] - 0.005 {
                    sell_then_recover += 1;
                }
            }
            let recover_rate = sell_then_recover as f64 / neg_count.max(1) as f64;
            0.4 + 0.6 * (recover_rate * 2.0).min(1.0)
        } else {
            0.0
        };

        // ── 3. 尾段放量 (volume_surge) vs 初始段 ──
        let first3 = &recs[..3.min(n)];
        let last3 = &recs[n.saturating_sub(3)..];
        let avg_first = first3.iter().map(|t| t.matched as f64).sum::<f64>() / first3.len().max(1) as f64;
        let avg_last = last3.iter().map(|t| t.matched as f64).sum::<f64>() / last3.len().max(1) as f64;
        let vr = avg_last / avg_first.max(1.0);
        let vs_score = if vr > 3.0 {
            1.0
        } else if vr > 1.5 {
            0.5
        } else {
            0.0
        };

        // ── 4. 末笔大额卖压 final_sell_wall ──
        let last_um = ums[n - 1];
        let fw_score = if last_um < -500 {
            1.0
        } else if last_um < -200 {
            0.6
        } else {
            0.0
        };

        // ── 5. 累计卖盘总量 total_sell_volume ──
        let neg_total: i64 = ums.iter().filter(|&&u| u < 0).map(|&u| -(u as i64)).sum();
        let tv_score = if neg_total > 50000 {
            1.0
        } else if neg_total > 10000 {
            0.5
        } else {
            0.0
        };

        // ── 6. 最高价出现在后半段 peak_late ──
        let max_idx = prices
            .iter()
            .enumerate()
            .max_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap())
            .map(|(i, _)| i)
            .unwrap_or(0);
        let late_peak = max_idx as f64 > n as f64 * 0.4;
        let pl_score = if late_peak { 1.0 } else { 0.0 };

        // ── 7. 卖盘→买盘恢复速度 sell_recovery ──
        let recovery_score = if neg_count >= 2 {
            let last_buy_idx = ums.iter().rposition(|&u| u > 0);
            if let Some(lbi) = last_buy_idx {
                if lbi as f64 > n as f64 * 0.5 {
                    let mut consecutive_sell = 0_usize;
                    let mut max_cons_sell = 0_usize;
                    for &u in ums.iter() {
                        if u < 0 {
                            consecutive_sell += 1;
                        } else {
                            max_cons_sell = max_cons_sell.max(consecutive_sell);
                            consecutive_sell = 0;
                        }
                    }
                    max_cons_sell = max_cons_sell.max(consecutive_sell);
                    if max_cons_sell <= 3 {
                        1.0
                    } else if max_cons_sell <= 6 {
                        0.5
                    } else {
                        0.2
                    }
                } else {
                    0.0
                }
            } else {
                0.0
            }
        } else {
            0.0
        };

        // ── 综合评分 ──
        let mut s = 0.0;
        s += sr_score * 0.20;
        s += pr_score * 0.20;
        s += vs_score * 0.15;
        s += fw_score * 0.10;
        s += tv_score * 0.10;
        s += pl_score * 0.10;
        s += recovery_score * 0.15;

        // ── 惩罚 ──
        let price_change_pct =
            (prices[n - 1] - prices[0]) / prices[0] * 100.0;
        if price_change_pct < -1.0 {
            s *= 0.3;
        } else if price_change_pct < -0.5 {
            s *= 0.6;
        }

        let max_match = matches.iter().max().copied().unwrap_or(0);
        if max_match < 300 {
            s *= 0.5;
        }
        if n < 20 {
            s *= 0.8;
        }
        s = s.min(1.0);

        let score = (s * 100.0).round() as i32;

        // ── 辅助指标 ──
        let pos_total: i64 = ums.iter().filter(|&&u| u > 0).map(|&u| u as i64).sum();
        let net_um = pos_total - neg_total;
        let auction_total_amt: f64 = matches
            .iter()
            .zip(prices.iter())
            .map(|(&m, &p)| m as f64 * p * 100.0)
            .sum();
        let big_order_ratio =
            matches.iter().filter(|&&m| m >= 1000).count() as f64 / n as f64;

        Some(FalseSellResult {
            score,
            match_: score >= self.threshold,
            neg_ratio: (neg_ratio * 1000.0).round() / 1000.0,
            price_up,
            vr: (vr * 100.0).round() / 100.0,
            last_um,
            neg_total,
            late_peak,
            recovery_score: (recovery_score * 100.0).round() / 100.0,
            n,
            max_match,
            price_change_pct: (price_change_pct * 100.0).round() / 100.0,
            net_um,
            total_amt: auction_total_amt,
            big_order_ratio: (big_order_ratio * 1000.0).round() / 1000.0,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveTime;

    fn t(h: u32, m: u32, s: u32) -> NaiveTime {
        NaiveTime::from_hms_opt(h, m, s).unwrap()
    }

    fn make_false_sell_ticks() -> Vec<AuctionTick> {
        // 卖盘多但价格不跌——典型的假压制
        let mut ticks = vec![];
        for i in 0..20 {
            ticks.push(AuctionTick {
                time: t(9, 15 + i as u32 / 3, (i as u32 * 20) % 60),
                price: 10.00 + (i as f64 * 0.005),
                matched: 300 + (i as i32) * 10,
                unmatched: if i % 3 == 0 { -200 } else { 50 },
            });
        }
        ticks
    }

    fn make_true_sell_ticks() -> Vec<AuctionTick> {
        // 卖盘多 + 价格持续下跌——真出货
        let mut ticks = vec![];
        for i in 0..20 {
            ticks.push(AuctionTick {
                time: t(9, 15 + i as u32 / 3, (i as u32 * 20) % 60),
                price: 10.00 - (i as f64 * 0.02),
                matched: 200 + (i as i32) * 5,
                unmatched: -500,
            });
        }
        ticks
    }

    #[test]
    fn test_false_sell_returns_some() {
        let d = FalseSellDetector::new(50);
        let r = d.analyze(&make_false_sell_ticks());
        assert!(r.is_some());
    }

    #[test]
    fn test_false_sell_higher_than_true_sell() {
        let d = FalseSellDetector::new(50);
        let false_r = d.analyze(&make_false_sell_ticks()).unwrap();
        let true_r = d.analyze(&make_true_sell_ticks()).unwrap();
        // 假压制得分应显著高于真出货
        assert!(
            false_r.score > true_r.score,
            "false={} should be > true={}",
            false_r.score,
            true_r.score
        );
    }

    #[test]
    fn test_penalty_for_price_drop() {
        let d = FalseSellDetector::new(50);
        let mut ticks = make_false_sell_ticks();
        // 修改为大幅下跌
        let n = ticks.len();
        ticks[n - 1].price = ticks[0].price - 0.15;
        let r = d.analyze(&ticks).unwrap();
        assert!(r.score < 50, "score should be heavily penalized: {}", r.score);
    }

    #[test]
    fn test_neg_ratio_scores() {
        let d = FalseSellDetector::new(50);
        // 全部卖盘
        let mut ticks = vec![];
        for i in 0..20 {
            ticks.push(AuctionTick {
                time: t(9, 15 + i as u32 / 3, (i as u32 * 20) % 60),
                price: 10.00,
                matched: 500,
                unmatched: -100,
            });
        }
        let r = d.analyze(&ticks).unwrap();
        assert!(r.neg_ratio > 0.5);
    }
}
