use crate::transaction::core::analyze_transactions;
use crate::transaction::types::{TransactionAnalysisResult, TransactionPoint};

/// 逐笔成交分析器 trait。将分析算法抽象为接口，便于切换版本和测试 mock。
pub trait TxAnalyzer {
    /// 分析逐笔成交数据，返回多维分析结果。
    fn analyze(
        &self,
        transactions: &[TransactionPoint],
        pre_close: f64,
        high: f64,
        low: f64,
    ) -> Option<TransactionAnalysisResult>;
}

/// 默认逐笔成交分析器，使用当前 `analyze_transactions` (V4) 算法。
#[derive(Debug, Clone, Default)]
pub struct DefaultTxAnalyzer;

impl TxAnalyzer for DefaultTxAnalyzer {
    fn analyze(
        &self,
        transactions: &[TransactionPoint],
        pre_close: f64,
        high: f64,
        low: f64,
    ) -> Option<TransactionAnalysisResult> {
        analyze_transactions(transactions, pre_close, high, low)
    }
}
