//! ak_ttm_squeeze_V260711 — TTM Squeeze 方向/交叉/挤压三维状态信号
//!
//! v1: 多头(TTM>0) / 空头(TTM<0)
//! v2: 金叉/死叉/二次上升/二次下降/正值翻转/负值翻转/...（12种交叉事件）
//! v3: 挤压/震动/转挤压/转震动（4种BB/KC挤压状态）

use talib_rs::{math_operator, overlap, volatility, statistic, ma_type::MaType};
use ak_core::ta;

pub struct TtmSignal {
    pub v1: &'static str,
    pub v2: &'static str,
    pub v3: &'static str,
}

/// 从价格序列计算 TTM 全量结果
///
/// Returns: (ttm序列, sqz_on, sqz_off)
pub fn compute_ttm(
    high: &[f64], low: &[f64], close: &[f64],
    kc_length: usize, kc_mult: f64,
    bb_length: usize, bb_mult: f64,
    atr_length: usize,
) -> Option<(Vec<f64>, Vec<bool>, Vec<bool>)> {
    let n = high.len();
    let min_bars = kc_length.max(bb_length).max(atr_length) + 5;
    if n < min_bars {
        return None;
    }

    let hhv = math_operator::max(high, kc_length).ok()?;
    let llv = math_operator::min(low, kc_length).ok()?;
    let ma_close = ta::sma(close, kc_length);

    let avg1: Vec<f64> = hhv.iter().zip(llv.iter()).map(|(h, l)| (h + l) / 2.0).collect();
    let avg2: Vec<f64> = avg1.iter().zip(ma_close.iter()).map(|(a, m)| (a + m) / 2.0).collect();
    let yy: Vec<f64> = close.iter().zip(avg2.iter()).map(|(c, a)| c - a).collect();

    let fv = yy.iter().position(|&v| !v.is_nan()).unwrap_or(n);
    let mut ttm = vec![f64::NAN; n];
    if fv + kc_length <= n {
        if let Ok(lr) = statistic::linearreg(&yy[fv..], kc_length) {
            for (i, &v) in lr.iter().enumerate() {
                ttm[fv + i] = v;
            }
        }
    }

    let (bb_upper, _, bb_lower) = overlap::bbands(close, bb_length, bb_mult, bb_mult, MaType::Sma).ok()?;
    let atr_vals = volatility::atr(high, low, close, atr_length).ok()?;

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

    Some((ttm, sqz_on, sqz_off))
}

/// 从 TTM 序列和索引生成信号值
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
    if cur < 3 { return "其他"; }
    let c = ttm[cur];
    let p = ttm[cur - 1];
    let pp = ttm[cur - 2];
    let ppp = ttm[cur - 3];
    if c.is_nan() || p.is_nan() || pp.is_nan() || ppp.is_nan() { return "其他"; }

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
    if cur >= sqz_on.len() || prev >= sqz_on.len() { return "其他"; }
    let on_cur = sqz_on[cur];
    let on_prev = sqz_on[prev];
    if on_cur && !on_prev { "转挤压" }
    else if !on_cur && on_prev { "转震动" }
    else if on_cur { "挤压" }
    else { "震动" }
}
