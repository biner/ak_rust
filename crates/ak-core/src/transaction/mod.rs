//! 逐笔成交数据分析器 —— 判断真突破 vs 假突破。
//!
//! 模块结构：
//! - `types` — 数据结构（TransactionPoint, TransactionAnalysisResult, BreakoutResult, ScreeningTxPoint, ScreeningMetrics）
//! - `analyzer` — 成交分析器 trait 与默认实现（TxAnalyzer, DefaultTxAnalyzer）
//! - `breakout` — 突破分析器 trait 与默认实现（BreakoutAnalyzer, DefaultBreakoutAnalyzer）
//! - `core` — 核心分析算法（analyze_transactions, analyze_breakout, calc_buy_sell_ratio, V4 评分）
//! - `screening` — 筛选指标统计（compute_screening_metrics）

pub mod types;
pub mod analyzer;
pub mod breakout;
pub mod core;
pub mod screening;

pub use types::{
    TransactionPoint, TransactionAnalysisResult,
    BreakoutResult, ScreeningTxPoint, ScreeningMetrics,
};
pub use analyzer::{TxAnalyzer, DefaultTxAnalyzer};
pub use breakout::{BreakoutAnalyzer, DefaultBreakoutAnalyzer};
pub use core::{analyze_transactions, analyze_breakout, calc_buy_sell_ratio};
pub use screening::compute_screening_metrics;
