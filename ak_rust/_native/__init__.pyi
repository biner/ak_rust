"""ak_rust._native — Rust 扩展模块类型存根（供 IDE/PyLance 使用）"""
from typing import Any, Optional

# ── tick 量价分析 ──
def tick_analysis(
    data: list[dict[str, Any]],
    pre_close: float,
    low_price: float | None = None,
    close_pos_threshold: float = 0.3,
) -> dict[str, Any] | None: ...

# ── 逐笔成交分析 ──
def trans_analysis(
    data: list[dict[str, Any]],
    pre_close: float,
    high: float,
    low: float,
) -> dict[str, Any] | None: ...

# ── 突破时刻分析 ──
def trans_breakout(
    data: list[dict[str, Any]],
    breakout_price: float,
) -> dict[str, Any] | None: ...

# ── 集合竞价 ──
def auction_scan(
    data: list[dict[str, Any]],
    threshold: float = 0.5,
    open_price: float | None = None,
    pre_close: float | None = None,
    limit_up: float | None = None,
    limit_down: float | None = None,
) -> dict[str, Any] | None: ...

def auction_false_sell(
    data: list[dict[str, Any]],
    threshold: float = 0.5,
) -> dict[str, Any] | None: ...

def auction_scanner_version() -> str: ...

def auction_morphology_names() -> list[str]: ...

def false_sell_version() -> str: ...

# ── 筛选指标 ──
def screening_metrics(
    data: list[dict[str, Any]],
) -> dict[str, Any] | None: ...

# ── WaveTrend 信号 ──
def compute_wavetrend(
    high: list[float],
    low: list[float],
    close: list[float],
    n1: int = 10,
    n2: int = 21,
    n3: int = 4,
) -> list[list[float]]: ...

def wt_classify_zone(wt1: float) -> str: ...

def wt_signal(
    high: list[float],
    low: list[float],
    close: list[float],
    di: int = 1,
    n1: int = 10,
    n2: int = 21,
    n3: int = 4,
) -> dict[str, str]: ...

def wt_signal_batch(
    high: list[float],
    low: list[float],
    close: list[float],
    n1: int = 10,
    n2: int = 21,
    n3: int = 4,
) -> list[dict[str, str]]: ...

# ── 信号调度 ──
def generate_ak_signals(
    signals_config: list[dict[str, Any]],
    bars: list[dict[str, Any]],
) -> list[dict[str, str]]: ...

def ak_signal_list() -> list[str]: ...

# ── 信号统一调度（混合 ak_rust + czsc）──
def generate_all_signals(
    bars: list[Any],
    signals_config: list[dict[str, Any]],
    sdt: str = "20170101",
    init_n: int = 500,
    df: bool = True,
) -> Any: ...
