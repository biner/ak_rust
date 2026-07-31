//! 集合竞价主力吸筹分析器。
use chrono::Timelike;
use crate::auction::morphology;

// ── Trait 定义 ──

/// 集合竞价分析器 trait。
pub trait AuctionAnalyzer {
    /// 分析集合竞价数据，返回综合评分和形态分类。
    fn analyze(
        &self,
        data: &[AuctionTick],
        open_price: Option<f64>,
        pre_close: Option<f64>,
        limit_up: Option<f64>,
        limit_down: Option<f64>,
    ) -> Option<ScanResult>;
}


/// 集合竞价逐笔 tick。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AuctionTick {
    /// 时间（仅时分秒，如 09:20:00）
    pub time: chrono::NaiveTime,
    /// 撮合价（元）
    pub price: f64,
    /// 匹配量（手）
    pub matched: i32,
    /// 未匹配量（手，正值=买盘，负值=卖盘）
    pub unmatched: i32,
}

/// 集合竞价扫描结果。
#[derive(Debug, Clone, PartialEq)]
pub struct ScanResult {
    /// 综合得分 [0, 100] 百分制
    pub score: i32,
    /// 是否匹配（score >= threshold）
    pub match_: bool,
    /// 原始阶梯数（去重后的价格层级数 - 1）
    pub steps: usize,
    /// 有效阶梯数（>=2 时才计入评分）
    pub eff_steps: usize,
    /// 09:20-09:22 是否存在买卖交替
    pub neg_pos_alt: bool,
    /// 09:20-09:22 是否存在买盘反超
    pub buy_rev: bool,
    /// 尾段放量比（最后 3 tick 平均匹配量 / 中部平均匹配量）
    pub vr: f64,
    /// 是否存在尾盘压盘
    pub end_p: bool,
    /// 是否高开
    pub oh: Option<bool>,
    /// tick 总数
    pub n: usize,
    /// 最大匹配量
    pub max_m: i32,
    /// 第一笔价格
    pub first_p: f64,
    /// 最后一笔价格
    pub last_p: f64,
    /// 最后一笔未匹配量
    pub last_um: i32,
    /// 竞价形态分类名称
    pub morphology: String,
    /// 形态置信度 [0, 100] 百分制
    pub morphology_conf: i32,
    /// 7 种形态各自的得分 [0, 100]
    pub morphology_scores: [i32; 7],
    /// 形态编码 (0-6, 对应 MORPHOLOGY_NAMES 索引)
    pub morphology_code: usize,
    /// 形态分组的组名 (A/B/YZ/ZD)
    pub morphology_group: String,
    /// 形态编制代码 (如 A01, YZ01)
    pub morphology_tag: String,
}

/// 从参数推导涨跌停价。
fn resolve_limit_prices(
    limit_up: Option<f64>,
    limit_down: Option<f64>,
    pre_close: Option<f64>,
) -> (f64, f64) {
    match (limit_up, limit_down) {
        (Some(u), Some(d)) => (u, d),
        _ => {
            if let Some(pc) = pre_close {
                (pc * 1.10, pc * 0.90)
            } else {
                (0.0, 0.0)
            }
        }
    }
}

/// 集合竞价扫描器。
///
/// 使用 `Scanner::new(threshold)` 构造，调用 `analyze()` 执行分析。
#[derive(Debug, Clone)]
pub struct AuctionScanner {
    threshold: i32,
}

impl AuctionAnalyzer for AuctionScanner {
    fn analyze(
        &self,
        data: &[AuctionTick],
        open_price: Option<f64>,
        pre_close: Option<f64>,
        limit_up: Option<f64>,
        limit_down: Option<f64>,
    ) -> Option<ScanResult> {
        self.analyze_filter(data, open_price, pre_close, limit_up, limit_down, true)
    }
}

impl AuctionScanner {
    /// 创建扫描器。
    ///
    /// * `threshold` — 匹配阈值，百分制 [0, 100]（建议 50 严格 / 40 宽松）。
    pub fn new(threshold: i32) -> Self {
        Self { threshold }
    }

    /// 分析集合竞价数据。
    ///
    /// `limit_up` / `limit_down`: 涨停/跌停价，传入后可提高涨停试盘型/跌停撬板型识别的准确率。
    /// 不传时也会根据价格相对变化做近似判断。
    ///
    /// 默认只分析 `09:15-09:25` 竞价时段的数据。`stock_auction()` 在收盘后
    /// 会返回全日 tick（含连续交易），不过滤会虚增阶梯数等指标。
    /// 设置 `filter_auction=true`（默认）启用过滤，传入 `false` 关闭。
    /// 返回 `None` 表示数据不足或格式错误。
    pub fn analyze(
        &self,
        data: &[AuctionTick],
        open_price: Option<f64>,
        pre_close: Option<f64>,
        limit_up: Option<f64>,
        limit_down: Option<f64>,
    ) -> Option<ScanResult> {
        self.analyze_filter(data, open_price, pre_close, limit_up, limit_down, true)
    }

    /// 与 `analyze` 相同，但可控制是否过滤竞价时段。
    pub fn analyze_filter(
        &self,
        data: &[AuctionTick],
        open_price: Option<f64>,
        pre_close: Option<f64>,
        limit_up: Option<f64>,
        limit_down: Option<f64>,
        filter_auction: bool,
    ) -> Option<ScanResult> {
        if data.len() < 15 {
            return None;
        }

        let recs: Vec<AuctionTick> = if filter_auction {
            data.iter()
                .filter(|t| {
                    let h = t.time.hour();
                    let m = t.time.minute();
                    h == 9 && (15..=25).contains(&m)
                })
                .copied()
                .collect()
        } else {
            data.to_vec()
        };

        if recs.len() < 15 {
            return None;
        }
        let n_total = recs.len();

        // ── 阶梯拉升 ──────────────────────────────────────────────────────────
        // 全量阶梯（用于形态分类和输出）
        let mut seen_p: Vec<i64> = Vec::new();
        let mut deduped: Vec<i64> = Vec::new();
        for r in &recs {
            let p = (r.price * 100.0 + 0.5).floor() as i64;
            if let Some(&last) = deduped.last() {
                if (p - last).abs() <= 0 { continue; }
            }
            if !seen_p.contains(&p) { seen_p.push(p); deduped.push(p); }
        }
        let valid_steps = if deduped.is_empty() { 0 } else { deduped.len() - 1 };
        let effective_steps = if valid_steps >= 2 { valid_steps } else { 0 };

        // 真实期阶梯（09:20-09:25，用于评分，避免虚报期噪声）
        let t920 = chrono::NaiveTime::from_hms_opt(9, 20, 0).unwrap();
        let mut real_seen: Vec<i64> = Vec::new();
        let mut real_deduped: Vec<i64> = Vec::new();
        for r in recs.iter().filter(|t| t.time >= t920) {
            let p = (r.price * 100.0 + 0.5).floor() as i64;
            if let Some(&last) = real_deduped.last() {
                if (p - last).abs() <= 0 { continue; }
            }
            if !real_seen.contains(&p) { real_seen.push(p); real_deduped.push(p); }
        }
        let real_valid = if real_deduped.is_empty() { 0 } else { real_deduped.len() - 1 };
        let real_eff_steps = if real_valid >= 2 { real_valid } else { 0 };

        // ── 09:20-09:22 买卖判别 ──────────────────────────────────────────────
        let phase2: Vec<&AuctionTick> = recs
            .iter()
            .filter(|r| {
                let h = r.time.hour();
                let m = r.time.minute();
                h == 9 && (20..=22).contains(&m)
            })
            .collect();

        let has_p2 = phase2.len() >= 3;
        let (neg_pos_alt, buy_rev) = if has_p2 {
            let neg = phase2.iter().any(|r| r.unmatched < 0);
            let pos = phase2.iter().any(|r| r.unmatched > 0);
            let alt = neg && pos;
            let rev = if neg && pos {
                let ni: Vec<usize> = phase2
                    .iter()
                    .enumerate()
                    .filter(|(_, r)| r.unmatched < 0)
                    .map(|(i, _)| i)
                    .collect();
                let pi: Vec<usize> = phase2
                    .iter()
                    .enumerate()
                    .filter(|(_, r)| r.unmatched > 0)
                    .map(|(i, _)| i)
                    .collect();
                !ni.is_empty() && !pi.is_empty() && *ni.last().unwrap() < *pi.last().unwrap()
            } else {
                false
            };
            (alt, rev)
        } else {
            (false, false)
        };

        // ── 尾段放量 ──────────────────────────────────────────────────────────
        let l3 = if recs.len() >= 3 {
            &recs[recs.len() - 3..]
        } else {
            &recs[..]
        };
        let mid_start = if recs.len() >= 2 {
            (recs.len() / 2).saturating_sub(1)
        } else {
            0
        };
        let mid_end = std::cmp::min(recs.len() / 2 + 2, recs.len());
        let mid = &recs[mid_start..mid_end];

        let avg_l = l3.iter().map(|r| r.matched as f64).sum::<f64>() / l3.len() as f64;
        // 基准量改用 09:20 后的真实期，排除虚报期噪声
        let base_ticks: Vec<&AuctionTick> = recs.iter().filter(|t| t.time >= t920).collect();
        let avg_base = if base_ticks.len() > 3 {
            base_ticks[..base_ticks.len() - 3].iter().map(|r| r.matched as f64).sum::<f64>()
                / (base_ticks.len() - 3) as f64
        } else {
            mid.iter().map(|r| r.matched as f64).sum::<f64>() / mid.len().max(1) as f64
        };
        let vr = if avg_base > 0.0 { avg_l / avg_base } else { 1.0 };

        // ── 尾盘压盘 ──────────────────────────────────────────────────────────
        let end_p = if recs.len() >= 3 {
            let last = recs.last().unwrap();
            let third = &recs[recs.len() - 3];
            last.unmatched < -500 && last.price >= third.price
        } else {
            false
        };

        // ── 高开 ──────────────────────────────────────────────────────────────
        let oh = match (open_price, pre_close) {
            (Some(op), Some(pc)) if pc > 0.0 => Some(op > pc),
            _ => None,
        };

        // ── 评分 ──────────────────────────────────────────────────────────────
        let mut score = 0.0_f64;
        score += (real_eff_steps as f64 / 4.0).min(1.0) * 0.25;
        score += if neg_pos_alt { 1.0 } else { 0.0 } * 0.10;
        score += if buy_rev { 1.0 } else { 0.0 } * 0.35;
        score += (vr / 2.0).min(1.0) * 0.15;
        if end_p {
            score += 0.15;
        }

        // ── 惩罚 ──────────────────────────────────────────────────────────────
        let mut score = score;
        // 价格方向改为以 09:20 为基准（真实期起始），排除虚报期扰动
        let ref_price = recs.iter().find(|r| r.time >= t920).map(|r| r.price).unwrap_or(recs[0].price);
        if recs.last().unwrap().price < ref_price - 0.01 {
            score *= 0.5;
        }
        if recs.len() < 20 {
            score *= 0.8;
        }
        // 流动性惩罚：按单 tick 最大成交额（元），避免高价股被手数阈值误伤
        let max_m = recs.iter().map(|r| r.matched).max().unwrap_or(0);
        let avg_price = recs.iter().map(|r| r.price).sum::<f64>() / recs.len() as f64;
        let max_amt = max_m as f64 * avg_price * 100.0;
        if max_amt < 500_000.0 {
            score *= 0.6;
        } else if max_amt < 1_000_000.0 {
            score *= 0.8;
        }
        // ≥ 100 万不惩罚
        if !has_p2 {
            score *= 0.5;
        }

        let last_tick = recs.last().unwrap();

        // ── 竞价形态分类 ──────────────────────────────────────────────────────
        let (lu_used, ld_used) = resolve_limit_prices(limit_up, limit_down, pre_close);
        let (morphology, morphology_conf, morphology_scores, morphology_code) =
            morphology::classify_morphology(&recs, lu_used, ld_used);

        Some(ScanResult {
            score: (score.min(1.0) * 100.0).round() as i32,
            match_: (score * 100.0).round() as i32 >= self.threshold,
            steps: valid_steps,
            eff_steps: effective_steps,
            neg_pos_alt,
            buy_rev,
            vr: (vr * 100.0).round() / 100.0,
            end_p,
            oh,
            n: n_total,
            max_m,
            first_p: recs[0].price,
            last_p: last_tick.price,
            last_um: last_tick.unmatched,
            morphology,
            morphology_conf,
            morphology_scores,
            morphology_code,
            morphology_group: morphology::morphology_group_tag(morphology_code).0.to_string(),
            morphology_tag: morphology::morphology_group_tag(morphology_code).1.to_string(),
        })
    }
}

// The PyO3-bound Scanner type is re-exported as `Scanner` above.
// For pure Rust consumers, `AuctionScanner` is the native API.

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveTime;

    fn t(h: u32, m: u32, s: u32) -> NaiveTime {
        NaiveTime::from_hms_opt(h, m, s).unwrap()
    }

    /// Build a minimal but valid auction dataset (15+ ticks, enough to pass gates).
    fn make_valid_ticks() -> Vec<AuctionTick> {
        let mut ticks = Vec::new();
        // 09:15 - gradual upward ticks
        for i in 0..8 {
            ticks.push(AuctionTick {
                time: t(9, 15 + i, 0),
                price: 10.00 + i as f64 * 0.01,
                matched: 100 + (i as i32) * 10,
                unmatched: -30 - (i as i32) * 5,
            });
        }
        // 09:20-09:22 - alternating unmatched signs
        ticks.push(AuctionTick { time: t(9, 20, 0), price: 10.08, matched: 200, unmatched: -50 });
        ticks.push(AuctionTick { time: t(9, 20, 30), price: 10.08, matched: 250, unmatched: 40 });
        ticks.push(AuctionTick { time: t(9, 21, 0), price: 10.09, matched: 180, unmatched: -20 });
        ticks.push(AuctionTick { time: t(9, 21, 30), price: 10.09, matched: 300, unmatched: 60 });
        ticks.push(AuctionTick { time: t(9, 22, 0), price: 10.10, matched: 220, unmatched: 30 });
        // 09:23-09:25 - end section
        ticks.push(AuctionTick { time: t(9, 23, 0), price: 10.10, matched: 400, unmatched: -100 });
        ticks.push(AuctionTick { time: t(9, 24, 0), price: 10.11, matched: 600, unmatched: -200 });
        ticks.push(AuctionTick { time: t(9, 25, 0), price: 10.12, matched: 800, unmatched: -600 });
        ticks
    }

    #[test]
    fn test_analyze_returns_none_for_too_few_ticks() {
        let s = AuctionScanner::new(50);
        let ticks = vec![
            AuctionTick { time: t(9, 20, 0), price: 10.0, matched: 100, unmatched: 0 };
            10
        ];
        assert!(s.analyze(&ticks, None, None, None, None).is_none());
    }

    #[test]
    fn test_analyze_returns_some_for_valid_data() {
        let s = AuctionScanner::new(50);
        let ticks = make_valid_ticks();
        let result = s.analyze(&ticks, None, None, None, None);
        assert!(result.is_some());
        let r = result.unwrap();
        assert!((0..=100).contains(&r.score));
        assert_eq!(r.n, ticks.len());
    }

    #[test]
    fn test_step_up_counting() {
        let ticks = vec![
            AuctionTick { time: t(9, 15, 0), price: 10.00, matched: 100, unmatched: 0 },
            AuctionTick { time: t(9, 16, 0), price: 10.01, matched: 100, unmatched: 0 },
            AuctionTick { time: t(9, 17, 0), price: 10.00, matched: 100, unmatched: 0 },
            AuctionTick { time: t(9, 18, 0), price: 10.02, matched: 100, unmatched: 0 },
            AuctionTick { time: t(9, 19, 0), price: 10.03, matched: 100, unmatched: 0 },
            AuctionTick { time: t(9, 20, 0), price: 10.03, matched: 100, unmatched: 0 },
            AuctionTick { time: t(9, 21, 0), price: 10.05, matched: 100, unmatched: 0 },
            AuctionTick { time: t(9, 22, 0), price: 10.06, matched: 100, unmatched: 0 },
            AuctionTick { time: t(9, 23, 0), price: 10.06, matched: 100, unmatched: 0 },
            AuctionTick { time: t(9, 24, 0), price: 10.06, matched: 100, unmatched: 0 },
            AuctionTick { time: t(9, 24, 30), price: 10.07, matched: 100, unmatched: 0 },
            AuctionTick { time: t(9, 24, 45), price: 10.07, matched: 100, unmatched: 0 },
            AuctionTick { time: t(9, 25, 0), price: 10.08, matched: 100, unmatched: 0 },
            AuctionTick { time: t(9, 25, 5), price: 10.08, matched: 100, unmatched: 0 },
            AuctionTick { time: t(9, 25, 10), price: 10.09, matched: 100, unmatched: 0 },
        ];
        let s = AuctionScanner::new(50);
        let r = s.analyze(&ticks, None, None, None, None).unwrap();
        // Distinct prices: 10.00, 10.01, 10.02, 10.03, 10.05, 10.06, 10.07, 10.08, 10.09 = 9 levels
        // Steps = 8, eff_steps = 8 (>=2)
        assert_eq!(r.steps, 8);
        assert_eq!(r.eff_steps, 8);
    }

    #[test]
    fn test_phase2_signals() {
        // 3 ticks in 09:20-09:22 with alternating signs → neg_pos_alt=true
        let mut ticks = vec![
            AuctionTick { time: t(9, 15, 0), price: 10.00, matched: 100, unmatched: 0 };
            6
        ];
        // phase2 with both buy and sell, last direction is buy
        ticks.push(AuctionTick { time: t(9, 20, 0), price: 10.00, matched: 100, unmatched: -50 });
        ticks.push(AuctionTick { time: t(9, 21, 0), price: 10.01, matched: 100, unmatched: 30 });
        ticks.push(AuctionTick { time: t(9, 22, 0), price: 10.02, matched: 100, unmatched: 40 });
        // Fill rest so we have >=15 ticks
        for _ in 0..6 {
            ticks.push(AuctionTick { time: t(9, 23, 0), price: 10.02, matched: 100, unmatched: 0 });
        }
        let s = AuctionScanner::new(50);
        let r = s.analyze(&ticks, None, None, None, None).unwrap();
        assert!(r.neg_pos_alt);
        // ni = [0], pi = [1, 2], max(ni)=0 < max(pi)=2 → buy_rev=true
        assert!(r.buy_rev);
    }

    #[test]
    fn test_end_pressure() {
        // End pressure: last unmatched < -500 and last price >= third-from-last
        let mut ticks: Vec<AuctionTick> = (0..15)
            .map(|i| AuctionTick {
                time: t(9, 15 + i as u32 / 6, (i as u32 * 10) % 60),
                price: 10.00 + (i as f64 * 0.005),
                matched: 100 + (i as i32) * 5,
                unmatched: -10 - i as i32,
            })
            .collect();
        // Override last 3
        let n = ticks.len();
        ticks[n - 3].price = 10.10;
        ticks[n - 3].unmatched = -100;
        ticks[n - 2].price = 10.10;
        ticks[n - 2].unmatched = -200;
        ticks[n - 1].price = 10.11;
        ticks[n - 1].unmatched = -600; // < -500 AND price >= third-from-last (10.10)

        let s = AuctionScanner::new(50);
        let r = s.analyze(&ticks, None, None, None, None).unwrap();
        assert!(r.end_p);
    }

    #[test]
    fn test_penalties() {
        // Ending lower → score * 0.5
        let mut ticks = make_valid_ticks();
        let n = ticks.len();
        ticks[n - 1].price = ticks[0].price - 0.02;

        let s = AuctionScanner::new(50);
        let before = s.analyze(&make_valid_ticks(), None, None, None, None).unwrap();
        let after = s.analyze(&ticks, None, None, None, None).unwrap();
        assert!(after.score <= before.score);
    }

    #[test]
    fn test_high_open() {
        let ticks = make_valid_ticks();
        let s = AuctionScanner::new(50);
        let r = s.analyze(&ticks, Some(10.50), Some(10.00), None, None).unwrap();
        assert_eq!(r.oh, Some(true));

        let r = s.analyze(&ticks, Some(9.50), Some(10.00), None, None).unwrap();
        assert_eq!(r.oh, Some(false));
    }

    #[test]
    fn test_match_threshold() {
        let ticks = make_valid_ticks();
        let s_strict = AuctionScanner::new(90);
        let s_loose = AuctionScanner::new(0);

        let r_strict = s_strict.analyze(&ticks, None, None, None, None).unwrap();
        let r_loose = s_loose.analyze(&ticks, None, None, None, None).unwrap();

        assert!(!r_strict.match_);
        assert!(r_loose.match_);
    }
}
