use talib_rs::{math_operator, overlap, volatility, statistic, ma_type::MaType};
use crate::{BarPoint, SignalOutput};
use crate::wavetrend::fill_one_missing;
use std::collections::HashMap;

pub struct TtmSignal {
    pub v1: &'static str,
    pub v2: &'static str,
    pub v3: &'static str,
}

fn compute_ttm_full(
    high: &[f64], low: &[f64], close: &[f64],
    kc_length: usize, kc_mult: f64,
    bb_length: usize, bb_mult: f64,
    _atr_length: usize,
) -> (Vec<f64>, Vec<bool>, Vec<bool>) {
    let n = high.len();

    let hhv = math_operator::max(high, kc_length).unwrap();
    let llv = math_operator::min(low, kc_length).unwrap();
    let ma_close = overlap::sma(close, kc_length).unwrap();

    let avg1: Vec<f64> = hhv.iter().zip(llv.iter()).map(|(h, l)| (h + l) / 2.0).collect();
    let avg2: Vec<f64> = avg1.iter().zip(ma_close.iter()).map(|(a, m)| (a + m) / 2.0).collect();
    let yy: Vec<f64> = close.iter().zip(avg2.iter()).map(|(c, a)| c - a).collect();

    let first_valid = yy.iter().position(|&v| !v.is_nan()).unwrap_or(n);
    let mut ttm = vec![f64::NAN; n];
    if first_valid + kc_length <= n {
        if let Ok(lr) = statistic::linearreg(&yy[first_valid..], kc_length) {
            for (i, &v) in lr.iter().enumerate() {
                ttm[first_valid + i] = v;
            }
        }
    }

    let (bb_upper, _, bb_lower) =
        overlap::bbands(close, bb_length, bb_mult, bb_mult, MaType::Sma).unwrap();
    let atr_vals = volatility::atr(high, low, close, kc_length).unwrap();

    let mut sqz_on = vec![false; n];
    let mut sqz_off = vec![false; n];
    for i in 0..n {
        if bb_upper[i].is_nan() || bb_lower[i].is_nan() || ma_close[i].is_nan() || atr_vals[i].is_nan() {
            continue;
        }
        let kc_u = ma_close[i] + kc_mult * atr_vals[i];
        let kc_l = ma_close[i] - kc_mult * atr_vals[i];
        if bb_lower[i] > kc_l && bb_upper[i] < kc_u {
            sqz_on[i] = true;
        }
        if bb_lower[i] < kc_l && bb_upper[i] > kc_u {
            sqz_off[i] = true;
        }
    }

    (ttm, sqz_on, sqz_off)
}

pub fn get_ttm_signal(
    ttm: &[f64], sqz_on: &[bool], sqz_off: &[bool], di: usize,
) -> Option<TtmSignal> {
    let len = ttm.len();
    if len < di + 3 { return None; }

    let cur = len - di;
    let prev = cur - 1;
    if ttm[cur].is_nan() { return None; }

    let v1 = if ttm[cur] > 0.0 { "多头" } else { "空头" };
    let v2 = classify_ttm_v2(ttm, cur);
    let v3 = classify_ttm_sqz(sqz_on, sqz_off, cur, prev);

    Some(TtmSignal { v1, v2, v3 })
}

fn classify_ttm_v2(ttm: &[f64], cur: usize) -> &'static str {
    if cur < 4 {
        return "其他";
    }
    let c = ttm[cur];
    let p = ttm[cur - 1];
    let pp = ttm[cur - 2];
    let ppp = ttm[cur - 3];

    if c.is_nan() || p.is_nan() || pp.is_nan() || ppp.is_nan() {
        return "其他";
    }

    if p <= 0.0 && c > 0.0 { return "金叉"; }
    if p >= 0.0 && c < 0.0 { return "死叉"; }
    if pp <= 0.0 && p > 0.0 && c > p && c > 0.0 { return "金叉次日"; }
    if pp >= 0.0 && p < 0.0 && c < p && c < 0.0 { return "死叉次日"; }
    if c > 0.0 && c > p && p < pp && c > pp { return "二次上升强势"; }
    if c > 0.0 && c > p && p < pp { return "二次上升"; }
    if c < 0.0 && c < p && p > pp { return "二次下降"; }
    if c > 0.0 && c < p && p > pp && c <= pp { return "正值翻转强势"; }
    if c > 0.0 && c < p && p > pp && c >= ppp { return "正值翻转"; }
    if c < 0.0 && c > p && p < pp && c >= pp { return "负值翻转强势"; }
    if c < 0.0 && c > p && p < pp && pp <= ppp { return "负值翻转"; }
    if p > 0.0 && p < pp && pp > ppp && c < p { return "正值翻转次日"; }
    if p < 0.0 && p > pp && pp < ppp && c > p { return "负值翻转次日"; }

    "其他"
}

fn classify_ttm_sqz(sqz_on: &[bool], _sqz_off: &[bool], cur: usize, prev: usize) -> &'static str {
    let on_cur = *sqz_on.get(cur).unwrap_or(&false);
    let on_prev = *sqz_on.get(prev).unwrap_or(&false);

    if on_cur && !on_prev {
        "转挤压"
    } else if !on_cur && on_prev {
        "转震动"
    } else if on_cur {
        "挤压"
    } else {
        "震动"
    }
}

fn ttm_signal_at(ttm: &[f64], sqz_on: &[bool], sqz_off: &[bool], freq: &str, i: usize, min_bars: usize) -> SignalOutput {
    if i + 1 < min_bars {
        return SignalOutput::with_score("ak_ttm_squeeze_V260711", freq, "其他", "任意", "任意", 0);
    }
    match get_ttm_signal(&ttm[..=i], &sqz_on[..=i], &sqz_off[..=i], 1) {
        Some(s) => SignalOutput::new("ak_ttm_squeeze_V260711", freq, s.v1, s.v2, s.v3),
        None => SignalOutput::with_score("ak_ttm_squeeze_V260711", freq, "其他", "任意", "任意", 0),
    }
}

pub fn compute_ttm_signal(bars: &[BarPoint], params: &HashMap<String, String>) -> Vec<SignalOutput> {
    let n1 = params.get("n1").and_then(|s| s.parse::<usize>().ok()).unwrap_or(20);
    let n2 = params.get("n2").and_then(|s| s.parse::<f64>().ok()).unwrap_or(1.5);
    let n3 = params.get("n3").and_then(|s| s.parse::<usize>().ok()).unwrap_or(20);
    let n4 = params.get("n4").and_then(|s| s.parse::<f64>().ok()).unwrap_or(2.0);
    let n5 = params.get("n5").and_then(|s| s.parse::<usize>().ok()).unwrap_or(n1);
    let freq = bars.first().map(|b| b.freq.clone()).unwrap_or_default();
    let high: Vec<f64> = bars.iter().map(|b| b.high).collect();
    let low: Vec<f64> = bars.iter().map(|b| b.low).collect();
    let close: Vec<f64> = bars.iter().map(|b| b.close).collect();
    let min_bars = n1.max(n3).max(n5) + 5;
    if close.len() < min_bars {
        return fill_one_missing(close.len());
    }
    let (ttm, sqz_on, sqz_off) = compute_ttm_full(&high, &low, &close, n1, n2, n3, n4, n5);
    (0..close.len()).map(|i| ttm_signal_at(&ttm, &sqz_on, &sqz_off, &freq, i, min_bars)).collect()
}
