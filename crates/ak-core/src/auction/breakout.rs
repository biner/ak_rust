//! 突破时刻逐笔成交分析器 —— 判断突破时的资金强度和真假突破。
//!
//! 与 `transaction.rs` 不同，本模块只分析**突破价及以上**的逐笔成交数据，
//! 排除突破前的噪声交易，聚焦于突破时刻的市场真实反应。
//!
//! Python 侧传入当日全部逐笔成交 + 突破价，Rust 侧过滤后分析：
//! - 突破后买/卖力量对比
//! - 大单方向（机构真金白银 vs 散户跟风）
//! - 突破后价格维持能力
//! - 整体判定：强突破 / 弱突破 / 假突破

use chrono::NaiveTime;

// ── 常量 ──

/// 大单阈值倍数（成交量大于均值 × 此数视为大单）
const LARGE_TRADE_MULTIPLIER: f64 = 3.0;

/// 突破价格容忍比例（突破价的 99.5% 算触及突破区）
const BREAKOUT_TOLERANCE: f64 = 0.995;

/// 最少突破区成交笔数要求
const MIN_BREAKOUT_TX: usize = 5;

/// 最少全天成交笔数要求
const MIN_TOTAL_TX: usize = 10;

// ── 数据结构 ──

/// 逐笔成交数据点（复用 TransactionPoint 结构，独立定义避免跨模块依赖）。
#[derive(Debug, Clone)]
pub struct TransactionPoint {
    pub time: NaiveTime,
    pub price: f64,
    pub vol: i64,
    pub trade_count: i32,
    pub bs_flag: i32,
}

/// 突破分析结果。
#[derive(Debug, Clone)]
pub struct BreakoutResult {
    // ── 数据概况 ──
    /// 全天总成交笔数
    pub total_n: usize,
    /// 突破区成交笔数
    pub breakout_n: usize,
    /// 突破区成交量占全天比例
    pub breakout_vol_ratio: f64,

    // ── 买卖力量（突破区）──
    /// 突破区主动买成交量
    pub breakout_buy_vol: i64,
    /// 突破区主动卖成交量
    pub breakout_sell_vol: i64,
    /// 突破区买量占比 buy/(buy+sell)
    pub breakout_buy_ratio: f64,

    // ── 大单分析（突破区）──
    /// 突破区大单总数量
    pub breakout_large_count: i32,
    /// 突破区大买单数量
    pub breakout_large_buy: i32,
    /// 突破区大卖单数量
    pub breakout_large_sell: i32,
    /// 突破区大单中买入占比
    pub breakout_large_buy_ratio: f64,

    // ── 价格维持 ──
    /// 突破区均价
    pub breakout_avg_price: f64,
    /// 突破区最后价格 vs 突破价的比值（>1 表示维持住）
    pub price_sustain_ratio: f64,

    // ── 综合判断 ──
    /// 是否强突破（买入主导 + 大单跟进）
    pub is_strong: bool,
    /// 突破分类："强突破" / "弱突破" / "假突破"
    pub classification: String,
    /// 置信度 [0, 1]
    pub confidence: f64,
    /// 分析明细
    pub details: String,
}

// ── 核心分析函数 ──

/// 分析突破时刻的逐笔成交数据，判断是真突破还是假突破。
///
/// # 参数
/// - `transactions`: 当日全部逐笔成交（按时间升序）
/// - `breakout_price`: 突破价（策略设定的入场触发价）
pub fn analyze_breakout(
    transactions: &[TransactionPoint],
    breakout_price: f64,
) -> Option<BreakoutResult> {
    if transactions.len() < MIN_TOTAL_TX {
        return None;
    }

    let total_n = transactions.len();
    let total_vol: i64 = transactions.iter().map(|t| t.vol).sum();
    if total_vol == 0 {
        return None;
    }

    // ── 过滤：突破价及以上的交易 ──
    let threshold = breakout_price * BREAKOUT_TOLERANCE;
    let breakout_txns: Vec<&TransactionPoint> = transactions
        .iter()
        .filter(|t| t.price >= threshold)
        .collect();

    let breakout_n = breakout_txns.len();
    if breakout_n < MIN_BREAKOUT_TX {
        // 突破区交易太少，无法判断
        return Some(BreakoutResult {
            total_n,
            breakout_n,
            breakout_vol_ratio: 0.0,
            breakout_buy_vol: 0,
            breakout_sell_vol: 0,
            breakout_buy_ratio: 0.5,
            breakout_large_count: 0,
            breakout_large_buy: 0,
            breakout_large_sell: 0,
            breakout_large_buy_ratio: 0.0,
            breakout_avg_price: 0.0,
            price_sustain_ratio: 1.0,
            is_strong: false,
            classification: "数据不足".to_string(),
            confidence: 0.0,
            details: format!("突破区仅{}笔交易，不足以判断", breakout_n),
        });
    }

    let breakout_vol: i64 = breakout_txns.iter().map(|t| t.vol).sum();
    let breakout_vol_ratio = if total_vol > 0 {
        breakout_vol as f64 / total_vol as f64
    } else {
        0.0
    };

    // ── 买卖量分组 ──
    let mut buy_vol: i64 = 0;
    let mut sell_vol: i64 = 0;
    let mut total_price: f64 = 0.0;

    for t in &breakout_txns {
        match t.bs_flag {
            0 => buy_vol += t.vol,
            1 => sell_vol += t.vol,
            _ => {}
        }
        total_price += t.price;
    }

    let breakout_avg_price = total_price / breakout_n as f64;
    let breakout_buy_ratio = if buy_vol + sell_vol > 0 {
        buy_vol as f64 / (buy_vol + sell_vol) as f64
    } else {
        0.5
    };

    // ── 大单分析 ──
    let avg_vol = breakout_vol as f64 / breakout_n as f64;
    let large_threshold = avg_vol * LARGE_TRADE_MULTIPLIER;

    let mut large_count: i32 = 0;
    let mut large_buy: i32 = 0;
    let mut large_sell: i32 = 0;

    for t in &breakout_txns {
        if (t.vol as f64) > large_threshold {
            large_count += 1;
            match t.bs_flag {
                0 => large_buy += 1,
                1 => large_sell += 1,
                _ => {}
            }
        }
    }

    let breakout_large_buy_ratio = if large_count > 0 {
        large_buy as f64 / large_count as f64
    } else {
        0.0
    };

    // ── 价格维持能力 ──
    let last_price = breakout_txns.last().unwrap().price;
    let first_price = breakout_txns.first().unwrap().price;
    let price_sustain_ratio = if breakout_price > 0.0 {
        // 突破区最后价格 / 突破价 > 1 表示维持住
        last_price / breakout_price
    } else {
        1.0
    };

    // ── 评分 ──
    let (confidence, classification, details) = compute_breakout_quality(
        breakout_n,
        breakout_buy_ratio,
        large_count,
        large_buy,
        large_sell,
        price_sustain_ratio,
        breakout_vol_ratio,
    );

    Some(BreakoutResult {
        total_n,
        breakout_n,
        breakout_vol_ratio: (breakout_vol_ratio * 1000.0).round() / 1000.0,
        breakout_buy_vol: buy_vol,
        breakout_sell_vol: sell_vol,
        breakout_buy_ratio: (breakout_buy_ratio * 1000.0).round() / 1000.0,
        breakout_large_count: large_count,
        breakout_large_buy: large_buy,
        breakout_large_sell: large_sell,
        breakout_large_buy_ratio: (breakout_large_buy_ratio * 1000.0).round() / 1000.0,
        breakout_avg_price: (breakout_avg_price * 100.0).round() / 100.0,
        price_sustain_ratio: (price_sustain_ratio * 1000.0).round() / 1000.0,
        is_strong: confidence >= 0.60,
        classification,
        confidence: (confidence * 1000.0).round() / 1000.0,
        details,
    })
}

// ── 评分逻辑 ──

fn compute_breakout_quality(
    breakout_n: usize,
    buy_ratio: f64,
    large_count: i32,
    large_buy: i32,
    large_sell: i32,
    sustain_ratio: f64,
    vol_ratio: f64,
) -> (f64, String, String) {
    let mut score = 0.0_f64;
    let mut strengths: Vec<String> = Vec::new();
    let mut risks: Vec<String> = Vec::new();

    // 1. 买卖力量 (权重 0.35)
    if buy_ratio >= 0.60 {
        strengths.push(format!("买盘主导({:.0}%)", buy_ratio * 100.0));
        score += 1.0 * 0.35;
    } else if buy_ratio >= 0.55 {
        strengths.push(format!("买盘偏强({:.0}%)", buy_ratio * 100.0));
        score += 0.7 * 0.35;
    } else if buy_ratio >= 0.45 {
        score += 0.3 * 0.35; // 均衡
    } else {
        risks.push(format!("卖盘主导({:.0}%)", (1.0 - buy_ratio) * 100.0));
        score += 0.0 * 0.35;
    }

    // 2. 大单方向 (权重 0.30)
    if large_count >= 3 {
        if large_buy > large_sell * 2 {
            strengths.push(format!("大单集中买入({}/{})", large_buy, large_sell));
            score += 1.0 * 0.30;
        } else if large_buy > large_sell {
            strengths.push(format!("大单买多({}/{})", large_buy, large_sell));
            score += 0.7 * 0.30;
        } else if large_buy >= large_sell {
            score += 0.4 * 0.30;
        } else {
            risks.push(format!("大单卖出多于买入({}/{})", large_buy, large_sell));
            score += 0.1 * 0.30;
        }
    } else if large_count >= 1 {
        score += 0.3 * 0.30; // 少量大单
    } else {
        risks.push("突破区无大单".to_string());
        score += 0.1 * 0.30;
    }

    // 3. 价格维持 (权重 0.25)
    if sustain_ratio >= 1.005 {
        strengths.push(format!("价格站稳突破价之上({:.1}%)", (sustain_ratio - 1.0) * 100.0));
        score += 1.0 * 0.25;
    } else if sustain_ratio >= 0.998 {
        score += 0.6 * 0.25; // 基本维持
    } else if sustain_ratio >= 0.99 {
        risks.push(format!("价格回落到突破价附近"));
        score += 0.3 * 0.25;
    } else {
        risks.push(format!("突破后价格回落({:.2})", sustain_ratio));
        score += 0.0 * 0.25;
    }

    // 4. 突破区量能 (权重 0.10)
    if vol_ratio >= 0.30 {
        strengths.push(format!("突破区成交活跃(占比{:.0}%)", vol_ratio * 100.0));
        score += 1.0 * 0.10;
    } else if vol_ratio >= 0.15 {
        score += 0.5 * 0.10;
    } else {
        score += 0.2 * 0.10;
    }

    let confidence = score.clamp(0.0, 1.0);

    let (classification, base_reason) = if confidence >= 0.65 {
        ("强突破".to_string(), "资金积极跟进，大单买入主导".to_string())
    } else if confidence >= 0.45 {
        ("弱突破".to_string(), "突破有资金参与但力度一般".to_string())
    } else if confidence >= 0.30 {
        ("疑似假突破".to_string(), "突破时资金跟随意愿不足".to_string())
    } else {
        ("假突破".to_string(), "突破即高点，资金出逃明显".to_string())
    };

    let strengths_str = if !strengths.is_empty() {
        format!("｜{}", strengths.join("; "))
    } else {
        String::new()
    };
    let risks_str = if !risks.is_empty() {
        format!("｜风险: {}", risks.join("; "))
    } else {
        String::new()
    };

    let details = format!("{}{}{}", base_reason, strengths_str, risks_str);
    (confidence, classification, details)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn t(h: u32, m: u32, s: u32) -> NaiveTime {
        NaiveTime::from_hms_opt(h, m, s).unwrap()
    }

    /// 构建强突破数据：突破后买盘主导 + 大单跟进
    fn make_strong_breakout() -> Vec<TransactionPoint> {
        let mut txns = Vec::new();
        // 突破前交易（价格在突破价之下）
        for i in 0..10 {
            txns.push(TransactionPoint {
                time: t(9, 30 + i, 0),
                price: 100.0 + i as f64 * 0.1,
                vol: 1000,
                trade_count: 2,
                bs_flag: 0,
            });
        }
        // 突破后交易（价格在突破价之上）—— 买盘主导
        for i in 0..15 {
            let price = 101.5 + i as f64 * 0.05;
            let vol = 5000 + i * 1000;
            // 80% 是买入
            let bs_flag = if i % 5 == 0 { 1 } else { 0 };
            txns.push(TransactionPoint {
                time: t(10, 0 + i, 0),
                price,
                vol: vol as i64,
                trade_count: (vol / 2000 + 1) as i32,
                bs_flag,
            });
        }
        txns
    }

    /// 构建假突破数据：突破后卖盘主导 + 价格回落
    fn make_fake_breakout() -> Vec<TransactionPoint> {
        let mut txns = Vec::new();
        // 突破前交易
        for i in 0..10 {
            txns.push(TransactionPoint {
                time: t(9, 30 + i, 0),
                price: 100.0 + i as f64 * 0.1,
                vol: 1000,
                trade_count: 2,
                bs_flag: 0,
            });
        }
        // 突破后交易——先拉升（少量买入）后出货（大量卖出）
        for i in 0..5 {
            let price = 101.5 + i as f64 * 0.10;
            let vol = 3000;
            txns.push(TransactionPoint {
                time: t(10, 0 + i, 0),
                price,
                vol,
                trade_count: 3,
                bs_flag: 0, // 少量买入拉高
            });
        }
        for i in 0..12 {
            let price = 102.0 - i as f64 * 0.08;
            let vol = 8000 + i * 2000;
            txns.push(TransactionPoint {
                time: t(10, 5 + i, 0),
                price: price.max(101.0),
                vol: vol as i64,
                trade_count: (vol / 3000 + 1) as i32,
                bs_flag: 1, // 大量卖出
            });
        }
        txns
    }

    /// 构建无突破数据：价格一直低于突破价
    fn make_no_breakout() -> Vec<TransactionPoint> {
        (0..20)
            .map(|i| TransactionPoint {
                time: t(9, 30 + i, 0),
                price: 99.0 + i as f64 * 0.05,
                vol: 1000,
                trade_count: 2,
                bs_flag: if i % 3 == 0 { 1 } else { 0 },
            })
            .collect()
    }

    #[test]
    fn test_strong_breakout() {
        let txns = make_strong_breakout();
        let result = analyze_breakout(&txns, 101.5).unwrap();
        println!(
            "强突破: n={} breakout_n={} buy_ratio={:.3} large={}/{}/{} sustain={:.3} conf={:.3} class={}",
            result.total_n, result.breakout_n, result.breakout_buy_ratio,
            result.breakout_large_count, result.breakout_large_buy, result.breakout_large_sell,
            result.price_sustain_ratio, result.confidence, result.classification
        );
        assert!(result.is_strong, "强突破数据应判定为强突破");
        assert!(result.confidence >= 0.5);
        assert!(result.breakout_buy_ratio > 0.5);
    }

    #[test]
    fn test_fake_breakout() {
        let txns = make_fake_breakout();
        let result = analyze_breakout(&txns, 101.5).unwrap();
        println!(
            "假突破: n={} breakout_n={} buy_ratio={:.3} large={}/{}/{} sustain={:.3} conf={:.3} class={}",
            result.total_n, result.breakout_n, result.breakout_buy_ratio,
            result.breakout_large_count, result.breakout_large_buy, result.breakout_large_sell,
            result.price_sustain_ratio, result.confidence, result.classification
        );
        assert!(!result.is_strong, "假突破数据不应判定为强突破");
        assert!(result.confidence < 0.6);
    }

    #[test]
    fn test_no_breakout() {
        let txns = make_no_breakout();
        let result = analyze_breakout(&txns, 101.5).unwrap();
        println!(
            "无突破: n={} breakout_n={} class={}",
            result.total_n, result.breakout_n, result.classification
        );
        assert!(result.breakout_n < 5, "无突破数据突破区笔数应很少");
        assert!(!result.is_strong);
    }

    #[test]
    fn test_insufficient_data() {
        let txns = vec![TransactionPoint {
            time: t(9, 30, 0),
            price: 100.0,
            vol: 1000,
            trade_count: 1,
            bs_flag: 0,
        }];
        assert!(analyze_breakout(&txns, 101.0).is_none());
    }
}
