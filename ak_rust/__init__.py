"""ak_rust — A 股量化分析工具箱（Rust 实现）

用法：
    from ak_rust import tick_analysis, trans_analysis, generate_all_signals
"""

from typing import Optional

from .signaling import generate_all_signals
from .models import (
    TickChartResult,
    TransactionAnalysisResult,
    BreakoutResult,
    ScanResult,
    FalseSellResult,
    ScreeningMetrics,
)

from ._native import (
    tick_analysis as _tick_analysis,
    trans_analysis as _trans_analysis,
    trans_breakout as _trans_breakout,
    screening_metrics as _screening_metrics,
    auction_scan as _auction_scan,
    auction_false_sell as _auction_false_sell,
    auction_scanner_version,
    auction_morphology_names,
    false_sell_version,
    compute_wavetrend,
    wt_classify_zone,
    wt_signal,
    wt_signal_batch,
    generate_ak_signals,
    ak_signal_list,
)


def tick_analysis(
    data: list[dict],
    pre_close: float,
    low_price: Optional[float] = None,
    close_pos_threshold: float = 0.3,
) -> Optional[TickChartResult]:
    """分时图量价质量分析。

    返回类型安全的 `TickChartResult` dataclass，数据不足时返回 None。
    """
    raw = _tick_analysis(data, pre_close, low_price, close_pos_threshold)
    if raw is None:
        return None
    return TickChartResult(**raw)


def trans_analysis(
    data: list[dict],
    pre_close: float,
    high: float,
    low: float,
) -> Optional[TransactionAnalysisResult]:
    """逐笔成交分析（V4 增强版）。

    返回类型安全的 `TransactionAnalysisResult` dataclass。
    """
    raw = _trans_analysis(data, pre_close, high, low)
    if raw is None:
        return None
    return TransactionAnalysisResult(**raw)


def trans_breakout(
    data: list[dict],
    breakout_price: float,
) -> Optional[BreakoutResult]:
    """突破时刻分析。

    返回类型安全的 `BreakoutResult` dataclass。
    """
    raw = _trans_breakout(data, breakout_price)
    if raw is None:
        return None
    return BreakoutResult(**raw)


def screening_metrics(
    data: list[dict],
) -> Optional[ScreeningMetrics]:
    """逐笔成交筛选指标统计。

    返回类型安全的 `ScreeningMetrics` dataclass。
    """
    raw = _screening_metrics(data)
    if raw is None:
        return None
    return ScreeningMetrics(**raw)


def auction_scan(
    data: list[dict],
    threshold: float = 0.5,
    open_price: Optional[float] = None,
    pre_close: Optional[float] = None,
    limit_up: Optional[float] = None,
    limit_down: Optional[float] = None,
) -> Optional[ScanResult]:
    """集合竞价扫描评分。

    返回类型安全的 `ScanResult` dataclass。
    """
    raw = _auction_scan(data, threshold, open_price, pre_close, limit_up, limit_down)
    if raw is None:
        return None
    return ScanResult(**raw)


def auction_false_sell(
    data: list[dict],
    threshold: float = 0.5,
) -> FalseSellResult:
    """虚假卖出检测（T2 模式）。

    返回类型安全的 `FalseSellResult` dataclass（数据不足时所有字段为默认值）。
    """
    raw = _auction_false_sell(data, threshold)
    if raw is None:
        return FalseSellResult()
    return FalseSellResult(**raw)


__all__ = [
    # 类型化 wrapper
    "tick_analysis",
    "trans_analysis",
    "trans_breakout",
    "screening_metrics",
    "auction_scan",
    "auction_false_sell",
    # 直接透传的信号函数
    "compute_wavetrend",
    "wt_classify_zone",
    "wt_signal",
    "wt_signal_batch",
    "generate_ak_signals",
    "ak_signal_list",
    "generate_all_signals",
    # 工具函数
    "auction_scanner_version",
    "auction_morphology_names",
    "false_sell_version",
    # 类型
    "TickChartResult",
    "TransactionAnalysisResult",
    "BreakoutResult",
    "ScanResult",
    "FalseSellResult",
    "ScreeningMetrics",
]
