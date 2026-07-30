//! 逐笔成交筛选指标：分时段成交占比、大单/特大单占比、加权均价等。

use chrono::Timelike;
use crate::transaction::types::{ScreeningTxPoint, ScreeningMetrics};

/// 计算逐笔成交筛选指标。
pub fn compute_screening_metrics(txns: &[ScreeningTxPoint]) -> Option<ScreeningMetrics> {
    let n = txns.len();
    if n < 5 {
        return None;
    }

    let avg_vol: f64 = txns.iter().map(|t| t.vol as f64).sum::<f64>() / n as f64;
    let large_threshold = (avg_vol * 3.0) as i64;
    let xl_threshold = (avg_vol * 10.0) as i64;
    let avg_amount: f64 = txns.iter().map(|t| t.price * t.vol as f64).sum::<f64>() / n as f64;
    let large_amount_threshold = avg_amount * 3.0;
    let xl_amount_threshold = avg_amount * 10.0;

    let mut first30_vol: i64 = 0;
    let mut midday_vol: i64 = 0;
    let mut afternoon_vol: i64 = 0;
    let mut last30_vol: i64 = 0;
    let mut last5_vol: i64 = 0;
    let mut large_vol: i64 = 0;
    let mut xl_vol: i64 = 0;
    let mut large_amount_vol: i64 = 0;
    let mut xl_amount_vol: i64 = 0;
    let mut total_vol: i64 = 0;
    let mut total_vol_price: f64 = 0.0;
    let mut total_trade_count: i64 = 0;
    let mut single_trade_vol: i64 = 0;

    for t in txns {
        let h = t.time.hour();
        let m = t.time.minute();
        let v = t.vol;

        total_vol += v;
        total_vol_price += t.price * v as f64;
        total_trade_count += t.trade_count as i64;

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

        if v >= large_threshold {
            large_vol += v;
        }
        if v >= xl_threshold {
            xl_vol += v;
        }

        let amount = t.price * v as f64;
        if amount >= large_amount_threshold {
            large_amount_vol += v;
        }
        if amount >= xl_amount_threshold {
            xl_amount_vol += v;
        }

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
mod tests {
    use super::*;
    use chrono::NaiveTime;

    fn t(h: u32, m: u32, s: u32) -> NaiveTime {
        NaiveTime::from_hms_opt(h, m, s).unwrap()
    }

    #[test]
    fn test_screening_normal() {
        let mut txns = Vec::new();
        for i in 0..10 {
            txns.push(ScreeningTxPoint { time: t(9, 30 + i, 0), price: 10.0, vol: 1000 });
        }
        for i in 0..5 {
            txns.push(ScreeningTxPoint { time: t(10, 30 + i, 0), price: 10.0, vol: 2000 });
        }
        for i in 0..5 {
            txns.push(ScreeningTxPoint { time: t(13, i, 0), price: 10.0, vol: 1500 });
        }
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
