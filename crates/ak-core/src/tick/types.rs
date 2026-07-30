use chrono::NaiveTime;

/// 分钟级 Tick 数据点。
#[derive(Debug, Clone)]
pub struct TickPoint {
    /// 时间（时分秒，如 09:31:00）
    pub time: NaiveTime,
    /// 当前价格
    pub price: f64,
    /// 均价
    pub avg: f64,
    /// 成交量（股）
    pub vol: i64,
    /// 动量（主动买-主动卖）
    pub momentum: f64,
}

/// Tick 图表分析结果。
#[derive(Debug, Clone)]
pub struct TickChartResult {
    // ── 基础统计 ──
    /// 收盘位置：(close - low) / (high - low)，[0, 1]
    pub close_pos: f64,
    /// 当日涨跌幅（基于前收盘）
    pub day_chg_pct: f64,
    /// 振幅：(high - low) / pre_close * 100
    pub amplitude_pct: f64,
    /// 总成交量
    pub total_vol: i64,
    /// 数据点数量
    pub n: usize,

    // ── 时段分析 ──
    /// 早盘均价（9:30-11:30）
    pub am_avg_price: f64,
    /// 午盘均价（13:00-15:00）
    pub pm_avg_price: f64,
    /// 早午均价差百分比
    pub am_pm_diff_pct: f64,
    /// 尾盘 30 分钟涨跌幅
    pub last_30min_chg_pct: f64,

    // ── 大单分析 ──
    /// 大单次数（成交量 > 均值 × 2）
    pub large_trade_count: usize,
    /// 大单成交量占比
    pub large_trade_vol_ratio: f64,
    /// 早盘大单次数
    pub large_am: usize,
    /// 午盘大单次数
    pub large_pm: usize,
    /// 尾盘 30 分钟大单次数
    pub large_last_30: usize,
    /// 尾盘 30 分钟大单成交量
    pub large_last_30_vol: i64,

    // ── 分钟级统计 ──
    /// 每分钟平均成交量
    pub avg_vol_per_min: f64,
    /// 每分钟成交量标准差
    pub vol_std: f64,

    // ── 综合判断 ──
    /// 综合信号质量评分 [0, 100]
    pub quality_score: f64,
    /// 是否应撤单（True = 信号质量差）
    pub should_cancel: bool,
    /// 撤单原因描述
    pub cancel_reason: String,
}
