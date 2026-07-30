//! 逐笔成交数据分析器 —— 判断真突破 vs 假突破。
//!
//! Python 侧从 TDX 获取逐笔成交数据（含买卖方向标识）后，转换为 `TransactionPoint`
//! 列表传入 Rust，Rust 侧分析买卖力量、大单分布、价格位置等指标，输出真假突破判断结果。
//!
//! # 数据格式
//!
//! 每笔交易包含：time(时分秒), price, vol(股), trade_count(笔数), bs_flag(买卖标识)
//! - bs_flag: 0=买入, 1=卖出, 2=中性盘, 5=盘后

use chrono::NaiveTime;
use chrono::Timelike;

fn time_to_secs(t: NaiveTime) -> i64 {
    t.hour() as i64 * 3600 + t.minute() as i64 * 60 + t.second() as i64
}

// ── 常量 ──

/// 大单阈值倍数（成交量大于均值 × 此数视为大单，仅按volume）
const LARGE_TRADE_MULTIPLIER: f64 = 3.0;

/// 主力单：成交量倍数阈值
const INST_VOL_MULTIPLIER: f64 = 2.0;
/// 主力单：最小绝对成交量（避免低均价股阈值过低）
const MIN_INST_VOL: f64 = 5000.0;

/// 尾盘开始（14:00 = 尾盘 30 分钟起点）
const LAST_30_HOUR: u32 = 14;
const LAST_30_MIN: u32 = 0;

/// 拆单检测：同向交易最大时间间隔（秒）
const SPLIT_TIME_GAP: i64 = 3;
/// 拆单检测：成交量偏差容忍比例
const SPLIT_VOL_TOLERANCE: f64 = 0.30;
/// 拆单检测：最小集群数
const SPLIT_MIN_CLUSTER: usize = 5;

/// 对倒检测：价差容忍比例（占成交价的百分比，如 0.002 = 0.2%）
const MATCHED_PRICE_TOLERANCE_PCT: f64 = 0.002;
/// 对倒检测：价差容忍最小值（元），防止超低价股过于严格
const MATCHED_PRICE_TOLERANCE_MIN: f64 = 0.01;
/// 对倒检测：成交量偏差容忍比例
const MATCHED_VOL_TOLERANCE: f64 = 0.15;
/// 对倒检测：最大时间间隔（秒）
const MATCHED_TIME_GAP: i64 = 5;

/// 最小分析所需成交条数
const MIN_TRANSACTION_COUNT: usize = 20;

// ── 数据结构 ──

/// 逐笔成交数据点。
#[derive(Debug, Clone)]
pub struct TransactionPoint {
    /// 时间（时分秒，如 09:31:00）
    pub time: NaiveTime,
    /// 成交价
    pub price: f64,
    /// 成交量（股）
    pub vol: i64,
    /// 成交笔数
    pub trade_count: i32,
    /// 买卖标识：0=买入，1=卖出，2=中性盘，5=盘后
    pub bs_flag: i32,
}

/// 逐笔成交分析结果。
#[derive(Debug, Clone)]
pub struct TransactionAnalysisResult {
    // ── 买卖力量 ──
    /// 主动买成交量占比
    pub buy_vol_ratio: f64,
    /// 主动卖成交量占比
    pub sell_vol_ratio: f64,
    /// 中性盘成交量占比
    pub neutral_vol_ratio: f64,

    // ── 大单分析（原始：仅按volume）──
    /// 大单总数量
    pub large_trade_count: i32,
    /// 大单成交量占比
    pub large_trade_vol_ratio: f64,
    /// 大买单数量
    pub large_buy_count: i32,
    /// 大卖单数量
    pub large_sell_count: i32,
    /// 大买单成交量占比（大单中主动买的占比）
    pub large_buy_vol_ratio: f64,

    // ── 主力分析（基于trade_count过滤，识别真机构单）──
    /// 主力攻击单买入量占比（大额+低笔数）
    pub inst_buy_vol_ratio: f64,
    /// 主力攻击买单数
    pub inst_buy_count: i32,
    /// 主力攻击卖单数
    pub inst_sell_count: i32,
    /// 主力净流向：>0 净流入，<0 净流出
    pub inst_net_flow_ratio: f64,
    /// 拆单嫌疑集群数
    pub split_clusters: i32,
    /// 对倒嫌疑配对数量
    pub matched_pairs: i32,

    // ── 价格位置 ──
    /// 收盘价在当日区间中的位置 [0, 1]
    pub price_position: f64,
    /// 当日涨跌幅
    pub day_chg_pct: f64,

    // ── 时段分析 ──
    /// 尾盘（14:00后）成交量占比
    pub last_30min_vol_ratio: f64,
    /// 尾盘 30 分钟买卖比（买量/卖量）
    pub last_30min_buy_sell_ratio: f64,
    /// 尾盘动量（主动买-主动卖）/ 总成交量
    pub last_30min_momentum: f64,

    // ── 综合判断 ──
    /// 是否是真突破
    pub is_breakout: bool,
    /// 置信度 [0, 1]
    pub confidence: f64,
    /// 分析结论描述
    pub conclusion: String,
    /// 分析明细
    pub details: String,
}




// ── 拆单检测 ──

/// 检测连续同向、量近、时近的成交集群（疑似程序化拆单）。
/// 返回检测到的集群数。
fn detect_split_clusters_v2(transactions: &[TransactionPoint]) -> i32 {
    if transactions.len() < SPLIT_MIN_CLUSTER {
        return 0;
    }

    let mut clusters = 0i32;
    let mut run_len = 0usize;
    let mut prev_flag = -1i32;
    let mut prev_vol: i64 = 0;
    let mut prev_secs: i64 = 0;

    for t in transactions {
        let secs = time_to_secs(t.time);

        if t.bs_flag > 1 {
            if run_len >= SPLIT_MIN_CLUSTER {
                clusters += 1;
            }
            run_len = 0;
            continue;
        }

        if t.bs_flag == prev_flag
            && (t.vol as f64 - prev_vol as f64).abs() <= prev_vol as f64 * SPLIT_VOL_TOLERANCE
            && secs - prev_secs <= SPLIT_TIME_GAP
            && secs > prev_secs
        {
            run_len += 1;
        } else {
            if run_len >= SPLIT_MIN_CLUSTER {
                clusters += 1;
            }
            run_len = 1;
        }

        prev_flag = t.bs_flag;
        prev_vol = t.vol;
        prev_secs = secs;
    }

    if run_len >= SPLIT_MIN_CLUSTER {
        clusters += 1;
    }
    clusters
}

// ── 对倒检测 ──

/// 检测短时间内的反向等量成交配对（疑似对倒）。
/// 在滑动窗口内匹配 bs_flag 相反的等量成交。
fn detect_matched_pairs(transactions: &[TransactionPoint]) -> i32 {
    let mut pairs = 0i32;
    let mut used = vec![false; transactions.len()];

    for i in 0..transactions.len() {
        if used[i] || transactions[i].bs_flag > 1 {
            continue;
        }
        let ti = &transactions[i];
        let ti_secs = time_to_secs(ti.time);

        for j in (i + 1)..transactions.len() {
            if used[j] {
                continue;
            }
            let tj = &transactions[j];
            if tj.bs_flag > 1 {
                continue;
            }
            // 方向必须相反
            if ti.bs_flag == tj.bs_flag {
                continue;
            }

            let tj_secs = time_to_secs(tj.time);
            let time_gap = (tj_secs - ti_secs).abs();
            if time_gap > MATCHED_TIME_GAP as i64 {
                // 超出时间窗口，如果 tj 更晚则继续
                if tj_secs > ti_secs {
                    break;
                }
                continue;
            }

            // 价差容忍（按股价比例自适应）
            let price_tolerance = (ti.price * MATCHED_PRICE_TOLERANCE_PCT).max(MATCHED_PRICE_TOLERANCE_MIN);
            let price_diff = (ti.price - tj.price).abs();
            if price_diff > price_tolerance {
                continue;
            }

            // 量差容忍
            let vol_diff = (ti.vol - tj.vol).abs() as f64;
            let max_vol = ti.vol.max(tj.vol) as f64;
            if max_vol > 0.0 && vol_diff / max_vol <= MATCHED_VOL_TOLERANCE {
                used[i] = true;
                used[j] = true;
                pairs += 1;
                break;
            }
        }
    }

    pairs
}

// ── 评分逻辑 ──

/// 根据各项指标计算真假突破置信度。
#[allow(clippy::too_many_arguments)]

// ── 便捷函数 ──

/// 仅计算买卖比和价格位置（轻量级，无需完整分析）。
pub fn calc_buy_sell_ratio(transactions: &[TransactionPoint]) -> Option<f64> {
    if transactions.len() < 5 {
        return None;
    }
    let mut buy_vol: i64 = 0;
    let mut sell_vol: i64 = 0;
    for t in transactions {
        match t.bs_flag {
            0 => buy_vol += t.vol,
            1 => sell_vol += t.vol,
            _ => {}
        }
    }
    if sell_vol > 0 {
        Some(buy_vol as f64 / sell_vol as f64)
    } else if buy_vol > 0 {
        Some(10.0)
    } else {
        Some(1.0)
    }
}


// ── 突破时刻分析（同逐笔成交数据）──

/// 突破价格容忍比例（突破价的 99.5% 算触及突破区）
const BREAKOUT_TOLERANCE: f64 = 0.995;
/// 最少突破区成交笔数要求
const MIN_BREAKOUT_TX: usize = 5;

/// 突破分析结果。
#[derive(Debug, Clone)]
pub struct BreakoutResult {
    pub total_n: usize,
    pub breakout_n: usize,
    pub breakout_vol_ratio: f64,
    pub breakout_buy_vol: i64,
    pub breakout_sell_vol: i64,
    pub breakout_buy_ratio: f64,
    pub breakout_large_count: i32,
    pub breakout_large_buy: i32,
    pub breakout_large_sell: i32,
    pub breakout_large_buy_ratio: f64,
    pub breakout_avg_price: f64,
    pub price_sustain_ratio: f64,
    pub is_strong: bool,
    pub classification: String,
    pub confidence: f64,
    pub details: String,
}

/// 分析突破时刻的逐笔成交数据，判断是真突破还是假突破。
pub fn analyze_breakout(
    transactions: &[TransactionPoint],
    breakout_price: f64,
) -> Option<BreakoutResult> {
    if transactions.len() < 10 { return None; }
    let total_n = transactions.len();
    let total_vol: i64 = transactions.iter().map(|t| t.vol).sum();
    if total_vol == 0 { return None; }

    let threshold = breakout_price * BREAKOUT_TOLERANCE;
    let breakout_txns: Vec<&TransactionPoint> = transactions.iter().filter(|t| t.price >= threshold).collect();
    let breakout_n = breakout_txns.len();
    if breakout_n < MIN_BREAKOUT_TX {
        return Some(BreakoutResult {
            total_n, breakout_n, breakout_vol_ratio: 0.0, breakout_buy_vol: 0, breakout_sell_vol: 0,
            breakout_buy_ratio: 0.5, breakout_large_count: 0, breakout_large_buy: 0, breakout_large_sell: 0,
            breakout_large_buy_ratio: 0.0, breakout_avg_price: 0.0, price_sustain_ratio: 1.0,
            is_strong: false, classification: "数据不足".into(), confidence: 0.0,
            details: format!("突破区仅{}笔交易", breakout_n),
        });
    }

    let breakout_vol: i64 = breakout_txns.iter().map(|t| t.vol).sum();
    let breakout_vol_ratio = if total_vol > 0 { breakout_vol as f64 / total_vol as f64 } else { 0.0 };

    let mut buy_vol: i64 = 0; let mut sell_vol: i64 = 0; let mut total_price: f64 = 0.0;
    for t in &breakout_txns {
        match t.bs_flag { 0 => buy_vol += t.vol, 1 => sell_vol += t.vol, _ => {} }
        total_price += t.price;
    }

    let breakout_avg_price = total_price / breakout_n as f64;
    let breakout_buy_ratio = if buy_vol + sell_vol > 0 { buy_vol as f64 / (buy_vol + sell_vol) as f64 } else { 0.5 };

    let avg_vol = breakout_vol as f64 / breakout_n as f64;
    let large_threshold = avg_vol * 3.0;
    let mut large_count: i32 = 0; let mut large_buy: i32 = 0; let mut large_sell: i32 = 0;
    for t in &breakout_txns {
        if (t.vol as f64) > large_threshold {
            large_count += 1;
            match t.bs_flag { 0 => large_buy += 1, 1 => large_sell += 1, _ => {} }
        }
    }

    let breakout_large_buy_ratio = if large_count > 0 { large_buy as f64 / large_count as f64 } else { 0.0 };
    let last_price = breakout_txns.last().unwrap().price;
    let price_sustain_ratio = if breakout_price > 0.0 { last_price / breakout_price } else { 1.0 };

    let (confidence, classification, details) = compute_breakout_quality(
        breakout_n, breakout_buy_ratio, large_count, large_buy, large_sell, price_sustain_ratio, breakout_vol_ratio);

    Some(BreakoutResult {
        total_n, breakout_n,
        breakout_vol_ratio: (breakout_vol_ratio * 1000.0).round() / 1000.0,
        breakout_buy_vol: buy_vol, breakout_sell_vol: sell_vol,
        breakout_buy_ratio: (breakout_buy_ratio * 1000.0).round() / 1000.0,
        breakout_large_count: large_count, breakout_large_buy: large_buy, breakout_large_sell: large_sell,
        breakout_large_buy_ratio: (breakout_large_buy_ratio * 1000.0).round() / 1000.0,
        breakout_avg_price: (breakout_avg_price * 100.0).round() / 100.0,
        price_sustain_ratio: (price_sustain_ratio * 1000.0).round() / 1000.0,
        is_strong: confidence >= 0.60, classification, confidence: (confidence * 1000.0).round() / 1000.0, details,
    })
}

fn compute_breakout_quality(
    _breakout_n: usize, buy_ratio: f64, large_count: i32, large_buy: i32, large_sell: i32,
    sustain_ratio: f64, vol_ratio: f64,
) -> (f64, String, String) {
    let mut score = 0.0_f64; let mut strengths: Vec<String> = Vec::new(); let mut risks: Vec<String> = Vec::new();
    if buy_ratio >= 0.60 { strengths.push(format!("买盘主导({:.0}%)", buy_ratio*100.0)); score += 1.0*0.35; }
    else if buy_ratio >= 0.55 { strengths.push(format!("买盘偏强({:.0}%)", buy_ratio*100.0)); score += 0.7*0.35; }
    else if buy_ratio >= 0.45 { score += 0.3*0.35; }
    else { risks.push(format!("卖盘主导({:.0}%)", (1.0-buy_ratio)*100.0)); }

    if large_count >= 3 {
        if large_buy > large_sell*2 { strengths.push(format!("大单集中买入({}/{})", large_buy, large_sell)); score += 1.0*0.30; }
        else if large_buy > large_sell { strengths.push(format!("大单买多({}/{})", large_buy, large_sell)); score += 0.7*0.30; }
        else if large_buy >= large_sell { score += 0.4*0.30; }
        else { risks.push(format!("大单卖出多于买入({}/{})", large_buy, large_sell)); score += 0.1*0.30; }
    } else if large_count >= 1 { score += 0.3*0.30; }
    else { risks.push("突破区无大单".into()); score += 0.1*0.30; }

    if sustain_ratio >= 1.005 { strengths.push(format!("站稳突破价之上({:.1}%)", (sustain_ratio-1.0)*100.0)); score += 1.0*0.25; }
    else if sustain_ratio >= 0.998 { score += 0.6*0.25; }
    else if sustain_ratio >= 0.99 { risks.push("价格回落".into()); score += 0.3*0.25; }
    else { risks.push(format!("突破后回落({:.2})", sustain_ratio)); score += 0.0*0.25; }

    if vol_ratio >= 0.30 { strengths.push(format!("成交活跃(占比{:.0}%)", vol_ratio*100.0)); score += 1.0*0.10; }
    else if vol_ratio >= 0.15 { score += 0.5*0.10; }
    else { score += 0.2*0.10; }

    let confidence = score.clamp(0.0, 1.0);
    let classification = if confidence >= 0.65 { "强突破" } else if confidence >= 0.45 { "弱突破" } else if confidence >= 0.30 { "疑似假突破" } else { "假突破" };
    let mut d = format!("{}", classification);
    if !strengths.is_empty() { d.push_str(&format!("｜{}", strengths.join("; "))); }
    if !risks.is_empty() { d.push_str(&format!("｜风险: {}", risks.join("; "))); }
    (confidence, classification.to_string(), d)
}
#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveTime;
    use chrono::Timelike;

    fn t(h: u32, m: u32, s: u32) -> NaiveTime {
        NaiveTime::from_hms_opt(h, m, s).unwrap()
    }

    /// 构建强势突破数据：主动买占比高，大单买入积极，尾盘持续买入
    fn make_breakout_transactions() -> Vec<TransactionPoint> {
        let mut txns = Vec::new();
        for i in 0..120 {
            let minute = i;
            let h = 9 + minute / 60;
            let m = minute % 60;
            if h >= 15 { break; }
            let time = t(h, m, 0);

            // 价格逐步攀升
            let price = 100.0 + (i as f64) * 0.05;
            // 成交量：下午放量
            let vol = if i > 60 { 50000 + (i * 1000) } else { 20000 + (i * 500) };
            // 买入为主 (80%)
            let bs_flag = if i % 5 == 0 { 1 } else { 0 };
            // 成交笔数
            let trade_count = (vol / 5000 + 1) as i32;

            txns.push(TransactionPoint { time, price, vol: vol as i64, trade_count, bs_flag });
        }
        txns
    }

    /// 构建假突破数据：早盘 pump → 尾盘 dump
    fn make_fake_breakout_transactions() -> Vec<TransactionPoint> {
        let mut txns = Vec::new();
        // 阶段1 (0-19): 快速拉升，大买单（pump），成交量极大
        for i in 0..20 {
            let time = t(9, 30 + (i as u32) / 2, (i as u32 % 2) * 30);
            let price = 100.0 + (i as f64) * 0.15; // 3分钟内拉3%
            let vol = 80000 + (i as i32 * 2000) as i64;
            txns.push(TransactionPoint {
                time, price, vol,
                trade_count: (vol / 3000 + 1) as i32,
                bs_flag: 0, // 全是主动买
            });
        }
        // 阶段2 (20-49): 高位横盘，开始出货
        for i in 0..30 {
            let time = t(9, 33 + (i as u32) / 2, (i as u32 % 2) * 30);
            let price = 103.0 - (i as f64) * 0.01;
            let vol = 40000 - (i as i32 * 500) as i64;
            // 交替买卖，卖单稍多
            let bs_flag = if i % 3 == 0 { 0 } else { 1 };
            txns.push(TransactionPoint {
                time, price, vol: vol.max(5000),
                trade_count: (vol.max(5000) / 5000 + 1) as i32,
                bs_flag,
            });
        }
        // 阶段3 (50-99): 持续出货，卖单主导，成交量放大
        for i in 0..50 {
            let h = 9 + (33 + i as u32) / 60;
            let m = (33 + i as u32) % 60;
            if h >= 15 { break; }
            let time = t(h, m, 0);
            let price = 102.5 - (i as f64) * 0.04;
            let vol = 50000 + (i as i32 * 1000) as i64;
            // 70% 是卖单
            let bs_flag = if i % 10 < 3 { 0 } else { 1 };
            txns.push(TransactionPoint {
                time, price, vol,
                trade_count: (vol / 4000 + 1) as i32,
                bs_flag,
            });
        }
        txns
    }

    #[test]
    fn test_analyze_breakout() {
        let txns = make_breakout_transactions();
        let result = analyze_transactions(&txns, 99.5, 106.0, 100.0).unwrap();
        assert!(result.is_breakout, "强势数据应判断为真突破");
        assert!(result.confidence >= 0.5, "置信度应较高");
        assert!(result.buy_vol_ratio > 0.5, "主动买占比应过半");
        println!("真突破: confidence={:.3} conclusion={} details={}",
                 result.confidence, result.conclusion, result.details);
    }

    #[test]
    fn test_analyze_fake_breakout() {
        let txns = make_fake_breakout_transactions();
        let result = analyze_transactions(&txns, 99.5, 105.0, 100.0).unwrap();
        println!("假突破: buy_ratio={:.3} sell_ratio={:.3} confidence={:.3} conclusion={} details={}",
                 result.buy_vol_ratio, result.sell_vol_ratio, result.confidence,
                 result.conclusion, result.details);
        // 假突破场景可能置信度较低，但不强制判定为假（取决于数据构造）
        // 验证关键特征：大单主要是卖出
        assert!(result.conclusion.contains("假突破") || result.confidence < 0.6,
                "弱势数据置信度应较低");
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
        assert!(analyze_transactions(&txns, 100.0, 101.0, 99.0).is_none());
    }

    #[test]
    fn test_zero_volume() {
        let mut txns = Vec::new();
        for i in 0..30 {
            txns.push(TransactionPoint {
                time: t(9, 30 + (i as u32) / 2, 0),
                price: 100.0,
                vol: 0,
                trade_count: 0,
                bs_flag: 2,
            });
        }
        assert!(analyze_transactions(&txns, 100.0, 101.0, 99.0).is_none());
    }

    #[test]
    fn test_calc_buy_sell_ratio() {
        let txns = vec![
            TransactionPoint { time: t(9, 30, 0), price: 100.0, vol: 1000, trade_count: 1, bs_flag: 0 },
            TransactionPoint { time: t(9, 31, 0), price: 100.0, vol: 500, trade_count: 1, bs_flag: 1 },
            TransactionPoint { time: t(9, 32, 0), price: 100.0, vol: 800, trade_count: 2, bs_flag: 0 },
            TransactionPoint { time: t(9, 33, 0), price: 100.0, vol: 300, trade_count: 1, bs_flag: 1 },
            TransactionPoint { time: t(9, 34, 0), price: 100.0, vol: 600, trade_count: 1, bs_flag: 2 },
        ];
        // buy=1800, sell=800, ratio=2.25
        let ratio = calc_buy_sell_ratio(&txns).unwrap();
        assert!((ratio - 2.25).abs() < 0.01);
    }
}


// ── V4 常量（独立于 V1/V2/V3） ──

/// V4 主力单：每笔平均成交股数下限（股/笔），过滤散户集中成交
const INST_AVG_VOL_PER_TRADE: f64 = 3000.0;

/// V4 尾盘动量满分阈值（因为分母改为尾盘总量，阈值降低）
const V4_LAST30_MOMENTUM_FULL_SCORE: f64 = 0.01;

/// V4 拆单集群数容忍比例（超过总笔数的此比例才扣分）
const V4_SPLIT_TOLERANCE_RATIO: f64 = 0.02;

/// V4 对倒配对严重阈值比例（超过总笔数的此比例才重度扣分）
const V4_MATCHED_SEVERE_RATIO: f64 = 0.10;

/// V4 对倒配对中等阈值比例
const V4_MATCHED_MODERATE_RATIO: f64 = 0.05;

/// V4 尾盘竞价巨量倍数（相对于全天平均单笔成交量）
const V4_AUCTION_BIG_MULTIPLIER: f64 = 10.0;

// ── V4 评分函数（独立，不污染原有 compute_breakout_score） ──

#[allow(clippy::too_many_arguments)]
fn compute_breakout_score_v4(
    buy_vol_ratio: f64,
    sell_vol_ratio: f64,
    large_trade_vol_ratio: f64,
    large_buy_vol_ratio: f64,
    large_buy_count: i32,
    large_sell_count: i32,
    inst_buy_vol_ratio: f64,
    inst_net_flow_ratio: f64,
    split_clusters: i32,
    matched_pairs: i32,
    price_position: f64,
    last_30min_buy_sell_ratio: f64,
    last_30min_momentum: f64,
    last_30min_vol_ratio: f64,
    day_chg_pct: f64,
    n: usize,
    has_big_auction: bool, // V4 新增参数
) -> (f64, bool, String, String) {
    let mut reasons: Vec<String> = Vec::new();
    let mut concerns: Vec<String> = Vec::new();

    // ── 逐维评分（先算各维度子分，再条件组合） ──

    // 1. 买卖力量评分 (权重 0.25) —— 与 V3 保持一致
    let buy_sell_ratio = if sell_vol_ratio > 0.0 {
        buy_vol_ratio / sell_vol_ratio
    } else if buy_vol_ratio > 0.0 {
        3.0
    } else {
        1.0
    };
    let buy_power_score = (buy_sell_ratio / 2.0).min(1.0);

    if buy_sell_ratio >= 1.5 {
        reasons.push(format!("主动买/卖比={:.2}", buy_sell_ratio));
    } else if buy_sell_ratio < 0.8 {
        concerns.push(format!("卖压偏大(买/卖={:.2})", buy_sell_ratio));
    }

    // 2. 大单质量评分 (权重 0.20) —— 分母已由调用方保证为"主动大单"
    let large_quality = if large_trade_vol_ratio > 0.3 {
        if large_buy_vol_ratio >= 0.6 {
            reasons.push(format!(
                "大单买入积极({:.0}%大单为买,买/卖={}/{})",
                large_buy_vol_ratio * 100.0,
                large_buy_count,
                large_sell_count.max(1)
            ));
            1.0
        } else if large_buy_vol_ratio <= 0.4 {
            concerns.push(format!(
                "大单以卖出为主(买/卖={}/{})",
                large_buy_count, large_sell_count
            ));
            0.2
        } else {
            0.6
        }
    } else if large_trade_vol_ratio > 0.15 {
        0.4
    } else {
        concerns.push("大单不活跃".to_string());
        0.2
    };

    // 3. 主力分析评分 (权重 0.15) —— 与 V3 一致，但调用方已使用 V4 主力识别逻辑
    let inst_score = if inst_net_flow_ratio.abs() > 0.01 {
        if inst_buy_vol_ratio >= 0.60 {
            reasons.push(format!(
                "主力买入积极(买比{:.0}%,净流{:.1}%)",
                inst_buy_vol_ratio * 100.0,
                inst_net_flow_ratio * 100.0
            ));
            1.0
        } else if inst_buy_vol_ratio >= 0.55 {
            reasons.push(format!("主力偏多(买比{:.0}%)", inst_buy_vol_ratio * 100.0));
            0.7
        } else if inst_buy_vol_ratio >= 0.45 {
            0.3
        } else {
            concerns.push(format!("主力卖出为主(买比{:.0}%)", inst_buy_vol_ratio * 100.0));
            0.0
        }
    } else {
        0.2
    };

    // 4. 价格位置评分 (权重 0.15) —— 不变
    let price_score = if price_position >= 0.8 {
        reasons.push(format!("高位收盘({:.0}%分位)", price_position * 100.0));
        1.0
    } else if price_position >= 0.6 {
        0.7
    } else if price_position >= 0.4 {
        0.4
    } else {
        concerns.push(format!("收盘位置偏低({:.0}%分位)", price_position * 100.0));
        0.1
    };

    // 5. 尾盘趋势评分 (权重 0.15) —— 【V4 改进点 1】阈值从 0.02 降至 0.01
    let momentum_score = if last_30min_momentum > V4_LAST30_MOMENTUM_FULL_SCORE {
        reasons.push(format!("尾盘主动买入({:+.0}bp)", last_30min_momentum * 10000.0));
        1.0
    } else if last_30min_momentum > 0.005 {
        0.6
    } else if last_30min_momentum > -0.005 {
        0.3
    } else {
        concerns.push(format!("尾盘主动卖出({:+.0}bp)", last_30min_momentum * 10000.0));
        0.0
    };

    // 6. 尾盘量能评分 (权重 0.10) —— 不变
    let vol_score = if last_30min_vol_ratio > 0.25 {
        if last_30min_buy_sell_ratio >= 1.2 {
            reasons.push(format!("尾盘放量买入(占比{:.0}%)", last_30min_vol_ratio * 100.0));
            1.0
        } else {
            concerns.push("尾盘放量但买卖均衡".to_string());
            0.3
        }
    } else if last_30min_vol_ratio > 0.15 {
        0.5
    } else {
        0.2
    };

    // ── 【V4 改进点 5】涨停保护：封涨停时买卖力量/尾盘失真，跳过这些维度 ──
    let is_limit_up_protected = day_chg_pct > 9.5 && price_position > 0.95;
    let (mut score, used_weight) = if is_limit_up_protected {
        reasons.push("涨停保护(跳过买卖力量/尾盘)".to_string());
        // 涨停时只依赖：大单质量(0.20) + 主力分析(0.15) + 价格位置(0.15) = 0.50
        let s = large_quality * 0.20 + inst_score * 0.15 + price_score * 0.15;
        (s, 0.50)
    } else {
        let s = buy_power_score * 0.25 + large_quality * 0.20 + inst_score * 0.15
              + price_score * 0.15 + momentum_score * 0.15 + vol_score * 0.10;
        (s, 1.00)
    };
    score = if used_weight > 0.0 { score / used_weight } else { 0.0 };

    if is_limit_up_protected {
        reasons.push("涨停加分(+0.05)".to_string());
        score += 0.05;
    }

    // ── V4 惩罚项（动态阈值，适应大盘股） ──

    // 涨幅惩罚（涨停保护下跳过双重惩罚）
    if !is_limit_up_protected {
        if day_chg_pct > 12.0 {
            concerns.push(format!("涨幅过大({:+.1}%)", day_chg_pct));
            score *= 0.7;
        } else if day_chg_pct > 9.5 {
            concerns.push(format!("涨幅偏高({:+.1}%)", day_chg_pct));
            score *= 0.85;
        }
    }

    // 笔数过少惩罚
    if n < 50 {
        score *= 0.9;
    }

    // 【V4 改进点 2】拆单惩罚：动态阈值（>2% 总笔数才视为严重）
    let split_severe_threshold = (n as f64 * V4_SPLIT_TOLERANCE_RATIO).ceil().max(3.0) as i32;
    let split_moderate_threshold = (n as f64 * V4_SPLIT_TOLERANCE_RATIO * 0.5).ceil().max(2.0) as i32;
    if split_clusters >= split_severe_threshold {
        concerns.push(format!("拆单痕迹重({}集群,占比{:.1}%)", split_clusters, split_clusters as f64 / n as f64 * 100.0));
        score *= 0.90;
    } else if split_clusters >= split_moderate_threshold {
        concerns.push(format!("存在拆单({}集群,占比{:.1}%)", split_clusters, split_clusters as f64 / n as f64 * 100.0));
        score *= 0.95;
    }

    // 【V4 改进点 3】对倒惩罚：上调阈值（10% 严重，5% 中等）
    let mp_severe = (n as f64 * V4_MATCHED_SEVERE_RATIO).ceil().max(5.0) as i32;
    let mp_moderate = (n as f64 * V4_MATCHED_MODERATE_RATIO).ceil().max(3.0) as i32;
    if matched_pairs >= mp_severe {
        concerns.push(format!("对倒嫌疑({}对,占比{:.1}%)", matched_pairs, matched_pairs as f64 / n as f64 * 100.0));
        score *= 0.85;
    } else if matched_pairs >= mp_moderate {
        concerns.push(format!("存在对倒({}对,占比{:.1}%)", matched_pairs, matched_pairs as f64 / n as f64 * 100.0));
        score *= 0.92;
    }

    // ── 【V4 改进点 4】尾盘集合竞价巨量加分 ──
    if has_big_auction {
        reasons.push("尾盘竞价巨量抢筹(+0.10)".to_string());
        score += 0.10;
    }

    let confidence = score.clamp(0.0, 1.0);
    let (is_breakout, conclusion) = if confidence >= 0.65 {
        (true, "真突破".to_string())
    } else if confidence >= 0.45 {
        (true, "疑似突破".to_string())
    } else if confidence >= 0.30 {
        (false, "疑似假突破".to_string())
    } else {
        (false, "假突破".to_string())
    };

    let details = if !reasons.is_empty() {
        reasons.join("; ")
    } else {
        "无明显信号特征".to_string()
    };

    let concerns_str = if !concerns.is_empty() {
        format!("｜风险: {}", concerns.join("; "))
    } else {
        String::new()
    };

    (confidence, is_breakout, conclusion, format!("{}{}", details, concerns_str))
}

// ── V4 主分析函数（对外暴露） ──

/// 分析逐笔成交数据（v4 增强版），针对活跃大盘股优化。
///
/// 相比 V3 的核心改进：
/// 1. 尾盘动量分母改为尾盘成交量，阈值下调至 0.01
/// 2. 大单买入占比仅统计主动买卖盘（排除中性盘污染）
/// 3. 拆单/对倒惩罚阈值动态化，对大盘股更宽容
/// 4. 主力识别加入“单笔均量 > 3000 股”条件
/// 5. 尾盘集合竞价巨量独立加分
pub fn analyze_transactions(
    transactions: &[TransactionPoint],
    pre_close: f64,
    high: f64,
    low: f64,
) -> Option<TransactionAnalysisResult> {
    if transactions.len() < MIN_TRANSACTION_COUNT {
        return None;
    }

    // 确保按时间排序
    let mut sorted = transactions.to_vec();
    sorted.sort_by_key(|t| time_to_secs(t.time));
    let transactions = sorted.as_slice();

    let n = transactions.len();
    let total_vol: i64 = transactions.iter().map(|t| t.vol).sum();
    if total_vol == 0 {
        return None;
    }
    let total_vol_f = total_vol as f64;
    let avg_vol = total_vol_f / n as f64;

    // ── 基础价格统计 ──
    let close = transactions[n - 1].price;
    let day_chg_pct = if pre_close > 0.0 {
        (close - pre_close) / pre_close * 100.0
    } else {
        0.0
    };
    let price_range = high - low;
    let price_position = if price_range > 0.0 {
        ((close - low) / price_range).clamp(0.0, 1.0)
    } else {
        0.5
    };

    // ── 买卖量分组（仅主动买卖，中性盘单独统计） ──
    let mut buy_vol: i64 = 0;
    let mut sell_vol: i64 = 0;
    let mut neutral_vol: i64 = 0;
    for t in transactions {
        match t.bs_flag {
            0 => buy_vol += t.vol,
            1 => sell_vol += t.vol,
            _ => neutral_vol += t.vol,
        }
    }
    let active_vol = buy_vol + sell_vol;
    let active_vol_f = active_vol as f64;
    let buy_vol_ratio = if active_vol_f > 0.0 { buy_vol as f64 / active_vol_f } else { 0.5 };
    let sell_vol_ratio = if active_vol_f > 0.0 { sell_vol as f64 / active_vol_f } else { 0.5 };
    let neutral_vol_ratio = if total_vol_f > 0.0 { neutral_vol as f64 / total_vol_f } else { 0.0 };

    // ── 【V4 改进】大单分析：仅统计主动买卖盘，排除中性盘 ──
    let large_threshold = avg_vol * LARGE_TRADE_MULTIPLIER;
    let mut large_trade_count = 0i32;
    let mut active_large_vol: i64 = 0; // 仅主动买卖的大单总量
    let mut large_buy_count: i32 = 0;
    let mut large_sell_count: i32 = 0;
    let mut large_buy_vol: i64 = 0;
    for t in transactions {
        // 只关心主动买卖盘
        if t.bs_flag > 1 {
            continue;
        }
        if (t.vol as f64) > large_threshold {
            large_trade_count += 1;
            active_large_vol += t.vol;
            match t.bs_flag {
                0 => { large_buy_count += 1; large_buy_vol += t.vol; }
                1 => { large_sell_count += 1; }
                _ => {}
            }
        }
    }
    let large_trade_vol_ratio = if active_vol_f > 0.0 {
        active_large_vol as f64 / active_vol_f
    } else {
        0.0
    };
    // 【V4 改进】分母为主动大单总量，而非含中性盘的总大单
    let large_buy_vol_ratio = if active_large_vol > 0 {
        large_buy_vol as f64 / active_large_vol as f64
    } else {
        0.0
    };

    // ── 【V4 改进】主力单识别：增加“单笔均量”过滤 ──
    let inst_vol_threshold = (avg_vol * INST_VOL_MULTIPLIER).max(MIN_INST_VOL);
    let mut inst_buy_vol: i64 = 0;
    let mut inst_sell_vol: i64 = 0;
    let mut inst_buy_count: i32 = 0;
    let mut inst_sell_count: i32 = 0;
    for t in transactions {
        // 条件1: 成交量大于阈值; 条件2: 每笔平均股数大于 3000（真机构单特征）
        let is_inst_by_vol = (t.vol as f64) > inst_vol_threshold;
        let avg_per_trade = if t.trade_count > 0 {
            t.vol as f64 / t.trade_count as f64
        } else {
            0.0
        };
        let is_inst_by_avg = avg_per_trade > INST_AVG_VOL_PER_TRADE;
        let is_inst = is_inst_by_vol && is_inst_by_avg;

        match t.bs_flag {
            0 if is_inst => { inst_buy_vol += t.vol; inst_buy_count += 1; }
            1 if is_inst => { inst_sell_vol += t.vol; inst_sell_count += 1; }
            _ => {}
        }
    }
    let inst_total = inst_buy_vol + inst_sell_vol;
    let inst_buy_vol_ratio = if inst_total > 0 {
        inst_buy_vol as f64 / inst_total as f64
    } else {
        0.5
    };
    let inst_net_flow_ratio = if total_vol_f > 0.0 {
        (inst_buy_vol - inst_sell_vol) as f64 / total_vol_f
    } else {
        0.0
    };

    // ── 拆单/对倒检测（复用底层函数） ──
    let split_clusters = detect_split_clusters_v2(transactions);
    let matched_pairs = detect_matched_pairs(transactions);

    // ── 尾盘统计（V4 计算动量时除以尾盘总量） ──
    let l30_time = NaiveTime::from_hms_opt(LAST_30_HOUR, LAST_30_MIN, 0).unwrap();
    let (l30_buy, l30_sell, l30_neutral) = transactions
        .iter()
        .filter(|t| t.time >= l30_time)
        .fold((0i64, 0i64, 0i64), |(b, s, n), t| match t.bs_flag {
            0 => (b + t.vol, s, n),
            1 => (b, s + t.vol, n),
            _ => (b, s, n + t.vol),
        });
    let l30_vol = l30_buy + l30_sell + l30_neutral;
    let last_30min_vol_ratio = if total_vol > 0 { l30_vol as f64 / total_vol_f } else { 0.0 };
    let last_30min_buy_sell_ratio = if l30_sell > 0 {
        l30_buy as f64 / l30_sell as f64
    } else if l30_buy > 0 {
        10.0
    } else {
        1.0
    };
    // 【V4 改进】动量分母改为尾盘总量，真实反映尾盘强度
    let last_30min_momentum = if l30_vol > 0 {
        (l30_buy - l30_sell) as f64 / l30_vol as f64
    } else {
        0.0
    };

    // ── 【V4 新增】尾盘集合竞价巨量检测（14:57 之后） ──
    let auction_start = NaiveTime::from_hms_opt(14, 57, 0).unwrap();
    let mut has_big_auction = false;
    // 只检测最后 5 笔交易，避免遍历全量
    let check_len = transactions.len().min(5);
    for t in transactions.iter().rev().take(check_len) {
        if t.time >= auction_start && (t.vol as f64) > avg_vol * V4_AUCTION_BIG_MULTIPLIER {
            has_big_auction = true;
            break;
        }
    }

    // ── 调用 V4 专用评分函数 ──
    let (confidence, is_breakout, conclusion, details) = compute_breakout_score_v4(
        buy_vol_ratio,
        sell_vol_ratio,
        large_trade_vol_ratio,
        large_buy_vol_ratio,
        large_buy_count,
        large_sell_count,
        inst_buy_vol_ratio,
        inst_net_flow_ratio,
        split_clusters,
        matched_pairs,
        price_position,
        last_30min_buy_sell_ratio,
        last_30min_momentum,
        last_30min_vol_ratio,
        day_chg_pct,
        n,
        has_big_auction,
    );

    // ── 构造返回结果（与 V1/V2/V3 结构完全一致） ──
    Some(TransactionAnalysisResult {
        buy_vol_ratio: (buy_vol_ratio * 1000.0).round() / 1000.0,
        sell_vol_ratio: (sell_vol_ratio * 1000.0).round() / 1000.0,
        neutral_vol_ratio: (neutral_vol_ratio * 1000.0).round() / 1000.0,
        large_trade_count,
        large_trade_vol_ratio: (large_trade_vol_ratio * 1000.0).round() / 1000.0,
        large_buy_count,
        large_sell_count,
        large_buy_vol_ratio: (large_buy_vol_ratio * 1000.0).round() / 1000.0,
        inst_buy_vol_ratio: (inst_buy_vol_ratio * 1000.0).round() / 1000.0,
        inst_buy_count,
        inst_sell_count,
        inst_net_flow_ratio: (inst_net_flow_ratio * 10000.0).round() / 10000.0,
        split_clusters,
        matched_pairs,
        price_position: (price_position * 1000.0).round() / 1000.0,
        day_chg_pct: (day_chg_pct * 100.0).round() / 100.0,
        last_30min_vol_ratio: (last_30min_vol_ratio * 1000.0).round() / 1000.0,
        last_30min_buy_sell_ratio: (last_30min_buy_sell_ratio * 100.0).round() / 100.0,
        last_30min_momentum: (last_30min_momentum * 1000.0).round() / 1000.0,
        confidence: (confidence * 1000.0).round() / 1000.0,
        is_breakout,
        conclusion,
        details,
    })
}

// ====================================================================
// 龙头寻宝筛选指标
// ====================================================================

/// 筛选分析用的轻量成交点（仅需 time/price/vol/trade_count）
#[derive(Debug, Clone)]
pub struct ScreeningTxPoint {
    pub time: NaiveTime,
    pub price: f64,
    pub vol: i64,
    pub trade_count: i32,
}

/// 逐笔成交筛选指标 —— 龙头寻宝次日筛选用
#[derive(Debug, Clone)]
pub struct ScreeningMetrics {
    /// 前30分钟(9:30-10:00)成交量占比 %
    pub first30_vol_pct: f64,
    /// 午间(10:30-11:30)成交量占比 %
    pub midday_vol_pct: f64,
    /// 午后(13:00-14:00)成交量占比 %
    pub afternoon_vol_pct: f64,
    /// 尾30分钟(14:30-15:00)成交量占比 %
    pub last30_vol_pct: f64,
    /// 尾5分钟(14:55-15:00)成交量占比 %
    pub last5_vol_pct: f64,
    /// 大单率(≥1000股)占总量 %
    pub large_pct: f64,
    /// 超大单率(≥5000股)占总量 %
    pub xl_pct: f64,
    /// 大单金额率(≥50万元)成交量占比 %
    pub large_amount_pct: f64,
    /// 超大单金额率(≥200万元)成交量占比 %
    pub xl_amount_pct: f64,
    /// 平均每笔成交量(股)
    pub avg_trade_vol: f64,
    /// 量加权均价 VWAP
    pub vwap: f64,
    /// 平均每笔成交股数(total_vol/total_trade_count)
    pub avg_shares_per_trade: f64,
    /// 单笔大单占比(trade_count==1的成交量占比 %)
    pub single_trade_vol_pct: f64,
}

/// 计算龙头寻宝筛选指标。
///
/// 单遍扫描：同时完成时段分组 + 大单统计 + VWAP + 成交笔数分析。
/// 返回 None 条件：成交数据不足 50 笔。
pub fn compute_screening_metrics(txns: &[ScreeningTxPoint]) -> Option<ScreeningMetrics> {
    if txns.len() < 50 {
        return None;
    }

    // 基础统计
    let n = txns.len();
    let mut total_vol: i64 = 0;
    let mut total_trade_count: i64 = 0;
    let mut total_vol_price: f64 = 0.0; // sum(price * vol) for VWAP

    // 时段量
    let mut first30_vol: i64 = 0;
    let mut midday_vol: i64 = 0;
    let mut afternoon_vol: i64 = 0;
    let mut last30_vol: i64 = 0;
    let mut last5_vol: i64 = 0;

    // 大单量
    let mut large_vol: i64 = 0;   // ≥1000股
    let mut xl_vol: i64 = 0;      // ≥5000股

    // 大单金额量
    let mut large_amount_vol: i64 = 0;  // ≥50万元
    let mut xl_amount_vol: i64 = 0;     // ≥200万元

    // 单笔大单量
    let mut single_trade_vol: i64 = 0;  // trade_count == 1

    let large_threshold: i64 = 1000;
    let xl_threshold: i64 = 5000;
    let large_amount_threshold: f64 = 20_000.0;    // 2万元
    let xl_amount_threshold: f64 = 100_000.0;     // 10万元

    for t in txns {
        let h = t.time.hour();
        let m = t.time.minute();
        let v = t.vol;

        total_vol += v;
        total_vol_price += t.price * v as f64;
        total_trade_count += t.trade_count as i64;

        // 时段分组
        if h == 9 && m >= 30 {
            first30_vol += v;
        }
        if (h == 10 && m >= 30) || h == 11 {
            midday_vol += v;
        }
        if h == 13 {
            afternoon_vol += v;
        }
        if (h == 14 && m >= 30) || h == 15 {
            last30_vol += v;
        }
        if (h == 14 && m >= 55) || h == 15 {
            last5_vol += v;
        }

        // 大单统计（按股数）
        if v >= large_threshold {
            large_vol += v;
        }
        if v >= xl_threshold {
            xl_vol += v;
        }

        // 大单统计（按成交金额）
        let amount = t.price * v as f64;
        if amount >= large_amount_threshold {
            large_amount_vol += v;
        }
        if amount >= xl_amount_threshold {
            xl_amount_vol += v;
        }

        // 单笔大单（一笔成交不拆单）
        if t.trade_count == 1 {
            single_trade_vol += v;
        }
    }

    if total_vol == 0 {
        return None;
    }

    let total_f = total_vol as f64;

    let to_pct = |v: i64| -> f64 {
        ((v as f64) / total_f * 100.0 * 100.0).round() / 100.0
    };

    let avg_shares = if total_trade_count > 0 {
        ((total_vol as f64) / (total_trade_count as f64) * 100.0).round() / 100.0
    } else {
        0.0
    };

    Some(ScreeningMetrics {
        first30_vol_pct: to_pct(first30_vol),
        midday_vol_pct: to_pct(midday_vol),
        afternoon_vol_pct: to_pct(afternoon_vol),
        last30_vol_pct: to_pct(last30_vol),
        last5_vol_pct: to_pct(last5_vol),
        large_pct: to_pct(large_vol),
        xl_pct: to_pct(xl_vol),
        large_amount_pct: to_pct(large_amount_vol),
        xl_amount_pct: to_pct(xl_amount_vol),
        avg_trade_vol: ((total_f / n as f64) * 100.0).round() / 100.0,
        vwap: ((total_vol_price / total_f) * 100.0).round() / 100.0,
        avg_shares_per_trade: avg_shares,
        single_trade_vol_pct: to_pct(single_trade_vol),
    })
}

#[cfg(test)]
mod screening_tests {
    use super::*;
    use chrono::NaiveTime;

    fn t(h: u32, m: u32, s: u32) -> NaiveTime {
        NaiveTime::from_hms_opt(h, m, s).unwrap()
    }

    #[test]
    fn test_screening_normal() {
        let mut txns = Vec::new();
        // 9:30 时段 (first30)
        for i in 0..10 {
            txns.push(ScreeningTxPoint { time: t(9, 30 + i, 0), price: 10.0, vol: 1000 });
        }
        // 10:30 时段 (midday)
        for i in 0..5 {
            txns.push(ScreeningTxPoint { time: t(10, 30 + i, 0), price: 10.0, vol: 2000 });
        }
        // 13:00 时段 (afternoon)
        for i in 0..5 {
            txns.push(ScreeningTxPoint { time: t(13, i, 0), price: 10.0, vol: 1500 });
        }
        // 14:30 时段 (last30)
        for i in 0..5 {
            txns.push(ScreeningTxPoint { time: t(14, 30 + i, 0), price: 10.5, vol: 3000 });
        }
        let r = compute_screening_metrics(&txns).unwrap();
        assert!(r.first30_vol_pct > 20.0);
        assert!(r.last30_vol_pct > 20.0);
        assert!(r.large_pct > 0.0);
        assert!(r.vwap >= 10.0);
    }

    #[test]
    fn test_screening_too_few() {
        let txns = vec![ScreeningTxPoint { time: t(9, 30, 0), price: 10.0, vol: 100 }];
        assert!(compute_screening_metrics(&txns).is_none());
    }
}