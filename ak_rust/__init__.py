"""ak_rust — A 股量化分析工具箱（Rust 实现）

用法：
    from ak_rust import analyze_transactions, generate_all_signals
"""

from . import _native
from .signaling import generate_all_signals

from ._native import (
    analyze_tick_chart,
    analyze_transactions,
    analyze_transactions_v2,
    analyze_transactions_v4,
    analyze_breakout,
    scanner_auction,
    scan_false_sell,
    auction_scanner_version,
    auction_morphology_names,
    false_sell_version,
    compute_wavetrend,
    wt_classify_zone,
    wt_signal,
    wt_signal_batch,
    generate_ak_signals,
    ak_signal_list,
    ak_init_builtins,
    ak_register_signal,
)

__all__ = [k for k in dir() if not k.startswith("_")]
