"""ak_rust 分析结果类型 —— 类型安全的 dataclass 封装。

所有 dataclass 的字段名与 Rust 侧返回的 dict key 一一对应，
可直接通过 `**raw_dict` 构造。
"""

from __future__ import annotations

from dataclasses import dataclass, field
from typing import Optional


@dataclass
class TickChartResult:
    """分时图量价分析结果。"""
    close_pos: float
    day_chg_pct: float
    amplitude_pct: float
    total_vol: int
    n: int
    am_avg_price: float = 0.0
    pm_avg_price: float = 0.0
    am_pm_diff_pct: float = 0.0
    last_30min_chg_pct: float = 0.0
    large_trade_count: int = 0
    large_trade_vol_ratio: float = 0.0
    large_am: int = 0
    large_pm: int = 0
    large_last_30: int = 0
    large_last_30_vol: int = 0
    avg_vol_per_min: float = 0.0
    vol_std: float = 0.0
    quality_score: float = 0.0
    should_cancel: bool = False
    cancel_reason: str = ""


@dataclass
class TransactionAnalysisResult:
    """逐笔成交分析结果（V4 增强版）。"""
    buy_vol_ratio: float = 0.0
    sell_vol_ratio: float = 0.0
    neutral_vol_ratio: float = 0.0
    large_trade_count: int = 0
    large_trade_vol_ratio: float = 0.0
    large_buy_count: int = 0
    large_sell_count: int = 0
    large_buy_vol_ratio: float = 0.0
    inst_buy_vol_ratio: float = 0.0
    inst_buy_count: int = 0
    inst_sell_count: int = 0
    inst_net_flow_ratio: float = 0.0
    split_clusters: int = 0
    matched_pairs: int = 0
    price_position: float = 0.0
    day_chg_pct: float = 0.0
    last_30min_vol_ratio: float = 0.0
    last_30min_buy_sell_ratio: float = 0.0
    last_30min_momentum: float = 0.0
    is_breakout: bool = False
    confidence: float = 0.0
    conclusion: str = ""
    details: str = ""


@dataclass
class BreakoutResult:
    """突破时刻分析结果。"""
    total_n: int = 0
    breakout_n: int = 0
    breakout_vol_ratio: float = 0.0
    breakout_buy_ratio: float = 0.0
    breakout_large_buy: int = 0
    breakout_large_sell: int = 0
    price_sustain_ratio: float = 0.0
    is_strong: bool = False
    classification: str = ""
    confidence: float = 0.0
    details: str = ""


@dataclass
class ScanResult:
    """集合竞价分析结果。"""
    score: int = 0
    match: bool = False
    steps: int = 0
    eff_steps: int = 0
    neg_pos_alt: bool = False
    buy_rev: bool = False
    vr: float = 0.0
    end_p: bool = False
    oh: Optional[bool] = None
    n: int = 0
    max_m: int = 0
    first_p: float = 0.0
    last_p: float = 0.0
    last_um: int = 0
    morphology: str = ""
    morphology_conf: int = 0
    morphology_scores: list[int] = field(default_factory=lambda: [0] * 7)
    morphology_code: int = 0
    morphology_group: str = ""
    morphology_tag: str = ""


@dataclass
class FalseSellResult:
    """虚假卖出检测结果（T2 模式）。"""
    score: int = 0
    match: bool = False
    neg_ratio: float = 0.0
    price_up: bool = False
    vr: float = 0.0
    last_um: int = 0
    neg_total: int = 0
    late_peak: bool = False
    recovery_score: float = 0.0
    n: int = 0
    max_match: int = 0
    price_change_pct: float = 0.0
    net_um: int = 0
    total_amt: float = 0.0
    big_order_ratio: float = 0.0


@dataclass
class ScreeningMetrics:
    """逐笔成交筛选指标。"""
    first30_vol_pct: float = 0.0
    midday_vol_pct: float = 0.0
    afternoon_vol_pct: float = 0.0
    last30_vol_pct: float = 0.0
    last5_vol_pct: float = 0.0
    large_pct: float = 0.0
    xl_pct: float = 0.0
    large_amount_pct: float = 0.0
    xl_amount_pct: float = 0.0
    avg_trade_vol: float = 0.0
    vwap: float = 0.0
    avg_shares_per_trade: float = 0.0
    single_trade_vol_pct: float = 0.0
