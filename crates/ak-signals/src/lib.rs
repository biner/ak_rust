pub mod wavetrend;
pub mod dmi;
pub mod ic;
pub mod ttm;
pub mod signals;

pub use wavetrend::{compute_wavetrend, get_wt_signal, classify_wt_zone, WtPoint, WtSignal};
pub use dmi::DmiSignal;
pub use ic::IcSignal;
pub use ttm::TtmSignal;
pub use signals::{register_builtins, dispatch_signals, dispatch_one, list_signal_names, BarPoint, SignalConfig, SignalOutput};
