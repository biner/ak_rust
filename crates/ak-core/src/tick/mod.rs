//! 盘中 Tick 图表分析器 —— 分钟级 tick 数据的量价特征分析。
//!
//! 用于判断金叉信号日 tick 数据的质量，辅助过滤虚假信号。
//!
//! 模块结构：
//! - `types` — 数据结构（TickPoint, TickChartResult）
//! - `analyzer` — 分析器 trait 与默认实现（TickAnalyzer, DefaultTickAnalyzer）
//! - `core` — 核心分析算法（analyze_tick_chart, compute_quality, calc_close_pos）

pub mod types;
pub mod analyzer;
pub mod core;

pub use types::{TickPoint, TickChartResult};
pub use analyzer::{TickAnalyzer, DefaultTickAnalyzer};
pub use core::{analyze_tick_chart, calc_close_pos};
