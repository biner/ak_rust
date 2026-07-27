"""信号统一调度层 —— 混合 ak_rust + czsc 信号函数。

Ak_rust 负责自身信号，其余委托 czsc 处理（列名完全兼容 czsc 格式）。

用法：
    from ak_rust.signaling import generate_all_signals

    signals_config = [
        {"name": "ak_wavetrend_V260625", "freq": "日线",
         "params": {"n1": "50", "n2": "105", "n3": "20"}},
        {"name": "obvm_line_V230610", "freq": "日线", "n": 6, "m": 14},
        {"name": "ak_dmi_state_V260714", "freq": "日线", "n2": 14},
    ]
    sdf = generate_all_signals(bars, signals_config)
"""

from datetime import datetime

from czsc import RawBar, Freq

from ._native import generate_all_signals as _generate_all_signals


def _ensure_rawbar(bars: list) -> list:
    """统一转为 RawBar 对象，兼容 dict 和 RawBar 两种输入。"""
    if not bars:
        return bars
    if not isinstance(bars[0], dict):
        return bars
    result = []
    for i, b in enumerate(bars):
        dt = b["dt"] if isinstance(b["dt"], datetime) else datetime.fromisoformat(str(b["dt"]))
        freq_str = b.get("freq", "日线")
        freq = Freq(freq_str) if isinstance(freq_str, str) and not isinstance(freq_str, Freq) else freq_str
        result.append(RawBar(
            symbol=b.get("symbol", "NONE"),
            id=i,
            dt=dt,
            freq=freq,
            open=float(b["open"]),
            high=float(b["high"]),
            low=float(b["low"]),
            close=float(b["close"]),
            vol=float(b.get("vol", 0)),
            amount=float(b.get("amount", 0)),
        ))
    return result


def generate_all_signals(
    bars: list,
    signals_config: list[dict],
    df: bool = True,
    sdt: str = "20170101",
    init_n: int = 500,
):
    """统一信号调度（Rust 原生实现）。

    参数:
        bars: K 线列表（RawBar 对象或 dict）
        signals_config: 信号配置列表
        df: 是否返回 DataFrame（默认 True）
        sdt: 信号开始计算日期，格式 "YYYYMMDD" 或 "YYYY-MM-DD"
        init_n: 预热 K 线数量
    """
    return _generate_all_signals(_ensure_rawbar(bars), signals_config, sdt, init_n, df)
