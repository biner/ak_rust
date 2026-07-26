"""信号统一调度层 —— 混合 ak_rust + czsc 信号函数。

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

from . import _native
from czsc import RawBar, generate_czsc_signals
import pandas as pd

AK_SIGNALS: set[str] = {
    "ak_wavetrend_V260625",
    "ak_wavetrend_zone_V260724",
    "ak_dmi_state_V260714",
    "ak_ic_state_V260718",
    "ak_ttm_squeeze_V260711",
}

# czsc 列名中的短名映射（key = name 去掉 ak_ 和 _V 后的 base）
_CZSC_SHORT = {
    "wavetrend": "WT",
    "wavetrend_zone": "WT",
    "dmi_state": "DMI",
    "ic_state": "IC",
    "ttm_squeeze": "TTM",
}


def _ak_col_name(cfg: dict) -> str:
    """为 ak_rust 信号构造 czsc 风格的列名: {freq}_D1{short}#{params}_{label}"""
    name: str = cfg["name"]
    freq: str = cfg.get("freq", "")
    # 提取短名
    base = name.replace("ak_", "").split("_V")[0]  # "wavetrend"/"wavetrend_zone"...
    short = _CZSC_SHORT.get(base, base)
    # 提取参数
    p = cfg.get("params", {})
    params = "#".join(str(v) for v in [
        p.get("n1", cfg.get("n1", "")),
        p.get("n2", cfg.get("n2", "")),
        p.get("n3", cfg.get("n3", "")),
    ] if v not in (None, ""))
    # 提取版本标签
    version = name.split("_V", 1)[-1] if "_V" in name else name  # "260625"
    # zone 信号追加 "区位" 标签
    label = f"区位V{version}" if "zone" in base else f"V{version}"
    return f"{freq}_D1{short}#{params}_{label}"


def _czsc_column_name(bars: list[RawBar], cfg: dict, sdt: str, init_n: int) -> str | None:
    """查询 czsc 对某个信号配置的列名（V25→V26 版本回退探测）"""
    name = cfg["name"]
    # 如果已经是 V25 版，直接查
    if "_V25" in name:
        sdf_tmp = generate_czsc_signals(bars, signals_config=[cfg], sdt=sdt, init_n=init_n, df=True)
        extra = [c for c in sdf_tmp.columns if c not in ("open","high","low","close","vol","amount","dt","freq","symbol","id")]
        return extra[0] if extra else None
    # V26 版：先用 V25 版查列名，再把 V25 换成 V26
    name_v25 = name.replace("_V26", "_V25")
    if name_v25 == name:
        return None
    cfg_v25 = {**cfg, "name": name_v25}
    sdf_tmp = generate_czsc_signals(bars, signals_config=[cfg_v25], sdt=sdt, init_n=init_n, df=True)
    extra = [c for c in sdf_tmp.columns if c not in ("open","high","low","close","vol","amount","dt","freq","symbol","id")]
    if extra:
        return extra[0].replace("_V25", "_V26")
    return None


def _param_str(cfg: dict) -> str:
    """从配置中提取参数字符串"""
    p = cfg.get("params", {})
    n1 = p.get("n1", cfg.get("n1", ""))
    n2 = p.get("n2", cfg.get("n2", ""))
    n3 = p.get("n3", cfg.get("n3", ""))
    return f"{n1}#{n2}#{n3}"


def generate_all_signals(
    bars: list[RawBar],
    signals_config: list[dict],
    df: bool = True,
    sdt: str = "20170101",
    init_n: int = 500,
) -> pd.DataFrame:
    """统一信号调度：ak_rust 负责知悉的信号，czsc 负责其余信号。

    参数:
        bars: K 线列表
        signals_config: 信号配置列表
        df: 是否返回 DataFrame（默认 True）
        sdt: 信号开始计算日期，格式 "YYYYMMDD" 或 "YYYY-MM-DD"
        init_n: 预热 K 线数量
    """
    ak_cfg = [s for s in signals_config if s["name"] in AK_SIGNALS]
    czsc_cfg = [s for s in signals_config if s["name"] not in AK_SIGNALS]

    # 1. czsc 负责非 ak 信号
    sdf = generate_czsc_signals(bars, signals_config=czsc_cfg, sdt=sdt, init_n=init_n, df=df)

    # 2. ak_rust 负责自身信号
    if ak_cfg:
        ak_bars = [_bar_to_dict(b) for b in bars]
        ak_results = _native.generate_ak_signals(ak_cfg, ak_bars)

        # 补齐基础列
        if sdf.empty:
            sdf = pd.DataFrame([{"dt": str(b.dt), "close": b.close} for b in bars])

        for idx, r in enumerate(ak_results):
            c = ak_cfg[idx] if idx < len(ak_cfg) else ak_cfg[0]
            val = f"{r['v1']}_{r['v2']}_{r['v3']}"
            col = _ak_col_name(c)
            sdf[col] = val

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
