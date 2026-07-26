//! ak_ic_state_V260718 — IC 云图位置/云型/线态三维状态信号
//!
//! v1: 云上/云下/云内/突破上/跌破下/测云顶/测云底/贯穿云（8种价格位置）
//! v2: 上升云/下降云（A vs B）
//! v3: 双强/双高/高影/线内/低影/双低/双弱/CL金叉/CL死叉（8种线态+交叉）

use talib_rs::math_operator;

pub struct IcSignal {
    pub v1: &'static str,
    pub v2: &'static str,
    pub v3: &'static str,
    pub score: i32,
}

/// 从高低价序列计算 CL/DL/A/B 云图四线
pub fn compute_ic_lines(high: &[f64], low: &[f64], n1: usize, n2: usize, n3: usize) -> Option<(
    Vec<f64>, Vec<f64>, Vec<f64>, Vec<f64>,
)> {
    let n = high.len();
    if n < n3 + n2 + 5 {
        return None;
    }

    let hhv_short = math_operator::max(high, n1).ok()?;
    let llv_short = math_operator::min(low, n1).ok()?;
    let hhv_mid = math_operator::max(high, n2).ok()?;
    let llv_mid = math_operator::min(low, n2).ok()?;
    let hhv_long = math_operator::max(high, n3).ok()?;
    let llv_long = math_operator::min(low, n3).ok()?;

    let mut cl = vec![f64::NAN; n];
    let mut dl = vec![f64::NAN; n];
    for i in 0..n {
        if !hhv_short[i].is_nan() && !llv_short[i].is_nan() {
            cl[i] = (hhv_short[i] + llv_short[i]) / 2.0;
        }
        if !hhv_mid[i].is_nan() && !llv_mid[i].is_nan() {
            dl[i] = (hhv_mid[i] + llv_mid[i]) / 2.0;
        }
    }

    let mut a = vec![f64::NAN; n];
    let mut b = vec![f64::NAN; n];
    for i in n2..n {
        let src = i - n2;
        if !cl[src].is_nan() && !dl[src].is_nan() {
            a[i] = (cl[src] + dl[src]) / 2.0;
        }
        if !hhv_long[src].is_nan() && !llv_long[src].is_nan() {
            b[i] = (hhv_long[src] + llv_long[src]) / 2.0;
        }
    }

    Some((cl, dl, a, b))
}

pub fn compute_ic_state(
    high: &[f64], low: &[f64], close: &[f64],
    n1: usize, n2: usize, n3: usize, di: usize,
) -> Option<IcSignal> {
    let (cl, dl, a, b) = compute_ic_lines(high, low, n1, n2, n3)?;
    let n = high.len();
    if n < di + 1 { return None; }

    let cur = n - di;
    let prev = cur - 1;
    if a[cur].is_nan() || b[cur].is_nan() || cl[cur].is_nan() || dl[cur].is_nan() {
        return None;
    }

    let cloud_top = a[cur].max(b[cur]);
    let cloud_bottom = a[cur].min(b[cur]);

    let v2 = if a[cur] > b[cur] { "上升云" } else { "下降云" };

    // v1: 8种价格位置
    let (h, l, c) = (high[cur], low[cur], close[cur]);
    let v1 = if l > cloud_top { "云上" }
    else if h < cloud_bottom { "云下" }
    else if c > cloud_top { "突破上" }
    else if c < cloud_bottom { "跌破下" }
    else if h > cloud_top && l < cloud_bottom { "贯穿云" }
    else if h > cloud_top { "测云顶" }
    else if l < cloud_bottom { "测云底" }
    else { "云内" };

    // v3: CL/DL关系
    let v3 = if prev > 0 && !cl[prev].is_nan() && !dl[prev].is_nan()
        && cl[prev] <= dl[prev] && cl[cur] > dl[cur] { "CL金叉" }
    else if prev > 0 && !cl[prev].is_nan() && !dl[prev].is_nan()
        && cl[prev] >= dl[prev] && cl[cur] < dl[cur] { "CL死叉" }
    else if l >= cl[cur] && l >= dl[cur] { "双强" }
    else if c >= cl[cur] && c >= dl[cur] { "双高" }
    else if h >= cl[cur] && h >= dl[cur] { "高影" }
    else if c >= cl[cur].min(dl[cur]) && c <= cl[cur].max(dl[cur]) { "线内" }
    else if c <= cl[cur] && c <= dl[cur] { "双低" }
    else if l <= cl[cur] && l <= dl[cur] { "低影" }
    else if h <= cl[cur] && h <= dl[cur] { "双弱" }
    else { "其他" };

    let score = ic_state_score(v1, v2, v3);
    Some(IcSignal { v1, v2, v3, score })
}

fn ic_state_score(v1: &str, v2: &str, v3: &str) -> i32 {
    match (v1, v2, v3) {
        ("云下", "上升云", "双强") => 78,
        ("跌破下", "上升云", "双强") | ("跌破下", "上升云", "双高") => 76,
        ("突破上", "上升云", "双强") | ("突破上", "上升云", "双高") => 76,
        ("云下", "上升云", "双高") | ("云下", "上升云", "其他") => 75,
        ("突破上", "下降云", "双强") | ("突破上", "下降云", "双高") => 70,
        ("云上", "上升云", "双强") | ("云上", "上升云", "双高") => 65,
        ("云下", "上升云", "高影") => 65,
        ("云下", "上升云", "低影") => 60,
        ("突破上", "上升云", "高影") => 60,
        ("云上", "上升云", "高影") => 55,
        ("云上", "上升云", "线内") => 53,
        ("云上", "上升云", "其他") => 52,
        ("云上", "下降云", "双强") | ("云上", "下降云", "双高") => 52,
        ("云上", "下降云", "其他") => 50,
        ("云下", "下降云", "双强") | ("云下", "下降云", "双高") => 50,
        ("云下", "下降云", "其他") => 60,
        ("云内", "上升云", "双强") | ("云内", "上升云", "双高") => 46,
        ("云上", "下降云", "高影") => 48,
        ("云上", "下降云", "线内") => 48,
        ("云内", "上升云", "其他") => 38,
        ("测云底", "上升云", "双强") | ("测云底", "上升云", "双高") => 38,
        ("贯穿云", "上升云", "双强") | ("贯穿云", "上升云", "双高") => 40,
        ("云下", "上升云", "双弱") => 42,
        ("云下", "下降云", "双低") => 35,
        ("云内", "下降云", "双弱") | ("云内", "下降云", "双低") => 10,
        _ => 5,
    }
}
