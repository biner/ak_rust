use chrono::NaiveTime;

/// 逐笔成交数据点。
#[derive(Debug, Clone)]
pub struct TransactionPoint {
    /// 时间（时分秒，如 09:31:00）
    pub time: NaiveTime,
    /// 成交价
    pub price: f64,
    /// 成交量（股）
    pub vol: i64,
    /// 成交笔数
    pub trade_count: i32,
    /// 买卖标识：0=买入，1=卖出，2=中性盘，5=盘后
    pub bs_flag: i32,
}

/// 逐笔成交分析结果。
#[derive(Debug, Clone)]
pub struct TransactionAnalysisResult {
    // ── 买卖力量 ──
    /// 主动买成交量占比
    pub buy_vol_ratio: f64,
    /// 主动卖成交量占比
    pub sell_vol_ratio: f64,
    /// 中性盘成交量占比
    pub neutral_vol_ratio: f64,

    // ── 大单分析（原始：仅按volume）──
    /// 大单总数量
    pub large_trade_count: i32,
    /// 大单成交量占比
    pub large_trade_vol_ratio: f64,
    /// 大买单数量
    pub large_buy_count: i32,
    /// 大卖单数量
    pub large_sell_count: i32,
    /// 大买单成交量占比（大单中主动买的占比）
    pub large_buy_vol_ratio: f64,

    // ── 主力分析（基于trade_count过滤，识别真机构单）──
    /// 主力攻击单买入量占比（大额+低笔数）
    pub inst_buy_vol_ratio: f64,
    /// 主力攻击买单数
    pub inst_buy_count: i32,
    /// 主力攻击卖单数
    pub inst_sell_count: i32,
    /// 主力净流向：>0 净流入，<0 净流出
    pub inst_net_flow_ratio: f64,
    /// 拆单嫌疑集群数
    pub split_clusters: i32,
    /// 对倒嫌疑配对数量
    pub matched_pairs: i32,

    // ── 价格位置 ──
    /// 收盘价在当日区间中的位置 [0, 1]
    pub price_position: f64,
    /// 当日涨跌幅
    pub day_chg_pct: f64,

    // ── 时段分析 ──
    /// 尾盘（14:00后）成交量占比
    pub last_30min_vol_ratio: f64,
    /// 尾盘 30 分钟买卖比（买量/卖量）
    pub last_30min_buy_sell_ratio: f64,
    /// 尾盘动量（主动买-主动卖）/ 总成交量
    pub last_30min_momentum: f64,

    // ── 综合判断 ──
    /// 是否是真突破
    pub is_breakout: bool,
    /// 置信度 [0, 100]
    pub confidence: f64,
    /// 分析结论描述
    pub conclusion: String,
    /// 分析明细
    pub details: String,
}

/// 突破时刻分析结果。
#[derive(Debug, Clone)]
pub struct BreakoutResult {
    /// 总成交笔数
    pub total_n: usize,
    /// 突破（成交价≥阈值）的笔数
    pub breakout_n: usize,
    /// 突破成交量占比
    pub breakout_vol_ratio: f64,
    /// 突破时买方占比
    pub breakout_buy_ratio: f64,
    /// 突破时的主动大买的笔数（仅买卖盘，filter bs_flag=0）
    pub breakout_large_buy: usize,
    /// 突破时的主动大卖笔数
    pub breakout_large_sell: usize,
    /// 价格维持度（突破后价格维持在阈值之上的比例）
    pub price_sustain_ratio: f64,
    /// 是否强势突破
    pub is_strong: bool,
    /// 分类标签（强势/普通/弱势）
    pub classification: String,
    /// 置信度 [0, 100]
    pub confidence: f64,
    /// 分析明细
    pub details: String,
}

/// 筛选用逐笔成交数据点（简化版，仅需 time/price/vol/trade_count）。
#[derive(Debug, Clone)]
pub struct ScreeningTxPoint {
    pub time: NaiveTime,
    pub price: f64,
    pub vol: i64,
    pub trade_count: i32,
}

/// 逐笔成交筛选指标（分时段成交占比、大单占比、加权均价等）。
#[derive(Debug, Clone)]
pub struct ScreeningMetrics {
    /// 早盘前30分钟（9:30-10:00）成交量占比（%）
    pub first30_vol_pct: f64,
    /// 午间（10:30-11:30）成交量占比（%）
    pub midday_vol_pct: f64,
    /// 下午盘（13:00-14:00）成交量占比（%）
    pub afternoon_vol_pct: f64,
    /// 尾盘30分钟（14:30-15:00）成交量占比（%）
    pub last30_vol_pct: f64,
    /// 尾盘5分钟（14:55-15:00）成交量占比（%）
    pub last5_vol_pct: f64,
    /// 大单成交笔数占比（单笔≥均量×3，%）
    pub large_pct: f64,
    /// 特大单成交笔数占比（单笔≥均量×10，%）
    pub xl_pct: f64,
    /// 大单成交金额占比（单笔金额≥均额×3，%）
    pub large_amount_pct: f64,
    /// 特大单成交金额占比（单笔金额≥均额×10，%）
    pub xl_amount_pct: f64,
    /// 平均每笔成交量（股）
    pub avg_trade_vol: f64,
    /// 成交量加权均价（VWAP）
    pub vwap: f64,
    /// 单笔均量（总股数/总成交笔数）
    pub avg_shares_per_trade: f64,
    /// 单笔最大成交量占比（%）
    pub single_trade_vol_pct: f64,
}
