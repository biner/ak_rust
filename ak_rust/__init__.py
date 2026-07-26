"""ak_rust — A 股量化分析工具箱（Rust 实现）

用法：
    from ak_rust import trans_analysis, generate_all_signals
"""

from .signaling import generate_all_signals

from ._native import (
    tick_analysis,
    trans_analysis,
    trans_breakout,
    auction_scan,
    auction_false_sell,
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

__all__ = [k for k in dir() if not k.startswith("_")]
