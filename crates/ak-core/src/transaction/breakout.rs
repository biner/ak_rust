use crate::transaction::core::analyze_breakout;
use crate::transaction::types::{BreakoutResult, TransactionPoint};

/// 突破分析器 trait。
pub trait BreakoutAnalyzer {
    /// 分析突破时刻的逐笔成交数据，判断是真突破还是假突破。
    fn analyze(&self, transactions: &[TransactionPoint], breakout_price: f64) -> Option<BreakoutResult>;
}

/// 默认突破分析器。
#[derive(Debug, Clone, Default)]
pub struct DefaultBreakoutAnalyzer;

impl BreakoutAnalyzer for DefaultBreakoutAnalyzer {
    fn analyze(&self, transactions: &[TransactionPoint], breakout_price: f64) -> Option<BreakoutResult> {
        analyze_breakout(transactions, breakout_price)
    }
}
