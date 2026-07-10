//! 集合竞价七种典型形态的自动分类。
//!
//! 分类算法基于 tick 序列的价格轨迹、量能变化、买卖盘对比进行无监督聚类。

use crate::AuctionTick;

/// 七种竞价形态的名称。
pub const MORPHOLOGY_NAMES: [&str; 7] = [
    "涨停试盘型",   // 早段触板 → 回落
    "阶梯攀升型",   // 逐级上移 + 放量
    "钓鱼线型",     // 9:20 前高位 → 骤降
    "跌停撬板型",   // 早段触跌停 → 回升
    "一字放量型",   // 价格不动 + 大量成交堆积
    "无量一字型",   // 价格不动 + 地量
    "震荡分歧型",   // 反复波动 + 方向不明
];

/// 对 tick 序列进行七种竞价形态分类。
pub fn classify_morphology(
    recs: &[AuctionTick],
    limit_up: f64,
    limit_down: f64,
) -> (String, f64, [f64; 7], usize) {
    let scores = [
        m1_limit_up_test(recs, limit_up),
        m2_step_climbing(recs),
        m3_fishing_line(recs),
        m4_limit_down_reversal(recs, limit_down),
        m5_flat_high_vol(recs),
        m6_low_volume_flat(recs),
        m7_oscillation(recs),
    ];

    let best_idx = scores
        .iter()
        .enumerate()
        .max_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap())
        .map(|(i, _)| i)
        .unwrap_or(6);

    let best_score = scores[best_idx];
    let (best_idx, mut name, conf) = if best_score < 0.2 {
        (6usize, MORPHOLOGY_NAMES[6].to_string(), scores[6])
    } else {
        (best_idx, MORPHOLOGY_NAMES[best_idx].to_string(), best_score)
    };

    // 一字放量型/无量一字型区分方向
    if (best_idx == 4 || best_idx == 5) && limit_up > 0.0 && limit_down > 0.0 && !recs.is_empty() {
        let avg_p: f64 = recs.iter().map(|t| t.price).sum::<f64>() / recs.len() as f64;
        let up_dist = (avg_p - limit_up).abs();
        let dn_dist = (avg_p - limit_down).abs();
        if up_dist < dn_dist && avg_p >= limit_up * 0.98 {
            name.push_str("(多头)");
        } else if dn_dist < up_dist && avg_p <= limit_down * 1.02 {
            name.push_str("(空头)");
        } else {
            name.push_str("(中性)");
        }
    }
    (name, conf, scores, best_idx)
}

fn flat_price_stats(recs: &[AuctionTick]) -> (f64, f64) {
    let min_p = recs.iter().map(|t| t.price).fold(f64::MAX, f64::min);
    let max_p = recs.iter().map(|t| t.price).fold(f64::MIN, f64::max);
    let range = (max_p - min_p) / min_p * 100.0;
    let total_vol: i32 = recs.iter().map(|t| t.matched).sum();
    let avg_vol = total_vol as f64 / recs.len() as f64;
    let range_score = if range < 0.3 { 1.0 } else if range < 1.0 { 0.5 } else { 0.0 };
    (range_score, avg_vol)
}

fn m1_limit_up_test(recs: &[AuctionTick], limit_up: f64) -> f64 {
    if limit_up <= 0.0 || recs.len() < 8 { return 0.0; }
    let mid = recs.len() / 2;
    let early_max = recs[..mid].iter().map(|t| t.price).max_by(|a,b| a.partial_cmp(b).unwrap()).unwrap_or(0.0);
    let final_price = recs.last().unwrap().price;
    let near_limit = early_max >= limit_up * 0.96;
    if !near_limit { return 0.0; }
    let drop = (early_max - final_price) / limit_up;
    if drop <= 0.0 { return 0.3; }
    let peak_vol: i32 = recs[mid..].iter().filter(|t| t.price >= limit_up * 0.96).map(|t| t.matched).sum();
    let total_vol: i32 = recs.iter().map(|t| t.matched).sum();
    let vol_ratio = peak_vol as f64 / total_vol.max(1) as f64;
    0.4 * (drop / 0.05).min(1.0) + 0.3 * vol_ratio.min(1.0) + 0.3 * if drop >= 0.02 { 1.0 } else { 0.0 }
}

fn m2_step_climbing(recs: &[AuctionTick]) -> f64 {
    if recs.len() < 8 { return 0.0; }
    if recs.last().unwrap().price <= recs.first().unwrap().price { return 0.0; }
    let mut seen = Vec::new();
    for r in recs { let p = (r.price * 100.0 + 0.5).floor() as i64; if !seen.contains(&p) { seen.push(p); } }
    let steps = if seen.is_empty() { 0 } else { seen.len() - 1 };
    let step_score = ((steps as f64 - 1.0) / 4.0).min(1.0).max(0.0);
    let mid = recs.len() / 2;
    let early_vol: i32 = recs[..mid].iter().map(|t| t.matched).sum();
    let late_vol: i32 = recs[mid..].iter().map(|t| t.matched).sum();
    let vol_growth = if early_vol > 0 { late_vol as f64 / early_vol as f64 } else { 1.0 };
    let max_drop = recs.windows(2).map(|w| (w[1].price-w[0].price)/w[0].price).filter(|&d| d<0.0).map(|d| d.abs()).fold(0.0_f64, f64::max);
    0.4 * step_score + 0.3 * (vol_growth / 2.0).min(1.0) + 0.3 * if max_drop < 0.01 { 1.0 } else { 0.5 }
}

fn m3_fishing_line(recs: &[AuctionTick]) -> f64 {
    if recs.len() < 8 { return 0.0; }
    let t920 = chrono::NaiveTime::from_hms_opt(9,20,0).unwrap();
    let before: Vec<&AuctionTick> = recs.iter().filter(|t| t.time < t920).collect();
    let after: Vec<&AuctionTick> = recs.iter().filter(|t| t.time >= t920).collect();
    if before.is_empty() || after.is_empty() { return 0.0; }
    let high_before = before.iter().map(|t| t.price).max_by(|a,b| a.partial_cmp(b).unwrap()).unwrap();
    let price_after: f64 = after.iter().take(3).map(|t| t.price).sum::<f64>() / 3.0;
    let drop = (high_before - price_after) / high_before;
    if drop < 0.015 { return 0.0; }
    (drop / 0.05).min(1.0) * 0.7 + 0.3
}

fn m4_limit_down_reversal(recs: &[AuctionTick], limit_down: f64) -> f64 {
    if limit_down <= 0.0 || recs.len() < 8 { return 0.0; }
    let mid = recs.len() / 2;
    let early_min = recs[..mid].iter().map(|t| t.price).min_by(|a,b| a.partial_cmp(b).unwrap()).unwrap();
    let final_price = recs.last().unwrap().price;
    let near_ld = early_min <= limit_down * 1.04;
    if !near_ld { return 0.0; }
    let recovery = (final_price - early_min) / limit_down;
    if recovery <= 0.0 { return 0.2; }
    let buy_um: i32 = recs.iter().map(|t| t.unmatched.max(0)).sum();
    let sell_um: i32 = recs.iter().map(|t| (-t.unmatched).max(0)).sum();
    0.5 * (recovery / 0.05).min(1.0) + 0.3 * if buy_um > sell_um { 1.0 } else { 0.3 } + 0.2 * if recovery >= 0.02 { 1.0 } else { 0.0 }
}

/// 一字放量型：价格不动 + 量大
fn m5_flat_high_vol(recs: &[AuctionTick]) -> f64 {
    if recs.is_empty() { return 0.0; }
    let (range_score, avg_vol) = flat_price_stats(recs);
    let vol_score = if avg_vol >= 200.0 { 1.0 } else if avg_vol >= 50.0 { 0.5 } else { 0.0 };
    0.6 * range_score + 0.4 * vol_score
}

/// 无量一字型：价格不动 + 地量
fn m6_low_volume_flat(recs: &[AuctionTick]) -> f64 {
    if recs.is_empty() { return 0.0; }
    let (range_score, avg_vol) = flat_price_stats(recs);
    let vol_score = if avg_vol < 50.0 { 1.0 } else if avg_vol < 200.0 { 0.5 } else { 0.0 };
    0.6 * range_score + 0.4 * vol_score
}

fn m7_oscillation(recs: &[AuctionTick]) -> f64 {
    if recs.len() < 8 { return 0.0; }
    let (first_p, last_p) = (recs.first().unwrap().price, recs.last().unwrap().price);
    let min_p = recs.iter().map(|t| t.price).fold(f64::MAX, f64::min);
    let max_p = recs.iter().map(|t| t.price).fold(f64::MIN, f64::max);
    let range = (max_p - min_p) / min_p * 100.0;
    if range < 1.5 || range > 8.0 { return 0.0; }
    let oscillation_ratio = if range > 0.0 { (last_p-first_p).abs()/first_p*100.0 / range } else { 1.0 };
    let direction_changes = recs.windows(2).filter(|w| { let (s0,s1)=(w[0].unmatched.signum(),w[1].unmatched.signum()); s0!=0&&s1!=0&&s0!=s1 }).count();
    let change_rate = direction_changes as f64 / recs.len() as f64;
    0.3 * (range/4.0).min(1.0) + 0.4 * if oscillation_ratio<0.3{1.0}else{(0.5-oscillation_ratio*0.5).max(0.0)} + 0.3 * (change_rate*5.0).min(1.0)
}

/// 根据形态编码返回 (组名, 编制代码)
pub fn morphology_group_tag(code: usize) -> (&'static str, &'static str) {
    match code {
        0 => ("B",  "B01"),  // 涨停试盘型
        1 => ("A",  "A01"),  // 阶梯攀升型
        2 => ("A",  "A02"),  // 钓鱼线型
        3 => ("B",  "B02"),  // 跌停撬板型
        4 => ("YZ", "YZ01"), // 一字放量型
        5 => ("YZ", "YZ02"), // 无量一字型
        6 => ("ZD", "ZD01"), // 震荡分歧型
        _ => ("?",  "?"),
    }
}
