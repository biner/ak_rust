//! ak-signals — 技术指标信号库
//!
//! 信号通过 `#[signal]` 宏标注，由 `inventory` 在编译期自动注册。

extern crate self as ak_signals;

pub mod wavetrend;
pub mod dmi;
pub mod ic;
pub mod ttm;
pub mod signals;
pub mod types;

pub use wavetrend::{compute_wavetrend, get_wt_signal, classify_wt_zone, classify_zone_v1, compute_long_term_state, compute_wt_zone_signal, WtPoint, WtSignal};
pub use dmi::DmiSignal;
pub use ic::IcSignal;
pub use ttm::TtmSignal;
pub use signals::{register_builtins, dispatch_signals, dispatch_one, list_signal_names, has_signal, BarPoint, SignalConfig, SignalOutput};

inventory::collect!(ak_signals::types::SignalDescriptor);
