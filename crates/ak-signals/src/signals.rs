//! 信号调度框架
//!
//! 信号函数通过 `#[signal]` 宏标注，由 `inventory` 编译期自动收集。
//! 运行时通过 `SIGNAL_REGISTRY` 查询并分派。

use std::collections::HashMap;
use std::sync::LazyLock;
use crate::types::AkSignalFn;
use crate::{dmi, ic};
use ak_signal_macros::signal;

#[derive(Debug, Clone)]
pub struct BarPoint {
    pub dt: String,
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub close: f64,
    pub vol: f64,
    pub amount: f64,
    pub freq: String,
}

#[derive(Debug, Clone)]
pub struct SignalConfig {
    pub name: String,
    pub freq: String,
    pub params: HashMap<String, String>,
}

#[derive(Debug, Clone)]
pub struct SignalOutput {
    pub name: String,
    pub freq: String,
    pub v1: String,
    pub v2: String,
    pub v3: String,
    pub score: i32,
}

impl SignalOutput {
    /// 构造默认输出（score=0）
    pub fn new(name: &str, freq: &str, v1: &str, v2: &str, v3: &str) -> Self {
        SignalOutput { name: name.into(), freq: freq.into(), v1: v1.into(), v2: v2.into(), v3: v3.into(), score: 0 }
    }

    /// 带 score
    pub fn with_score(name: &str, freq: &str, v1: &str, v2: &str, v3: &str, score: i32) -> Self {
        SignalOutput { name: name.into(), freq: freq.into(), v1: v1.into(), v2: v2.into(), v3: v3.into(), score }
    }
}

/// 运行时注册表：由 `inventory` 收集的 `SignalDescriptor` 构建
static SIGNAL_REGISTRY: LazyLock<HashMap<String, AkSignalFn>> = LazyLock::new(|| {
    let mut m = HashMap::new();
    for d in inventory::iter::<crate::types::SignalDescriptor> {
        m.insert(d.name.to_string(), d.func);
    }
    m
});

/// 注册内置信号（向后兼容）
///
/// 当前为 no-op：信号已通过 `#[signal]` + `inventory` 在编译期自动注册。
pub fn register_builtins() {
    let _ = &*SIGNAL_REGISTRY;
}

/// 查询信号是否已在 ak_rust 中注册
pub fn has_signal(name: &str) -> bool {
    SIGNAL_REGISTRY.contains_key(name)
}

pub fn list_signal_names() -> Vec<String> {
    SIGNAL_REGISTRY.keys().cloned().collect()
}

/// 分派单个信号，返回每根 bar 的信号值（长度 = bars.len()）
pub fn dispatch_one(name: &str, bars: &[BarPoint], params: &HashMap<String, String>) -> Vec<SignalOutput> {
    match SIGNAL_REGISTRY.get(name) {
        Some(f) => f(bars, params),
        None => bars.iter().map(|_| SignalOutput {
            name: name.to_string(), freq: String::new(),
            v1: "未注册".into(), v2: "未注册".into(), v3: "未注册".into(), score: 0,
        }).collect(),
    }
}

/// 分派多个信号，返回扁平列表（每根 bar × 每个信号）
pub fn dispatch_signals(configs: &[SignalConfig], bars: &[BarPoint]) -> Vec<SignalOutput> {
    let mut outputs = Vec::new();
    for config in configs {
        let sigs = dispatch_one(&config.name, bars, &config.params);
        for mut sig in sigs {
            sig.freq = config.freq.clone();
            outputs.push(sig);
        }
    }
    outputs
}

// ── 信号函数 ───────────────────────────────────────────────

#[signal(
    category = "kline",
    name = "ak_wavetrend_V260625",
    template = "{freq}_D{di}WT#{n1}#{n2}#{n3}_V260625",
    opcode = "AkWavetrend",
    param_kind = "AkWavetrend"
)]
pub fn ak_wavetrend_v260625(bars: &[BarPoint], params: &HashMap<String, String>) -> Vec<SignalOutput> {
    crate::wavetrend::compute_wt_signal(bars, params)
}

#[signal(
    category = "kline",
    name = "ak_dmi_state_V260714",
    template = "{freq}_D{di}DMI#{n1}#{n2}_V260714",
    opcode = "AkDmiState",
    param_kind = "AkDmiState"
)]
pub fn ak_dmi_state_v260714(bars: &[BarPoint], params: &HashMap<String, String>) -> Vec<SignalOutput> {
    let n1 = params.get("n1").and_then(|s| s.parse::<usize>().ok()).unwrap_or(14);
    let n2 = params.get("n2").and_then(|s| s.parse::<usize>().ok()).unwrap_or(6);
    let freq = bars.first().map(|b| b.freq.clone()).unwrap_or_default();
    let name = "ak_dmi_state_V260714";
    let high: Vec<f64> = bars.iter().map(|b| b.high).collect();
    let low: Vec<f64> = bars.iter().map(|b| b.low).collect();
    let close: Vec<f64> = bars.iter().map(|b| b.close).collect();
    let series = dmi::compute_dmi_state(&high, &low, &close, n1, n2);
    series.iter().map(|o| match o {
        Some((v1, v2, v3)) => SignalOutput::new(name, &freq, v1, v2, v3),
        None => SignalOutput::with_score(name, &freq, "其他", "任意", "任意", 0),
    }).collect()
}

#[signal(
    category = "kline",
    name = "ak_ic_state_V260718",
    template = "{freq}_D{di}IC#{n1}#{n2}#{n3}_V260718",
    opcode = "AkIcState",
    param_kind = "AkIcState"
)]
pub fn ak_ic_state_v260718(bars: &[BarPoint], params: &HashMap<String, String>) -> Vec<SignalOutput> {
    let n1 = params.get("n1").and_then(|s| s.parse::<usize>().ok()).unwrap_or(9);
    let n2 = params.get("n2").and_then(|s| s.parse::<usize>().ok()).unwrap_or(26);
    let n3 = params.get("n3").and_then(|s| s.parse::<usize>().ok()).unwrap_or(52);
    let di = params.get("di").and_then(|s| s.parse::<usize>().ok()).unwrap_or(1);
    let freq = bars.first().map(|b| b.freq.clone()).unwrap_or_default();
    let high: Vec<f64> = bars.iter().map(|b| b.high).collect();
    let low: Vec<f64> = bars.iter().map(|b| b.low).collect();
    let close: Vec<f64> = bars.iter().map(|b| b.close).collect();
    match ic::compute_ic_state(&high, &low, &close, n1, n2, n3, di) {
        Some(s) => vec![SignalOutput::with_score("ak_ic_state_V260718", &freq, s.v1, s.v2, s.v3, s.score)],
        None => vec![SignalOutput::new("ak_ic_state_V260718", &freq, "其他", "任意", "任意")],
    }
}

#[signal(
    category = "kline",
    name = "ak_ttm_squeeze_V260711",
    template = "{freq}_D{di}TTM#{n1}#{n2}#{n3}#{n4}#{n5}#{n6}_V260711",
    opcode = "AkTtmSqueeze",
    param_kind = "AkTtmSqueeze"
)]
pub fn ak_ttm_squeeze_v260711(bars: &[BarPoint], params: &HashMap<String, String>) -> Vec<SignalOutput> {
    crate::ttm::compute_ttm_signal(bars, params)
}

#[signal(
    category = "kline",
    name = "ak_osc_state_V260720",
    template = "{freq}_D{di}OSC#{n}#{m}_V260720",
    opcode = "AkOscStateV260720",
    param_kind = "AkOscState"
)]
pub fn ak_osc_state_v260720(bars: &[BarPoint], params: &HashMap<String, String>) -> Vec<SignalOutput> {
    crate::osc::compute_osc_720(bars, params)
}

#[signal(
    category = "kline",
    name = "ak_osc_state_V260721",
    template = "{freq}_D{di}OSC#{n}#{m}_V260721",
    opcode = "AkOscStateV260721",
    param_kind = "AkOscStateV260721"
)]
pub fn ak_osc_state_v260721(bars: &[BarPoint], params: &HashMap<String, String>) -> Vec<SignalOutput> {
    crate::osc::compute_osc_721(bars, params)
}

#[signal(
    category = "kline",
    name = "ak_vol_ratio_V260721",
    template = "{freq}_D{di}VOL#{n}_V260721",
    opcode = "AkVolRatio",
    param_kind = "AkVolRatio"
)]
pub fn ak_vol_ratio_v260721(bars: &[BarPoint], params: &HashMap<String, String>) -> Vec<SignalOutput> {
    crate::vol::compute_vol_ratio(bars, params)
}

#[signal(
    category = "kline",
    name = "ak_wavetrend_zone_V260724",
    template = "{freq}_D{di}WT#{n1}#{n2}#{n3}_V260724",
    opcode = "AkWavetrendZone",
    param_kind = "AkWavetrendZone"
)]
pub fn ak_wavetrend_zone_v260724(bars: &[BarPoint], params: &HashMap<String, String>) -> Vec<SignalOutput> {
    crate::wavetrend::compute_wt_zone_signal_full(bars, params)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_bars(n: usize) -> Vec<BarPoint> {
        (0..n).map(|i| BarPoint {
            dt: format!("2025-01-01"), open: 100.0 + i as f64,
            high: 101.0 + i as f64, low: 99.0 + i as f64,
            close: 100.0 + i as f64 * 0.5,
            vol: 1000.0, amount: 100000.0, freq: "日线".into(),
        }).collect()
    }

    #[test]
    fn test_dispatch_wt() {
        let bars = make_bars(100);
        let sigs = dispatch_one("ak_wavetrend_V260625", &bars, &HashMap::from([("n1".into(),"10".into()),("n2".into(),"21".into()),("n3".into(),"4".into())]));
        assert_eq!(sigs.len(), bars.len(), "should return 1 per bar");
        let last = sigs.last().unwrap();
        let v1 = last.v1.as_str();
        assert!(["极度超买","超买","偏多","弱多","弱空","偏空","超卖","极度超卖"].contains(&v1));
    }

    #[test]
    fn test_dispatch_dmi() {
        let bars = make_bars(100);
        let sigs = dispatch_one("ak_dmi_state_V260714", &bars, &HashMap::from([("n1".into(),"14".into()),("n2".into(),"6".into())]));
        assert_eq!(sigs.len(), bars.len());
        let last = sigs.last().unwrap();
        assert!(["盘整","酝酿","趋势","强趋势","其他"].contains(&last.v1.as_str()));
    }

    #[test]
    fn test_all_registered() {
        let names = list_signal_names();
        assert!(names.contains(&"ak_wavetrend_V260625".into()));
        assert!(names.contains(&"ak_dmi_state_V260714".into()));
        assert!(names.contains(&"ak_ic_state_V260718".into()));
        assert!(names.contains(&"ak_ttm_squeeze_V260711".into()));
    }

    #[test]
    fn test_has_signal() {
        assert!(has_signal("ak_wavetrend_V260625"));
        assert!(has_signal("ak_ttm_squeeze_V260711"));
        assert!(!has_signal("nonexistent_V999999"));
    }

    #[test]
    fn test_unregistered_falls_through() {
        let bars = make_bars(50);
        let sigs = dispatch_one("ak_ttm_squeeze_V250711", &bars, &HashMap::new());
        assert_eq!(sigs.len(), bars.len());
        assert!(sigs.last().is_some());
        sigs.last().map(|s| assert_eq!(s.v1, "未注册"));
    }
}
