"""信号统一调度层 —— 混合 ak_rust + czsc 信号函数。

用法：
    from ak_rust.signaling import generate_all_signals

    signals_config = [
        {"name": "ak_wavetrend_V260625", "freq": "日线",
         "params": {"n1": "50", "n2": "105", "n3": "20"}},
        {"name": "obvm_line_V230610", "freq": "日线", "n": 6, "m": 14},
        {"name": "ak_dmi_state_V250714", "freq": "日线", "n2": 14},
    ]
    sdf = generate_all_signals(bars, signals_config)
"""

from . import _native
from czsc import RawBar, Freq, generate_czsc_signals
import pandas as pd

AK_SIGNALS = {"ak_wavetrend_V260625", "ak_wavetrend_zone_V250724", "ak_dmi_state_V250714", "ak_ic_state_V250718", "ak_ttm_squeeze_V250711"}


def generate_all_signals(bars: list[RawBar], signals_config: list[dict], df: bool = True) -> pd.DataFrame:
    """统一信号调度：ak_rust + czsc 混合。
    
    ak 信号走 ak_rust（WT），czsc 信号走 czsc（OBV/DMI/VOL 等）。
    """
    ak_cfg = [s for s in signals_config if s["name"] in AK_SIGNALS]
    czsc_cfg = [s for s in signals_config if s["name"] not in AK_SIGNALS]

    sdf = pd.DataFrame()
    if czsc_cfg:
        sdf = generate_czsc_signals(bars, signals_config=czsc_cfg, df=True)

    if ak_cfg:
        ak_bars = [_bar_to_dict(b) for b in bars]
        ak_results = _native.generate_ak_signals(ak_cfg, ak_bars)
        if sdf.empty:
            sdf = pd.DataFrame([{"dt": str(b.dt), "close": b.close} for b in bars])
        for idx, r in enumerate(ak_results):
            c = ak_cfg[idx] if idx < len(ak_cfg) else ak_cfg[0]
            n1 = c.get("params", {}).get("n1", "")
            n2 = c.get("params", {}).get("n2", "")
            n3 = c.get("params", {}).get("n3", "")
            col = f"{r['name']}#{n1}#{n2}#{n3}_{r['freq']}"
            sdf[col] = f"{r['v1']}_{r['v2']}_{r['v3']}"

    return sdf


def _bar_to_dict(b: RawBar) -> dict:
    return {
        "dt": str(b.dt),
        "open": float(b.open),
        "high": float(b.high),
        "low": float(b.low),
        "close": float(b.close),
        "vol": float(b.vol),
        "amount": float(b.amount),
        "freq": str(b.freq),
    }
