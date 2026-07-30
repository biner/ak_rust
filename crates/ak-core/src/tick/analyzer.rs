use crate::tick::core::analyze_tick_chart;
use crate::tick::types::{TickChartResult, TickPoint};

/// 分时图分析器 trait。将分析算法抽象为接口，便于测试 mock 和未来版本切换。
pub trait TickAnalyzer {
    /// 分析分时 tick 数据，返回量价分析结果。
    fn analyze(
        &self,
        ticks: &[TickPoint],
        pre_close: f64,
        low_price: Option<f64>,
        close_pos_threshold: f64,
    ) -> Option<TickChartResult>;
}

/// 默认分时图分析器，使用当前 `analyze_tick_chart` 算法。
#[derive(Debug, Clone, Default)]
pub struct DefaultTickAnalyzer;

impl TickAnalyzer for DefaultTickAnalyzer {
    fn analyze(
        &self,
        ticks: &[TickPoint],
        pre_close: f64,
        low_price: Option<f64>,
        close_pos_threshold: f64,
    ) -> Option<TickChartResult> {
        analyze_tick_chart(ticks, pre_close, low_price, close_pos_threshold)
    }
}
