//! 信号调度框架

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};
use crate::wavetrend::{compute_wavetrend, get_wt_signal};
use crate::{dmi, ic, ttm};

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
}

type SignalFn = Box<dyn Fn(&[BarPoint], &HashMap<String, String>) -> Vec<SignalOutput> + Send>;

fn get_registry() -> std::sync::MutexGuard<'static, HashMap<String, SignalFn>> {
    static REGISTRY: OnceLock<Mutex<HashMap<String, SignalFn>>> = OnceLock::new();
    REGISTRY.get_or_init(|| Mutex::new(HashMap::new())).lock().unwrap()
}

pub fn register_builtins() {
    let mut r = get_registry();
    r.insert("ak_wavetrend_V260625".into(), Box::new(signal_ak_wavetrend_v260625));
    r.insert("ak_dmi_state_V250714".into(), Box::new(signal_ak_dmi));
    r.insert("ak_ic_state_V250718".into(), Box::new(signal_ak_ic));
    r.insert("ak_ttm_squeeze_V250711".into(), Box::new(signal_ak_ttm));
    r.insert("ak_wavetrend_zone_V250724".into(), Box::new(signal_ak_wt_zone));
}

pub fn list_signal_names() -> Vec<String> {
    get_registry().keys().cloned().collect()
}

pub fn dispatch_one(name: &str, bars: &[BarPoint], params: &HashMap<String, String>) -> Vec<SignalOutput> {
    let r = get_registry();
    match r.get(name) {
        Some(f) => f(bars, params),
        None => vec![SignalOutput { name: name.to_string(), freq: String::new(), v1: "未注册".into(), v2: "未注册".into(), v3: "未注册".into() }],
    }
}

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

fn signal_ak_wavetrend_v260625(bars: &[BarPoint], params: &HashMap<String, String>) -> Vec<SignalOutput> {
    let n1 = params.get("n1").and_then(|s| s.parse::<usize>().ok()).unwrap_or(10);
    let n2 = params.get("n2").and_then(|s| s.parse::<usize>().ok()).unwrap_or(21);
    let n3 = params.get("n3").and_then(|s| s.parse::<usize>().ok()).unwrap_or(4);
    let di = params.get("di").and_then(|s| s.parse::<usize>().ok()).unwrap_or(1);
    let freq = bars.first().map(|b| b.freq.clone()).unwrap_or_default();
    let high: Vec<f64> = bars.iter().map(|b| b.high).collect();
    let low: Vec<f64> = bars.iter().map(|b| b.low).collect();
    let close: Vec<f64> = bars.iter().map(|b| b.close).collect();
    if close.len() < n1 + n2 + n3 + 5 {
        return vec![SignalOutput { name: "ak_wavetrend_V260625".into(), freq, v1: "其他".into(), v2: "任意".into(), v3: "其他".into() }];
    }
    let wt = compute_wavetrend(&high, &low, &close, n1, n2, n3);
    match get_wt_signal(&wt, di) {
        Some(s) => vec![SignalOutput { name: "ak_wavetrend_V260625".into(), freq, v1: s.zone.into(), v2: s.direction.into(), v3: s.cross.into() }],
        None => vec![SignalOutput { name: "ak_wavetrend_V260625".into(), freq, v1: "其他".into(), v2: "任意".into(), v3: "其他".into() }],
    }
}

fn signal_ak_dmi(bars: &[BarPoint], params: &HashMap<String, String>) -> Vec<SignalOutput> {
    let n1 = params.get("n1").and_then(|s| s.parse::<usize>().ok()).unwrap_or(14);
    let n2 = params.get("n2").and_then(|s| s.parse::<usize>().ok()).unwrap_or(6);
    let di = params.get("di").and_then(|s| s.parse::<usize>().ok()).unwrap_or(1);
    let freq = bars.first().map(|b| b.freq.clone()).unwrap_or_default();
    let high: Vec<f64> = bars.iter().map(|b| b.high).collect();
    let low: Vec<f64> = bars.iter().map(|b| b.low).collect();
    let close: Vec<f64> = bars.iter().map(|b| b.close).collect();
    match dmi::compute_dmi(&high, &low, &close, n1, n2, di) {
        Some(s) => vec![SignalOutput { name: "ak_dmi_state_V250714".into(), freq, v1: s.v1.into(), v2: s.v2.into(), v3: s.v3.into() }],
        None => vec![SignalOutput { name: "ak_dmi_state_V250714".into(), freq, v1: "其他".into(), v2: "任意".into(), v3: "任意".into() }],
    }
}

fn signal_ak_ic(bars: &[BarPoint], params: &HashMap<String, String>) -> Vec<SignalOutput> {
    let n1 = params.get("n1").and_then(|s| s.parse::<usize>().ok()).unwrap_or(9);
    let n2 = params.get("n2").and_then(|s| s.parse::<usize>().ok()).unwrap_or(26);
    let n3 = params.get("n3").and_then(|s| s.parse::<usize>().ok()).unwrap_or(52);
    let di = params.get("di").and_then(|s| s.parse::<usize>().ok()).unwrap_or(1);
    let freq = bars.first().map(|b| b.freq.clone()).unwrap_or_default();
    let high: Vec<f64> = bars.iter().map(|b| b.high).collect();
    let low: Vec<f64> = bars.iter().map(|b| b.low).collect();
    let close: Vec<f64> = bars.iter().map(|b| b.close).collect();
    match ic::compute_ic_state(&high, &low, &close, n1, n2, n3, di) {
        Some(s) => vec![SignalOutput { name: "ak_ic_state_V250718".into(), freq, v1: s.v1.into(), v2: s.v2.into(), v3: s.v3.into() }],
        None => vec![SignalOutput { name: "ak_ic_state_V250718".into(), freq, v1: "其他".into(), v2: "任意".into(), v3: "任意".into() }],
    }
}

fn signal_ak_ttm(bars: &[BarPoint], params: &HashMap<String, String>) -> Vec<SignalOutput> {
    let n1 = params.get("n1").and_then(|s| s.parse::<usize>().ok()).unwrap_or(20);
    let n2 = params.get("n2").and_then(|s| s.parse::<f64>().ok()).unwrap_or(1.5);
    let n3 = params.get("n3").and_then(|s| s.parse::<usize>().ok()).unwrap_or(20);
    let n4 = params.get("n4").and_then(|s| s.parse::<f64>().ok()).unwrap_or(2.0);
    let n5 = params.get("n5").and_then(|s| s.parse::<usize>().ok()).unwrap_or(n1);
    let di = params.get("di").and_then(|s| s.parse::<usize>().ok()).unwrap_or(1);
    let freq = bars.first().map(|b| b.freq.clone()).unwrap_or_default();
    let high: Vec<f64> = bars.iter().map(|b| b.high).collect();
    let low: Vec<f64> = bars.iter().map(|b| b.low).collect();
    let close: Vec<f64> = bars.iter().map(|b| b.close).collect();
    match ttm::compute_ttm(&high, &low, &close, n1, n2, n3, n4, n5) {
        Some((ttm, sqz_on, sqz_off)) => match ttm::get_ttm_signal(&ttm, &sqz_on, &sqz_off, di) {
            Some(s) => vec![SignalOutput { name: "ak_ttm_squeeze_V250711".into(), freq, v1: s.v1.into(), v2: s.v2.into(), v3: s.v3.into() }],
            None => vec![SignalOutput { name: "ak_ttm_squeeze_V250711".into(), freq, v1: "其他".into(), v2: "任意".into(), v3: "任意".into() }],
        },
        None => vec![SignalOutput { name: "ak_ttm_squeeze_V250711".into(), freq, v1: "其他".into(), v2: "任意".into(), v3: "任意".into() }],
    }
}

fn signal_ak_wt_zone(bars: &[BarPoint], params: &HashMap<String, String>) -> Vec<SignalOutput> {
    use crate::wavetrend::{compute_wavetrend, compute_wt_zone_signal};
    let n1 = params.get("n1").and_then(|s| s.parse::<usize>().ok()).unwrap_or(10);
    let n2 = params.get("n2").and_then(|s| s.parse::<usize>().ok()).unwrap_or(21);
    let n3 = params.get("n3").and_then(|s| s.parse::<usize>().ok()).unwrap_or(4);
    let di = params.get("di").and_then(|s| s.parse::<usize>().ok()).unwrap_or(1);
    let freq = bars.first().map(|b| b.freq.clone()).unwrap_or_default();
    let high: Vec<f64> = bars.iter().map(|b| b.high).collect();
    let low: Vec<f64> = bars.iter().map(|b| b.low).collect();
    let close: Vec<f64> = bars.iter().map(|b| b.close).collect();
    if close.len() < n1 + n2 + n3 + 5 {
        return vec![SignalOutput { name: "ak_wavetrend_zone_V250724".into(), freq, v1: "其他".into(), v2: "任意".into(), v3: "其他".into() }];
    }
    let wt = compute_wavetrend(&high, &low, &close, n1, n2, n3);
    let len = wt.len();
    if len < di + 1 {
        return vec![SignalOutput { name: "ak_wavetrend_zone_V250724".into(), freq, v1: "其他".into(), v2: "任意".into(), v3: "其他".into() }];
    }
    // 遍历所有 bar，维护状态机
    let mut prev_state = 0i32;
    let mut last_output = SignalOutput { name: "ak_wavetrend_zone_V250724".into(), freq: freq.clone(), v1: "其他".into(), v2: "任意".into(), v3: "其他".into() };
    for i in 1..len {
        let (w1_c, w2_c, w1_p) = (wt[i].wt1, wt[i].wt2, wt[i-1].wt1);
        if w1_c.is_nan() || w2_c.is_nan() || w1_p.is_nan() { continue; }
        let (v1, v2, v3, new_state, _score) = compute_wt_zone_signal(w1_c, w2_c, w1_p, prev_state);
        prev_state = new_state;
        last_output = SignalOutput { name: "ak_wavetrend_zone_V250724".into(), freq: freq.clone(), v1: v1.into(), v2: v2.into(), v3: v3.into() };
    }
    vec![last_output]
}
#[cfg(test)]

/// ak_wavetrend_zone_V250724
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
        register_builtins();
        let bars = make_bars(100);
        let sigs = dispatch_one("ak_wavetrend_V260625", &bars, &HashMap::from([("n1".into(),"10".into()),("n2".into(),"21".into()),("n3".into(),"4".into())]));
        assert_eq!(sigs.len(), 1);
        assert!(["多头","多头强势","弱多","弱空","空头"].contains(&sigs[0].v1.as_str()));
    }

    #[test]
    fn test_dispatch_dmi() {
        register_builtins();
        let bars = make_bars(100);
        let sigs = dispatch_one("ak_dmi_state_V250714", &bars, &HashMap::from([("n1".into(),"14".into()),("n2".into(),"6".into())]));
        assert_eq!(sigs.len(), 1);
        println!("DMI v1={v1} v2={v2} v3={v3}", v1=sigs[0].v1, v2=sigs[0].v2, v3=sigs[0].v3);
        assert!(["盘整","酝酿","趋势","强趋势","其他"].contains(&sigs[0].v1.as_str()));
    }

    #[test]
    fn test_all_registered() {
        register_builtins();
        let names = list_signal_names();
        assert!(names.contains(&"ak_wavetrend_V260625".into()));
        assert!(names.contains(&"ak_dmi_state_V250714".into()));
        assert!(names.contains(&"ak_ic_state_V250718".into()));
        assert!(names.contains(&"ak_ttm_squeeze_V250711".into()));
    }
}

