"""WaveTrend 金叉突破策略 v20 —— ttm_v18 基础上增加 openkph 大单/对倒 K线过滤

相对 ttm_v18 新增：
  - 新增 openkph 大单净额 K线过滤（加法评分）
  - 新增 openkph 对倒金额 K线过滤
  - 新增 openkph 托单/压单 K线过滤
  - 新增 openkph 竞价资金净额 K线过滤
  - 新增 openkph 主力活跃度 K线过滤

用法：
    uv run python -m app.czsc.strategy.ttm.ttm_v20 000596
"""


from __future__ import annotations

import datetime
import numpy as np
import pandas as pd
from app.domain.symbol import Symbol
from ak_rust._native import (
    tick_analysis,         # tick 量价分析
    trans_analysis,    # v4: 增强版
)
from opentdx.const import ADJUST
# from czsc import RawBar, Freq, generate_czsc_signals
from ak_rust.signaling import generate_all_signals as generate_czsc_signals
from app.domain.symbol import Symbol
from app.czsc.strategy.strategy_utils import get_price_fluctuation
from app.dal import SymbolBacktradeDal
from app.utils.my_ta import BOLL, WAVETREND
from app.client.kph_client import kph_client

# ── 策略参数 ──
JUMP_MULT = 8
MIN_AMOUNT = 100_000_000  # 1亿成交额门槛

# ── TTM 信号常量 ──
ENTRY_V2 = {"负值翻转强势", "负值翻转次日", "金叉", "金叉次日", "二次上升强势"}
ENTRY_AGEST = {"负值翻转强势", "负值翻转次日"}
EXIT_TTM = {"正值翻转"}
EXIT_V2 = {"死叉"}

# ── 信号值提取 ──

def get_v1(s):
    if not isinstance(s, str):
        return "其他"
    return s.rsplit("_", 3)[0]

def get_v2(s):
    if not isinstance(s, str):
        return "其他"
    return s.rsplit("_", 3)[1]

def get_v3(s):
    if not isinstance(s, str):
        return "其他"
    parts = s.rsplit("_", 3)
    return parts[2] if len(parts) > 2 else "其他"


# ── 信号配置 ──

def get_signals_config():
    return [
        # TTM 信号
        {"name": "ak_ttm_squeeze_V250711", "freq": "日线", "n1": 20},
        # {"name": "skdj_up_dw_line_V230611", "freq": "日线", "n": 233, "m": 89, "up": 60, "dw": 40},
        # 倍数WT (n1=50, n2=105, n3=20) → v1=区位, v2=WTCD方向, v3=金叉/死叉/多头/空头
        {"name": "ak_wavetrend_V260625", "freq": "日线", "n1": 50, "n2": 105, "n3": 20},
        # 标准WT (n1=10, n2=21, n3=4) → v1=区位, v3=金叉/死叉/多头/空头
        {"name": "ak_wavetrend_V260625", "freq": "日线", "n1": 10, "n2": 21, "n3": 4},
        # 辅助判断
        {"name": "ak_wavetrend_zone_V250724", "freq": "日线", "n1": 10, "n2": 21, "n3": 4},
        {"name": "obvm_line_V230610", "freq": "日线", "n": 6, "m": 14},
        {"name": "ak_dmi_state_V250714", "freq": "日线", "n2": 14},
        {"name": "ak_vol_ratio_V250721", "freq": "日线", "n": 20},
    ]


# ── 核心：单只股票回测 ──

def run_one(symbol: str, verbose: bool = True) -> list[dict]:
    """WaveTrend 金叉突破策略 — 单只股票回测。

    参数
    ----------
    symbol : str
        股票代码，如 "000596"、"688008"
    verbose : bool
        是否打印策略报告（默认 True 以输出策略报告）

    返回
    -------
    list[dict]
        每笔交易的字段：signal_date, entry_date, exit_date,
        signal, entry_price, exit_price, return, return_pct, holding_days
    """
    print(f"\n{'=' * 60}")
    print(f"  WaveTrend 金叉突破策略 v20 — {symbol}")
    print(f"{'=' * 60}")

    # ── 1. 获取数据 ──
    try:
        sym_obj = Symbol(symbol)
        df = sym_obj.get_tdx_symbol_bars_cache(myunit="1d", count=800, fq=ADJUST.QFQ)
        if df.empty:
            print("  ❌ 数据获取失败 - 空数据")
            return []
    except Exception as e:
        print(f"  ❌ 数据获取失败 - {e}")
        return []

    bars: list[RawBar] = []
    for d in df.itertuples():
        bars.append(RawBar(
            symbol=symbol, id=d.Index, dt=d.datetime, freq=Freq.D,
            open=d.open, high=d.high, low=d.low, close=d.close,
            vol=getattr(d, "volume", getattr(d, "vol", 0)), amount=d.amount,
        ))

    if len(bars) < 60:
        print("  ❌ K线数据不足 (＜60)")
        return []

    print(f"  日线: {bars[0].dt.date()} ~ {bars[-1].dt.date()} ({len(bars)} bars)")

    # ── 2. 生成信号 ──
    try:
        sdf = generate_czsc_signals(bars, signals_config=get_signals_config(), df=True)
    except Exception as e:
        print(f"  ❌ 信号生成失败 - {e}")
        return []

    sdf["close"] = sdf["close"].astype(float)
    sdf["amount"] = sdf["amount"].astype(float)
    sdf["dt"] = pd.to_datetime(sdf["dt"])

    # ── 3. 定位信号列 ──
    wt_scaled_col = [c for c in sdf.columns if "WT#50#105#20_V260625" in c][0]  # 倍数WT
    wt_std_col = [c for c in sdf.columns if "WT#10#21#4_V260625" in c][0]      # 标准WT
    wt_zone_col = [c for c in sdf.columns if "区位" in c][0]                   # WT区位
    obv_col = [c for c in sdf.columns if "OBV能量V230610" in c][0]
    dmi_col = [c for c in sdf.columns if "DMI#" in c and "TDX" not in c][0]
    vol_col = [c for c in sdf.columns if "VOL#" in c][0]

    print(f"\n  信号列映射:")
    print(f"    倍数WT: {wt_scaled_col} → v1(区位)/v2(WTCD)/v3(金叉死叉)")
    print(f"    标准WT: {wt_std_col} → v1(区位)/v3(金叉死叉)")

    # ── 4. 信号值提取 ──
    sdf["wt_scaled_v1"] = sdf[wt_scaled_col].apply(get_v1)  # 倍数WT区位
    sdf["wt_scaled_v2"] = sdf[wt_scaled_col].apply(get_v2)  # 倍数WTCD方向
    sdf["wt_scaled_v3"] = sdf[wt_scaled_col].apply(get_v3)  # 倍数WT金叉/死叉/多头/空头
    sdf["wt_std_v1"] = sdf[wt_std_col].apply(get_v1)        # 标准WT区位
    sdf["wt_std_v3"] = sdf[wt_std_col].apply(get_v3)        # 标准WT金叉/死叉/多头/空头
    sdf["wt_zone_v1"] = sdf[wt_zone_col].apply(get_v1)      # WT区位（0轴上下）
    sdf["obv_v1"] = sdf[obv_col].apply(get_v1)
    sdf["dmi_v1"] = sdf[dmi_col].apply(get_v1)

    # ── TTM 信号列 ──
    ttm_col = [c for c in sdf.columns if "V250711" in c]
    if ttm_col:
        ttm_col = ttm_col[0]
        sdf["ttm_v1"] = sdf[ttm_col].apply(get_v1)
        sdf["ttm_v2"] = sdf[ttm_col].apply(get_v2)
        sdf["ttm_v3"] = sdf[ttm_col].apply(get_v3)
    else:
        sdf["ttm_v1"] = "其他"
        sdf["ttm_v2"] = "其他"
        sdf["ttm_v3"] = "其他"
    sdf["dmi_v2"] = sdf[dmi_col].apply(get_v2)
    sdf["dmi_v3"] = sdf[dmi_col].apply(get_v3)
    sdf["vol_v1"] = sdf[vol_col].apply(get_v1)

    # ── 5. 换手率 + WT1上行 ──
    sdf["turnover"] = df["turnover"].iloc[-len(sdf):].values
    sdf["turnover_rising"] = sdf["turnover"] > sdf["turnover"].shift(1)
    sdf["wt1_rising"] = sdf["wt_scaled_v2"] == "增大"

    # ── EMA100/EMA200 计算（用于收盘位置动态阈值）──
    closes_series = pd.Series([b.close for b in bars])
    ema100_all = closes_series.ewm(span=100, adjust=False).mean().values
    ema200_all = closes_series.ewm(span=200, adjust=False).mean().values
    sdf["ema100"] = ema100_all[-len(sdf):]
    sdf["ema200"] = ema200_all[-len(sdf):]

    # ── 布林带中轴计算（默认参数 N=20, P=2）──
    _, middle_all, _ = BOLL(closes_series)
    sdf["boll_mid"] = middle_all[-len(sdf):]

    # ── WT1 数值计算（标准WT n1=10, n2=21，取信号区末尾段）──
    highs_all = pd.Series([b.high for b in bars])
    lows_all = pd.Series([b.low for b in bars])
    wt1_all, wt2_all, _ = WAVETREND(highs_all, lows_all, closes_series, N1=10, N2=21)
    sdf["wt1_val"] = wt1_all[-len(sdf):]  # WT1 原始数值，用于 -30~30 中性区过滤

    # ── 所属行业板块的MA5计算（板块强度过滤，按日期对齐）──
    board_close_map = None  # dict: date_str → close
    board_ma5_cache = {}    # dict: date_str → ma5 (惰性计算)
    board_symbol_str = ""
    try:
        from app.helpers.stock_helper import get_tdx_board_quotes
        quotes_df = get_tdx_board_quotes('A股')
        match = quotes_df[quotes_df["code"] == symbol]
        if len(match) > 0:
            row = match.iloc[0]
            board_symbol_str = str( row.get("industry_symbol", ""))
        if board_symbol_str and board_symbol_str.startswith("88"):
            board_sym = Symbol(board_symbol_str)
            board_df = board_sym.get_tdx_symbol_bars_cache(myunit="1d", count=800, fq=ADJUST.QFQ)
            if board_df is not None and len(board_df) > 20:
                # 构建 date → close 映射
                board_dates = pd.to_datetime(board_df["datetime"]).map(lambda x: x.strftime("%Y-%m-%d"))
                board_closes = board_df["close"].values
                board_close_map = dict(zip(board_dates, board_closes))
                # 用 MyTA.MA 计算板块MA5（基于talib.SMA）
                from app.utils.my_ta import MA as _MA
                board_ma5_series = _MA(board_df["close"], timeperiod=5)
                for idx, dt_str in enumerate(board_dates):
                    val = board_ma5_series.iloc[idx]
                    if not np.isnan(val):
                        board_ma5_cache[dt_str] = val
    except Exception:
        pass

    # ── openkph 大单/对倒 K线数据（日线级，按日期对齐）──
    dadan_net_map: dict[str, int] = {}
    tuoyadan_map: dict[str, tuple[int, int]] = {}
    duidao_map: dict[str, int] = {}
    bidvol_map: dict[str, tuple[int, int]] = {}
    main_activity_map: dict[str, int] = {}
    try:
        dadan_net_map = kph_client.get_stock_dadan_kline_map(symbol, period=1, count=130)
        tuoyadan_map = kph_client.get_stock_tuoyadan_kline_map(symbol, period=1, count=130)
        duidao_map = kph_client.get_stock_duidao_kline_map(symbol, period=1, count=130)
        bidvol_map = kph_client.get_stock_bidvol_kline_map(symbol, period=1, count=130)
        main_activity_map = kph_client.get_stock_main_activity_map(symbol, period=1, count=130)
        if verbose:
            print(f"  openkph 大单净额: {len(dadan_net_map)}天, 托单压单: {len(tuoyadan_map)}天")
    except Exception:
        pass

    sdf_bars_idx = list(range(len(bars) - len(sdf), len(bars)))

    # ── 6. 状态机 ──
    holding = False
    plan_entry_price: float | None = None
    plan_cancel_price: float | None = None
    plan_signal_bar: int | None = None
    plan_signal_tag: str | None = None  # "TTM金叉" 或 "TTM金叉次日"
    trades: list[dict] = []
    death_cross_stop: float | None = None
    max_high_since_entry: float = 0.0
    entry_turnover: float | None = None
    prev_gc_blocked_by_tx = False  # 上一根bar的金叉是否被analyze_transactions拦截
    gc_detected_no_trade = False   # 金叉已检测但未产生成交

    # 零轴上下区域
    _below_zero = {"弱空", "偏空", "超卖", "极度超卖"}
    _above_zero = {"弱多", "偏多", "超买", "极度超买"}

    for i in range(len(sdf)):
        bar_i = sdf_bars_idx[i]
        bar = bars[bar_i]
        date_str = bar.dt.strftime("%Y-%m-%d")

        if holding and bar.high > max_high_since_entry:
            max_high_since_entry = bar.high

        wt_golden_cross = sdf["wt_std_v3"].iloc[i] == "金叉"
        wt_death_cross = sdf["wt_std_v3"].iloc[i] == "死叉"
        turnover_rising = sdf["turnover_rising"].iloc[i]

        # ── wt1_rising: 默认要求倍速WT方向增大 ──
        #     但如果倍速WT是多头且在零轴上，说明大周期看多，不强求方向增大
        wt_scaled_v3_val = get_v3(sdf[wt_scaled_col].iloc[i])
        wt_scaled_v1_val = get_v1(sdf[wt_scaled_col].iloc[i])
        _above_zero = {"弱多", "偏多", "超买", "极度超买"}
        if wt_scaled_v3_val == "多头" and wt_scaled_v1_val in _above_zero:
            wt1_rising = True
        else:
            wt1_rising = sdf["wt1_rising"].iloc[i]

        # TTM 二次上升强势检测（类似WT上穿零轴）
        wt_cross_zero = False
        if i >= 1:
            prev_zone = sdf["wt_std_v1"].iloc[i - 1]
            cur_zone = sdf["wt_std_v1"].iloc[i]
            wt_cross_zero = prev_zone in _below_zero and cur_zone in _above_zero

        # 记录上一根bar的金叉是否被逐笔分析拦截（用于次日信号判断）
        _prev_gc_blocked = prev_gc_blocked_by_tx

        entry_signal = None
        if not holding and plan_entry_price is None:
            # TTM 信号检测
            ttm_v2_val = sdf["ttm_v2"].iloc[i] if "ttm_v2" in sdf.columns else "其他"
            ttm_in_entry = ttm_v2_val in ENTRY_V2
            if ttm_in_entry and turnover_rising and wt1_rising:
                entry_signal = "TTM金叉"
                gc_detected_no_trade = True
            elif _prev_gc_blocked and not ttm_in_entry and turnover_rising and wt1_rising:
                entry_signal = "TTM金叉次日"
            elif ttm_v2_val == "二次上升强势" and gc_detected_no_trade and not holding and turnover_rising and wt1_rising:
                entry_signal = "二次上升强势"
                gc_detected_no_trade = False

        if entry_signal is not None:
            # ── P0-1. 超买区位过滤：超买/极度超买区位不开多 ──
            _overbought = {"超买", "极度超买"}
            wt_std_v1_val = sdf["wt_std_v1"].iloc[i]
            if wt_std_v1_val in _overbought:
                print(f"  [{date_str}] ⛔ {entry_signal}但处于{wt_std_v1_val}区位，跳过")
                prev_gc_blocked_by_tx = False
                entry_signal = None
                continue

            # ── P0-2. 中轨下行过滤：倍速WTCD方向连续3天"减小"时不开仓 ──
            if i >= 2:
                wt2_v2_0 = sdf["wt_scaled_v2"].iloc[i]
                wt2_v2_1 = sdf["wt_scaled_v2"].iloc[i - 1]
                wt2_v2_2 = sdf["wt_scaled_v2"].iloc[i - 2]
                if (wt2_v2_0 == "减小" and wt2_v2_1 == "减小" and wt2_v2_2 == "减小"):
                    print(f"  [{date_str}] ⛔ {entry_signal}但倍速WTCD连续3天减小(中轨下行)，跳过")
                    prev_gc_blocked_by_tx = False
                    entry_signal = None
                    continue

            # ── P0-3. 布林带中轴过滤：收盘价在中轴之下不开多 ──
            boll_mid_val = sdf["boll_mid"].iloc[i]
            if pd.notna(boll_mid_val) and bar.close < boll_mid_val:
                print(f"  [{date_str}] ⛔ {entry_signal}但收盘价{bar.close:.2f}<布林中轴{boll_mid_val:.2f}，跳过")
                prev_gc_blocked_by_tx = False
                entry_signal = None
                continue

            # ── P0-5. 板块MA5过滤：所属行业板块收盘价 > 板块MA5（按日期对齐）──
            if board_close_map is not None and date_str in board_close_map:
                board_close = board_close_map[date_str]
                board_ma5 = board_ma5_cache.get(date_str)
                if board_ma5 is not None and board_close <= board_ma5:
                    print(f"  [{date_str}] ⛔ {entry_signal}但行业板块({board_symbol_str})收盘{board_close:.2f}≤MA5{board_ma5:.2f}，跳过")
                    prev_gc_blocked_by_tx = False
                    entry_signal = None
                    continue

            # 空头趋势下金叉日涨幅 < 2% 时拒绝开仓（弱反弹，高概率亏损）
            # 但如果倍数WT的WT1>0且WTCD增大，则豁免（趋势转强中）
            if entry_signal == "TTM金叉":
                wt_scaled_v3 = get_v3(sdf[wt_scaled_col].iloc[i])
                if wt_scaled_v3 == "空头":
                    wt_scaled_v1 = get_v1(sdf[wt_scaled_col].iloc[i])
                    wt_scaled_v2 = get_v2(sdf[wt_scaled_col].iloc[i])
                    _above_zero = {"弱多", "偏多", "超买", "极度超买"}
                    if wt_scaled_v1 in _above_zero and wt_scaled_v2 == "增大":
                        pass  # 倍数WT已转强（零轴上且方向增大），豁免空头+小阳线判断
                    else:
                        day_chg = (bar.close - bars[bar_i - 1].close) / bars[bar_i - 1].close * 100
                        if day_chg < 2.0:
                            print(f"  [{date_str}] ⛔ {entry_signal}但倍数WT空头+小阳线({day_chg:+.1f}%)，跳过")
                            prev_gc_blocked_by_tx = True
                            continue

                # ── P0-4. TTM金叉时WT1不在-30~30中性区（弱信号区不开仓）──
                wt1_val = sdf["wt1_val"].iloc[i]
                if pd.notna(wt1_val) and -30 <= wt1_val <= 30:
                    print(f"  [{date_str}] ⛔ {entry_signal}但WT1={wt1_val:.1f}在[-30,30]中性区，跳过")
                    prev_gc_blocked_by_tx = True
                    continue

            amount = sdf["amount"].iloc[i]
            if int(amount) < MIN_AMOUNT:
                entry_signal = None
                prev_gc_blocked_by_tx = False
                continue

            # 前日换手率 < 1% 时拒绝开仓（冷门股，金叉可靠性低）
            if i >= 1 and sdf["turnover"].iloc[i - 1] < 0.01:
                print(f"  [{date_str}] ⛔ {entry_signal}但前日换手率<1%(冷门)，跳过")
                prev_gc_blocked_by_tx = False
                continue

            # 入场换手率 > 10% 时拒绝开仓（高换手率主力出货概率大，胜率仅 30%）
            entry_turnover = sdf["turnover"].iloc[i]
            if entry_turnover > 10.0:
                print(f"  [{date_str}] ⛔ {entry_signal}但换手率{entry_turnover:.1f}%>10%，跳过")
                prev_gc_blocked_by_tx = False
                continue

            # K线模式过滤：低低模式(dd)时拒绝开仓
            if bar_i >= 1:
                prev_bar = bars[bar_i - 1]
                dd_mode = (bar.high < prev_bar.high and bar.low < prev_bar.low)
                if dd_mode:
                    print(f"  [{date_str}] ⛔ {entry_signal}但为低低K线模式，跳过")
                    prev_gc_blocked_by_tx = False
                    continue

            # 距EMA100 < -5% 时拒绝开仓（深度跌破均线，超跌反弹陷阱）
            dist_ema100 = (bar.close - sdf["ema100"].iloc[i]) / sdf["ema100"].iloc[i] * 100
            if dist_ema100 < -5:
                print(f"  [{date_str}] ⛔ {entry_signal}但距EMA100仅{dist_ema100:.1f}%(<-5%)，跳过")
                prev_gc_blocked_by_tx = False
                continue

            # ── openkph 大单/对倒综合评分（加法评分，各维度独立贡献）──
            openkph_score = 0
            openkph_details = []

            # D1: 大单净额
            dadan_net = dadan_net_map.get(date_str)
            if dadan_net is not None:
                if dadan_net > 0:
                    openkph_score += 1
                    openkph_details.append(f"大单净额{dadan_net/1e4:.0f}万👍")
                else:
                    openkph_score -= 1
                    openkph_details.append(f"大单净额{dadan_net/1e4:.0f}万👎")
            else:
                openkph_details.append("大单净额无数据")

            # D2: 对倒金额占比
            duidao_amt = duidao_map.get(date_str)
            amount_today = float(sdf["amount"].iloc[i])
            if duidao_amt is not None and amount_today > 0:
                duidao_ratio = duidao_amt / amount_today
                if duidao_ratio < 0.1:
                    openkph_score += 1
                    openkph_details.append(f"对倒{duidao_ratio:.0%}👍")
                elif duidao_ratio < 0.3:
                    openkph_details.append(f"对倒{duidao_ratio:.0%}⚠️")
                else:
                    openkph_score -= 1
                    openkph_details.append(f"对倒{duidao_ratio:.0%}👎")
            else:
                openkph_details.append("对倒无数据")

            # D3: 托单vs压单
            tyd = tuoyadan_map.get(date_str)
            if tyd is not None:
                tuo_amt, ya_amt = tyd
                if tuo_amt == 0 and ya_amt == 0:
                    openkph_details.append("托压单无数据")
                elif tuo_amt > ya_amt:
                    openkph_score += 1
                    openkph_details.append(f"托{tuo_amt/1e4:.0f}万>压{ya_amt/1e4:.0f}万👍")
                else:
                    openkph_score -= 1
                    openkph_details.append(f"托{tuo_amt/1e4:.0f}万≤压{ya_amt/1e4:.0f}万👎")
            else:
                openkph_details.append("托压无数据")

            # D4: 竞价资金净额
            bv = bidvol_map.get(date_str)
            if bv is not None:
                _, bid_net = bv
                if bid_net > 0:
                    openkph_score += 1
                    openkph_details.append(f"竞价净额{bid_net/1e4:.0f}万👍")
                else:
                    openkph_score -= 1
                    openkph_details.append(f"竞价净额{bid_net/1e4:.0f}万👎")
            else:
                openkph_details.append("竞价无数据")

            # D5: 主力活跃度
            activity = main_activity_map.get(date_str)
            if activity is not None:
                if activity > 50:
                    openkph_score += 1
                    openkph_details.append(f"活跃度{activity}🔥")
                elif activity > 0:
                    openkph_details.append(f"活跃度{activity}✓")
                else:
                    openkph_score -= 1
                    openkph_details.append(f"活跃度{activity}👎")
            else:
                openkph_details.append("活跃度无数据")

            # 综合评分：总分 ≥ 0 才开仓
            if openkph_score < 0:
                print(f"  [{date_str}] ⛔ {entry_signal}+openkph总分{openkph_score}<0，跳过")
                for d in openkph_details:
                    print(f"    📊 {d}")
                prev_gc_blocked_by_tx = False
                continue
            elif openkph_score < 2 and verbose:
                print(f"  [{date_str}] 📊 {entry_signal}+openkph总分{openkph_score}（偏低）")
                for d in openkph_details:
                    print(f"    📊 {d}")

            g = get_price_fluctuation(bar.high)
            plan_entry_price = bar.high + g * JUMP_MULT
            plan_signal_bar = bar_i
            plan_signal_tag = entry_signal
            llv2 = min(bar.low, bars[bar_i - 1].low) if bar_i > 0 else bar.low
            plan_cancel_price = llv2 - g * JUMP_MULT
            entry_turnover = sdf["turnover"].iloc[i]

            # Rust tick质量分析：确定阈值 → 拉取tick数据 → Rust分析 → 撤单决策
            close_pos_threshold = 0.3
            above_ema = bar.low > sdf["ema100"].iloc[i] and bar.low > sdf["ema200"].iloc[i]
            if above_ema:
                close_pos_threshold = 0.2

            # 检查前一日tick特征：从bar数据获取前一日日期，前日已大涨(close_pos>0.5且涨幅>0%)则收紧阈值
            try:
                prev_bar = bars[bar_i - 1]
                prev_date_str = prev_bar.dt.strftime("%Y-%m-%d") if hasattr(prev_bar.dt, 'strftime') else str(prev_bar.dt)[:10]
                prev_tick = Symbol(symbol).get_symbol_tick_chart(query_date=prev_bar.dt)
                if prev_tick and prev_tick.get("charts"):
                    prev_charts = prev_tick["charts"]
                    prev_prices = [c["price"] for c in prev_charts]
                    prev_high = max(prev_prices)
                    prev_low = min(prev_prices)
                    prev_close = prev_tick["close"]
                    prev_pre_close = prev_tick["pre_close"]
                    prev_range = prev_high - prev_low
                    prev_close_pos = (prev_close - prev_low) / prev_range if prev_range > 0 else 0.5
                    prev_chg = (prev_close - prev_pre_close) / prev_pre_close * 100 if prev_pre_close > 0 else 0
                    if prev_close_pos > 0.5 and prev_chg > 0:
                        old_threshold = close_pos_threshold
                        close_pos_threshold = round(close_pos_threshold + 0.05, 2)
                        print(f"  [{date_str}] 前日已涨{prev_chg:.1f}%(收盘位{prev_close_pos:.2f})，阈值收紧{old_threshold}→{close_pos_threshold}")
            except Exception:
                pass

            # 拉取信号日的分钟级tick数据
            tick_data = []
            try:
                from datetime import date as dt_date
                sig_date = dt_date.fromisoformat(date_str)
                tick_raw = Symbol(symbol).get_symbol_tick_chart(query_date=sig_date)
                if tick_raw and tick_raw.get("charts"):
                    for c in tick_raw["charts"]:
                        tick_data.append({
                            "time": str(c["time"]),
                            "price": float(c["price"]),
                            "avg": float(c.get("avg", c["price"])),
                            "vol": int(c.get("vol", 0)),
                            "momentum": float(c.get("momentum", 0)),
                        })
            except Exception:
                pass

            should_cancel = False
            cancel_reason = ""
            _tick_chart_result = None  # 保存tick_chart结果用于输出
            if tick_data:
                try:
                    result = tick_analysis(
                        tick_data, float(bar.close),
                        float(bar.low), close_pos_threshold,
                    )
                    if result is not None:
                        should_cancel = result["should_cancel"]
                        cancel_reason = result["cancel_reason"]
                        _tick_chart_result = result
                except Exception as e:
                    print(f"  [{date_str}] ❌ Rust tick质量分析失败({e})，降级为Python内联计算")
                    # Rust不可用时降级到Python内联计算
                    total_range = bar.high - bar.low
                    close_pos = (bar.close - bar.low) / total_range if total_range > 0 else 0.5
                    if close_pos < close_pos_threshold:
                        should_cancel = True
                        cancel_reason = f"收盘位置仅{close_pos:.2f}(<{close_pos_threshold})"
            else:
                # 无tick数据时，降级到Python内联计算
                total_range = bar.high - bar.low
                close_pos = (bar.close - bar.low) / total_range if total_range > 0 else 0.5
                if close_pos < close_pos_threshold:
                    should_cancel = True
                    cancel_reason = f"收盘位置仅{close_pos:.2f}(<{close_pos_threshold})"

            if should_cancel:
                label = "Rust" if tick_data else "Python"
                print(f"  [{date_str}] ⛔ {plan_signal_tag}[{label}]{cancel_reason}，撤单")
                plan_entry_price = None
                plan_signal_bar = None
                plan_cancel_price = None
                prev_gc_blocked_by_tx = False
                continue

            # 逐笔成交数据分析：使用 Rust analyze_transactions 判断真假突破
            _tx_result = None  # 保存transactions结果用于输出
            try:
                from datetime import date as dt_date
                sig_date = dt_date.fromisoformat(date_str)
                trans_raw = Symbol(symbol).get_symbol_transactions(query_date=sig_date)
                if trans_raw and len(trans_raw) >= 100:
                    tx_list = [{"time": str(t["time"]), "price": float(t["price"]),
                                "vol": int(t["vol"]), "trade_count": int(t.get("trade_count", 1)),
                                "bs_flag": int(t.get("bs_flag", 0))} for t in trans_raw]
                    high_actual = max(t["price"] for t in trans_raw)
                    low_actual = min(t["price"] for t in trans_raw)
                    tx_result = trans_analysis(tx_list, pre_close=float(bar.close),
                                                     high=float(high_actual), low=float(low_actual))
                    _tx_result = tx_result
                    if tx_result is not None and tx_result["conclusion"] in ("疑似假突破", "假突破"):
                        print(f"  [{date_str}] ⛔ {plan_signal_tag}+逐笔成交分析: {tx_result['conclusion']}({tx_result['confidence']:.2f})，撤单")

                        print(f"    🔍 明细: 主动买比={tx_result['buy_vol_ratio']:.2f}, "
                              f"大单买/卖={tx_result['large_buy_count']}/{tx_result['large_sell_count']}, "
                              f"收盘位={tx_result['price_position']:.2f}, "
                              f"尾盘动量={tx_result['last_30min_momentum']:.3f}, "
                              f"置信度={tx_result['confidence']:.2f}")
                        plan_entry_price = None
                        plan_signal_bar = None
                        plan_cancel_price = None
                        prev_gc_blocked_by_tx = True
                        continue
            except Exception:
                pass

            print(f"  [{date_str}] 🟢 {plan_signal_tag}+量能确认+WT1上行 "
                  f"→ 计划单: 突破 {plan_entry_price:.2f} 取消 {plan_cancel_price:.2f}")

        # ── 计划单检查 ──
        if not holding and plan_entry_price is not None and plan_signal_bar is not None and bar_i > plan_signal_bar:
            # 检查信号日到当前之间是否出现死叉（如果有则取消计划单）
            for ci in range(plan_signal_bar + 1, bar_i + 1):
                # 找到 ci 在 sdf 中的信号索引
                try:
                    sig_idx = sdf_bars_idx.index(ci)
                    if sdf["wt_std_v3"].iloc[sig_idx] == "死叉":
                        print(f"    [{bars[ci].dt.strftime('%Y-%m-%d')}] 计划单取消 (死叉)")
                        plan_entry_price = None
                        plan_cancel_price = None
                        plan_signal_bar = None
                        break
                except ValueError:
                    pass
                # 原有跌破取消价检查
                if bars[ci].low <= plan_cancel_price:
                    print(f"    [{bars[ci].dt.strftime('%Y-%m-%d')}] 计划单取消 (跌破 {plan_cancel_price:.2f})")
                    plan_entry_price = None
                    plan_cancel_price = None
                    plan_signal_bar = None
                    break
                if bars[ci].high >= plan_entry_price:
                    holding = True
                    max_high_since_entry = bars[ci].high
                    sig_idx = next(j for j, bj in enumerate(sdf_bars_idx) if bj == plan_signal_bar)
                    trades.append({
                        "symbol": symbol,
                        "signal_date": bars[plan_signal_bar].dt.strftime("%Y-%m-%d"),
                        "entry_date": bars[ci].dt.strftime("%Y-%m-%d"),
                        "entry_price": round(plan_entry_price, 2),
                        "signal": plan_signal_tag or "TTM金叉",
                        "tick_score": f"{_tick_chart_result['quality_score']:.0f}/{_tick_chart_result['close_pos']:.2f}" if _tick_chart_result else "-",
                        "tx_conf": f"{_tx_result['confidence']:.2f}/{_tx_result['conclusion']}" if _tx_result else "-",
                        "wt_scaled_v3": sdf["wt_scaled_v3"].iloc[sig_idx],
                        "wt_std_v1": sdf["wt_std_v1"].iloc[sig_idx],
                        "dmi_v1": sdf["dmi_v1"].iloc[sig_idx],
                        "dmi_v2": sdf["dmi_v2"].iloc[sig_idx],
                        "dmi_v3": sdf["dmi_v3"].iloc[sig_idx],
                        "obv_v1": sdf["obv_v1"].iloc[sig_idx],
                        "vol_v1": sdf["vol_v1"].iloc[sig_idx],
                        "entry_turnover": float(entry_turnover) if entry_turnover else 0,
                    })
                    print(f"    [{bars[ci].dt.strftime('%Y-%m-%d')}] ✅ 买入 @ {plan_entry_price:.2f} "
                          f"(换手率: {entry_turnover:.2f}%)")
                    plan_entry_price = None
                    plan_cancel_price = None
                    plan_signal_bar = None
                    break

        # ── 死叉 → 挂止损单（TTM信号）──
        ttm_v2_val = sdf["ttm_v2"].iloc[i] if "ttm_v2" in sdf.columns else "其他"
        if holding and ttm_v2_val in EXIT_V2:
            _gap = get_price_fluctuation(bar.low)
            death_cross_stop = bar.low - _gap * JUMP_MULT
            print(f"    [{date_str}] 📋 死叉止损单: {death_cross_stop:.2f}")

        # ── 死叉止损单触发 ──
        if holding and death_cross_stop is not None and bar.low <= death_cross_stop:
            trades[-1]["exit_date"] = date_str
            trades[-1]["exit_price"] = round(death_cross_stop, 2)
            trades[-1]["exit_signal"] = "TTM死叉止损"
            ret = (death_cross_stop - trades[-1]["entry_price"]) / trades[-1]["entry_price"]
            trades[-1]["return"] = f"{ret * 100:+.2f}%"
            trades[-1]["holding_days"] = (pd.Timestamp(date_str) - pd.Timestamp(trades[-1]["entry_date"])).days
            trades[-1]["max_float_pct"] = round((max_high_since_entry - trades[-1]["entry_price"]) / trades[-1]["entry_price"] * 100, 2)
            holding = False
            death_cross_stop = None
            print(f"    [{date_str}] 🔴 死叉止损 @ {trades[-1]['exit_price']:.2f} "
                  f"(收益: {trades[-1]['return']})")

        # ── 硬性 -10% 止损 ──
        if holding and trades:
            entry = trades[-1]["entry_price"]
            if entry and bar.low <= entry * 0.9:
                trades[-1]["exit_date"] = date_str
                trades[-1]["exit_price"] = round(entry * 0.9, 2)
                trades[-1]["exit_signal"] = "硬止损-10%"
                trades[-1]["return"] = "-10.00%"
                trades[-1]["holding_days"] = (pd.Timestamp(date_str) - pd.Timestamp(trades[-1]["entry_date"])).days
                holding = False
                death_cross_stop = None
                print(f"    [{date_str}] 🔴 硬止损-10% @ {trades[-1]['exit_price']:.2f}")

        # 正常流程结束也更新prev_golden_cross
        prev_gc_blocked_by_tx = False

    # ── 7. 输出报告 ──
    print(f"\n{'=' * 60}")
    print(f"  策略报告 — {symbol}")

    if not trades:
        print("  无成交记录")
        return []

    returns_pct = [
        float(t.get("return", "0%").rstrip("%"))
        for t in trades
        if "return" in t
    ]
    wins = sum(1 for r in returns_pct if r > 0)
    losses = sum(1 for r in returns_pct if r < 0)

    print(f"  成交: {len(trades)} 笔 | 胜: {wins} | 负: {losses}")
    if returns_pct:
        print(f"  总收益: {sum(returns_pct):+.1f}%  均笔: {np.mean(returns_pct):+.1f}%")
        print(f"  最大正: {max(returns_pct):+.1f}%  最大负: {min(returns_pct):+.1f}%")

    header = f"  {'信号日期':<12} {'信号':<14} {'入场日期':<12} {'入场价':>7} {'出场日期':<12} {'出场价':>7} {'收益':>8} {'tick':>10} {'tx':>14}"
    print(f"\n{header}")
    print("  " + "-" * 105)
    for t in trades:
        entry_time = t.get("entry_date", "—")[:10]
        entry_price = t.get("entry_price", 0.0)
        sig_time = t.get("signal_date", "—")[:10]
        sig = t.get("signal", "—")
        exit_time = t.get("exit_date", "持仓中")
        if exit_time and len(exit_time) > 10:
            exit_time = exit_time[:10]
        exit_price = t.get("exit_price", 0.0)
        ret = t.get("return", "持仓中")
        tick_info = t.get("tick_score", "-")
        tx_info = t.get("tx_conf", "-")
        print(f"  {sig_time:<12} {sig:<14} {entry_time:<12} {entry_price:>7.2f} {exit_time:<12} {exit_price:>7.2f} {ret:>8} {tick_info:>10} {tx_info:>14}")

    return trades


def run_and_save(symbol: str, myunit: str = "1d", strategy_type: str = "TTM_V20",
                 rule_set: str = "prod", verbose: bool = True) -> dict | None:
    """运行 ttm_v20 策略回测并将结果写入数据库 + Redis。

    底层委托给 czsc_backtrade_service.run_czsc_backtrade()，
    确保 Redis 和 DB 双写，与 FastAPI 查询端点兼容。

    参数
    ----------
    symbol : str
        股票代码，如 "000596"
    myunit : str
        K线周期（默认 "1d"，v11 目前仅支持日线）
    strategy_type : str
        策略类型（默认 "WT_V11"，DB 中区分不同策略回测结果）
    rule_set : str
        规则集标识（默认 "prod"）
    verbose : bool
        是否打印策略报告（默认 True）

    返回
    -------
    dict | None
        构建的 backtrade_rs 字典（写入 DB 的内容），失败返回 None
    """
    from app.services.czsc_backtrade_service import run_czsc_backtrade
    return run_czsc_backtrade(
        symbol=symbol,
        myunit=myunit,
        strategy_type=strategy_type,
        rule_set=rule_set,
        verbose=verbose,
    )


def main():
    """命令行入口。"""
    import argparse
    parser = argparse.ArgumentParser(description="WaveTrend 金叉突破策略 ttm_v20（openkph 大单/对倒 K线过滤）")
    parser.add_argument("symbol", nargs="?", default="000001",
                        help="股票代码，如 000596、688008（默认 000001）")
    parser.add_argument("--save", action="store_true",
                        help="同时将回测结果写入数据库")
    args = parser.parse_args()

    if args.save:
        run_and_save(args.symbol)
    else:
        run_one(args.symbol)


if __name__ == "__main__":
    main()
