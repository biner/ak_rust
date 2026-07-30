# ak_rust

**A 股量化分析工具箱** — 基于 Rust 实现的高性能分时图、逐笔成交、集合竞价分析及技术指标信号库。
通过 PyO3 / maturin 编译为 Python 原生扩展，返回类型安全的 dataclass，所有评分统一使用百分制 `[0, 100]`。

---

## 项目结构

```
ak_rust/
├── Cargo.toml                  # Rust workspace 根
├── pyproject.toml              # Python 构建配置 (maturin)
├── LICENSE                     # Apache-2.0
│
├── ak_rust/                    # Python 包
│   ├── __init__.py             # 类型化 API（返回 dataclass）
│   ├── models.py               # 6 个 dataclass 类型定义
│   ├── signaling.py            # 信号统一调度层 (ak_rust + czsc 混合)
│   ├── py.typed                # PEP 561 类型标记
│   ├── _native.abi3.so         # 编译好的原生扩展
│   └── _native/
│       └── __init__.pyi        # 类型存根 (IDE 补全)
│
└── crates/
    ├── ak-core/                # 核心分析算法
    │   ├── src/
    │   │   ├── lib.rs          # 模块导出 + Analyzer trait re-export
    │   │   ├── ta.rs           # 基础技术分析算子 (EMA/SMA)
    │   │   ├── tick/           # 分时图量价分析（目录模块）
    │   │   │   ├── mod.rs
    │   │   │   ├── types.rs    # TickPoint, TickChartResult
    │   │   │   ├── analyzer.rs # TickAnalyzer trait + DefaultTickAnalyzer
    │   │   │   └── core.rs     # analyze_tick_chart, calc_close_pos
    │   │   ├── transaction/    # 逐笔成交分析（目录模块）
    │   │   │   ├── mod.rs
    │   │   │   ├── types.rs    # TransactionPoint, BreakoutResult, ScreeningMetrics
    │   │   │   ├── analyzer.rs # TxAnalyzer trait + DefaultTxAnalyzer
    │   │   │   ├── breakout.rs # BreakoutAnalyzer trait + DefaultBreakoutAnalyzer
    │   │   │   ├── core.rs     # analyze_transactions (v4), analyze_breakout
    │   │   │   └── screening.rs# compute_screening_metrics
    │   │   └── auction/        # 集合竞价分析
    │   │       ├── mod.rs
    │   │       ├── scan.rs     # 竞价扫描评分 + AuctionAnalyzer trait
    │   │       ├── false_sell.rs# 虚假卖出检测 + FalseSellAnalyzer trait
    │   │       └── morphology.rs# 七种竞价形态分类
    │   └── Cargo.toml
    │
    ├── ak-signals/             # 技术指标信号库
    │   ├── src/
    │   │   ├── lib.rs          # 模块导出 + inventory 注册
    │   │   ├── types.rs        # 信号描述符类型
    │   │   ├── signals.rs      # 信号调度框架 + 8 个 #[signal] 函数
    │   │   ├── wavetrend.rs    # WaveTrend 通道信号
    │   │   ├── dmi.rs          # DMI 趋势状态信号
    │   │   ├── ic.rs           # Ichimoku Cloud 云图信号
    │   │   ├── ttm.rs          # TTM Squeeze 挤压信号
    │   │   ├── osc.rs          # OSC 动量振荡信号
    │   │   └── vol.rs          # VOL 量比信号
    │   └── Cargo.toml
    │
    ├── ak-signal-macros/       # 信号注册过程宏
    │   └── src/lib.rs          # #[signal] 属性宏
    │
    └── ak-python/              # Python 绑定层 (PyO3)
        └── src/
            ├── lib.rs          # PyModule 入口
            ├── tick/mod.rs     # tick_analysis 绑定
            ├── transaction/
            │   ├── mod.rs
            │   ├── analyzer.rs # trans_analysis 绑定
            │   ├── breakout.rs # trans_breakout 绑定
            │   └── screening.rs# screening_metrics 绑定
            ├── auction/
            │   ├── mod.rs
            │   ├── scan.rs     # auction_scan 绑定
            │   └── false_sell.rs# auction_false_sell 绑定
            ├── dispatch.rs     # generate_ak_signals, ak_signal_list
            └── generate_all.rs # 混合信号总调度 (ak_rust + czsc)
```

---

## 核心功能

> 所有评分统一为百分制 `int [0, 100]`，与 ak-signals 的 `score: i32` 对齐，便于跨模块比较。

### 1. 分时图分析 (`tick_analysis`)

对日内 tick 序列进行量价质量评分，识别大单成交、计算收盘位置、判断是否应撤单。

| 输出字段 | 说明 |
|---------|------|
| `close_pos` | 收盘价在当日区间中的百分位位置 |
| `day_chg_pct` | 当日涨跌幅 |
| `amplitude_pct` | 振幅 |
| `total_vol` | 全天总成交量 |
| `large_trade_count` | 大单成交笔数（≥均价×2 的成交） |
| `large_trade_vol_ratio` | 大单成交量占比 |
| `quality_score` | 量价质量综合评分（0~100） |
| `should_cancel` / `cancel_reason` | 是否建议撤单及原因 |

### 2. 逐笔成交分析 (`trans_analysis`)

对逐笔成交明细进行多维分析，采用 **v4 增强算法**，包含买卖力量对比、大单/机构资金流向、分单检测、尾盘动量、尾盘竞价巨量检测、突破判定等。

| 输出字段 | 说明 |
|---------|------|
| `buy_vol_ratio` / `sell_vol_ratio` / `neutral_vol_ratio` | 主动买卖/中性成交量占比 |
| `large_trade_count` / `large_trade_vol_ratio` | 大单笔数/成交量占比 |
| `large_buy_count` / `large_sell_count` / `large_buy_vol_ratio` | 大单买卖明细 |
| `inst_buy_vol_ratio` / `inst_buy_count` / `inst_sell_count` / `inst_net_flow_ratio` | 机构资金流向 |
| `split_clusters` | 分单（拆单）检测数 |
| `matched_pairs` | 对倒交易匹配对数 |
| `price_position` / `day_chg_pct` | 价格位置 / 涨跌幅 |
| `last_30min_vol_ratio` | 尾盘 30 分钟成交量占比 |
| `last_30min_buy_sell_ratio` | 尾盘买卖比 |
| `last_30min_momentum` | 尾盘 30 分钟动量评分 |
| `is_breakout` / `confidence` / `conclusion` / `details` | 突破判定结果及详情 |

**v4 增强算法核心改进：**
- 买卖量分组仅统计主动买卖盘（排除中性盘干扰）
- 大单买入占比仅统计主动买卖盘（排除中性盘污染）
- 拆单/对倒惩罚阈值动态化，对大盘股更宽容
- 主力识别加入"单笔均量 > 3000 股"条件，避免散户集中成交误判
- 尾盘集合竞价巨量独立加分

### 3. 突破时刻分析 (`trans_breakout`)

在逐笔成交中检测关键价位突破行为，分析突破时的成交量、买卖方向、大单构成、价格维持度。

| 输出字段 | 说明 |
|---------|------|
| `total_n` / `breakout_n` | 总成交笔数 / 突破笔数 |
| `breakout_vol_ratio` | 突破成交量占比 |
| `breakout_buy_ratio` | 突破时买方占比 |
| `breakout_large_buy` / `breakout_large_sell` | 突破时大单买卖笔数 |
| `price_sustain_ratio` | 价格维持度（突破后价格保持在阈值之上的比例） |
| `is_strong` / `classification` | 强弱判定 / 分类标签 |
| `confidence` / `details` | 置信度 / 详细描述 |

### 4. 集合竞价分析

#### 4a. 竞价评分 (`auction_scan`)

对集合竞价阶段（09:15–09:25）的 tick 序列进行多维度评分，输出综合得分、买卖盘对比、形态分类等。

| 输出字段 | 说明 |
|---------|------|
| `score` | 竞价综合评分（0~1） |
| `match` / `steps` | 是否达标 / 有效步数 |
| `neg_pos_alt` | 正负交替次数 |
| `buy_rev` | 买方反转次数 |
| `vr` | 量比 |
| `end_p` | 尾段压力 |
| `oh` | 开盘跳空幅度 |
| `morphology` | 竞价形态名称（中文） |
| `morphology_conf` | 形态置信度 |
| `morphology_scores` | 7 种形态得分数组 |
| `morphology_tag` / `morphology_group` | 形态编码/分组 |

#### 4b. 七种竞价形态分类

分类算法基于 tick 序列的价格轨迹、量能变化、买卖盘对比进行无监督聚类。

| 编码 | 形名 | 说明 | 分组 |
|------|------|------|------|
| B01 | 涨停试盘型 | 早段触板→回落 | B |
| A01 | 阶梯攀升型 | 逐级上移+放量 | A |
| A02 | 钓鱼线型 | 9:20 前高位→骤降 | A |
| B02 | 跌停撬板型 | 早段触跌停→回升 | B |
| YZ01 | 一字放量型 | 价格不动+大量成交堆积 | YZ |
| YZ02 | 无量一字型 | 价格不动+地量 | YZ |
| ZD01 | 震荡分歧型 | 反复波动+方向不明 | ZD |

#### 4c. 虚假卖出检测 (`auction_false_sell`)

识别集合竞价阶段疑似虚假卖单（压单后撤单、诱空）行为。

| 输出字段 | 说明 |
|---------|------|
| `score` | 虚假卖出评分（0~1） |
| `match` | 是否触发阈值 |
| `neg_ratio` | 负压（委托卖-委托买）比率 |
| `price_up` | 价格上升段检测 |
| `vr` | 量比 |
| `last_um` | 尾段未匹配量方向 |
| `late_peak` | 尾段峰值压力 |
| `recovery_score` | 回弹力度评分 |
| `big_order_ratio` | 大单占比 |
| `price_change_pct` | 竞价期间价格变化百分比 |

### 5. 筛选指标 (`screening_metrics`)

对逐笔成交数据进行轻量级统计，提供分时段成交占比、大单特大单占比、加权均价等指标。

| 输出字段 | 说明 |
|---------|------|
| `first30_vol_pct` | 早盘前30分钟成交量占比 |
| `midday_vol_pct` | 午间成交量占比 |
| `afternoon_vol_pct` | 下午盘成交量占比 |
| `last30_vol_pct` / `last5_vol_pct` | 尾盘30分/5分钟占比 |
| `large_pct` / `xl_pct` | 大单/特大单成交笔数占比 |
| `large_amount_pct` / `xl_amount_pct` | 大单/特大单成交额占比 |
| `avg_trade_vol` | 平均每笔成交量 |
| `vwap` | 成交量加权均价 (VWAP) |
| `avg_shares_per_trade` | 单笔均量（股） |
| `single_trade_vol_pct` | 单笔最大占比 |

### 6. 技术指标信号 (`ak-signals`)

信号通过 `#[signal]` 过程宏 + `inventory` 实现编译期自动注册，运行时通过注册表分派。

#### 6a. WaveTrend 通道 (`compute_wavetrend` / `wt_signal` / `ak_wavetrend_V260625`)

CZSC 原版算法：HLC/3 → EMA → CI → WT1 → WT2，输出三维信号：

- **v1 (zone)**: 极度超买 / 超买 / 偏多 / 弱多 / 弱空 / 偏空 / 超卖 / 极度超卖
- **v2 (direction)**: 增大 / 缩小 / 平稳
- **v3 (cross)**: 金叉 / 死叉 / 多头 / 空头

#### 6b. WaveTrend Zone (`ak_wavetrend_zone_V260724`)

六区域定位 + 长期趋势状态机 + 多空方向，综合评分：

- **v1**: 极度超卖 / 超卖 / 中性0轴下 / 中性0轴上 / 超买 / 极度超买
- **v2**: 长期看多 / 长期看空 / 其他
- **v3**: 多头 / 空头
- **score**: 0~95 综合评分表

#### 6c. DMI 状态 (`ak_dmi_state_V260714`)

基于 TA-Lib ±DI / ADX / ADXR 的三维信号：

- **v1 (ADX 强度)**: 盘整 / 酝酿 / 趋势 / 强趋势
- **v2 (±DI 差值)**: 多头强势 / 多头 / 缠绕 / 空头 / 空头强势
- **v3 (ADX 斜率)**: 上升 / 下降 / 平稳

#### 6d. IC 云图 (`ak_ic_state_V260718`)

Ichimoku Cloud 四线（CL/DL/A/B）位置/云型/线态三维信号：

- **v1 (价格位置: 8种)**: 云上 / 云下 / 云内 / 突破上 / 跌破下 / 测云顶 / 测云底 / 贯穿云
- **v2 (云型)**: 上升云 / 下降云
- **v3 (线态: 8种+交叉)**: 双强 / 双高 / 高影 / 线内 / 低影 / 双低 / 双弱 / CL金叉 / CL死叉

#### 6e. TTM Squeeze (`ak_ttm_squeeze_V260711`)

Bollinger Bands ± Keltner Channel 挤压/震动三维信号：

- **v1 (方向)**: 多头 / 空头
- **v2 (交叉事件: 12种)**: 金叉 / 死叉 / 二次上升 / 二次下降 / 正值翻转 / 负值翻转 / ...
- **v3 (挤压状态)**: 挤压 / 震动 / 转挤压 / 转震动

#### 6f. OSC 动量振荡 (`ak_osc_state_V260720` / `ak_osc_state_V260721`)

基于价格偏离移动平均的动量信号，两个版本：

**V260720 (绝对值版):**
- **v1 (方向)**: 上 / 下 / 零
- **v2 (温度)**: 极冷 / 偏低 / 温和 / 偏高 / 过热
- **v3 (加速度)**: 上升 / 下降 / 平稳

**V260721 (百分比版):**
- **v1 (方向)**: 上 / 下 / 零
- **v2 (温度)**: 极度冷 / 偏冷 / 适中 / 偏热 / 极度热
- **v3 (加速度)**: 上升 / 下降 / 平稳

#### 6g. VOL 量比 (`ak_vol_ratio_V260721`)

- **v1**: 放量 / 缩量 / 平量（阈值 ±20%）

### 7. 基础技术分析算子 (`ta`)

- `ema(values, n)` — 指数移动平均（span = 2/(n+1)）
- `sma(values, n)` — 简单移动平均

---

## Python 快速开始

### 安装

```bash
pip install ak-rust
```

或从源码构建：

```bash
uv build
uv pip install .
```

### 分时图分析

```python
from ak_rust import tick_analysis

ticks = [
    {"time": "09:30:00", "price": 10.0, "avg": 9.98, "vol": 100, "momentum": 0.5},
    {"time": "09:31:00", "price": 10.1, "avg": 9.99, "vol": 200, "momentum": 0.8},
]
result = tick_analysis(ticks, pre_close=9.80, low_price=9.70)
```

### 逐笔成交分析

```python
from ak_rust import trans_analysis

txns = [
    {"time": "09:30:00", "price": 10.0, "vol": 1000, "trade_count": 5, "bs_flag": 1},
    {"time": "09:31:00", "price": 10.1, "vol": 2000, "trade_count": 8, "bs_flag": 2},
]
result = trans_analysis(txns, pre_close=9.80, high=10.2, low=9.90)
```

### 突破分析

```python
from ak_rust import trans_breakout

result = trans_breakout(txns, breakout_price=10.05)
```

### 筛选指标

```python
from ak_rust import screening_metrics

txns = [
    {"time": "09:30:00", "price": 10.0, "vol": 1000, "trade_count": 5},
    ...
]
metrics = screening_metrics(txns)
# 返回分时段成交占比、大单/特大单占比、VWAP 等
```

### 集合竞价分析

```python
from ak_rust import auction_scan, auction_false_sell, auction_morphology_names

# 查看支持的形态分类
print(auction_morphology_names())

# 竞价扫描评分
ticks = [
    {"time": "09:15:00", "price": 10.0, "matched": 100, "unmatched": 50},
    {"time": "09:20:00", "price": 10.2, "matched": 200, "unmatched": -30},
]
result = auction_scan(ticks, threshold=50, pre_close=9.80)

# 虚假卖出检测
result = auction_false_sell(ticks, threshold=50)
```

### 信号统一调度

混合调度 ak_rust 原生信号与 CZSC 第三方信号，自动路由：

```python
from ak_rust.signaling import generate_all_signals
from czsc import RawBar

bars = [RawBar(...), ...]
signals_config = [
    {"name": "ak_wavetrend_V260625", "freq": "日线",
     "params": {"n1": "50", "n2": "105", "n3": "20"}},
    {"name": "ak_dmi_state_V260714", "freq": "日线", "n1": 14, "n2": 6},
    {"name": "ak_ttm_squeeze_V260711", "freq": "日线",
     "params": {"n1": "20", "n2": "1.5", "n3": "20", "n4": "2.0"}},
    # CZSC 信号自动委托给 czsc.generate_czsc_signals
    {"name": "obvm_line_V230610", "freq": "日线", "n": 6, "m": 14},
]
sdf = generate_all_signals(bars, signals_config)
# 返回 DataFrame，ak_rust 信号列自动拼接（v1_v2_v3_score 格式）
```

也可直接调用原生底层接口：

```python
from ak_rust import generate_ak_signals, ak_signal_list

print(ak_signal_list())  # 列出所有已注册信号

results = generate_ak_signals(signals_config, bars)
```

### 类型化的返回结果

所有分析函数返回类型安全的 dataclass，字段名与 Rust 侧一致，IDE 自动补全：

```python
from ak_rust import tick_analysis, auction_scan

result = tick_analysis(ticks, pre_close=9.80)
# result: TickChartResult | None
if result:
    print(f"评分: {result.quality_score}, 撤单: {result.should_cancel}")

scan = auction_scan(ticks, threshold=50)
# scan: ScanResult | None
if scan:
    print(f"竞价分: {scan.score}, 形态: {scan.morphology} (置信度 {scan.morphology_conf})")
```

---

## 内置信号一览

| 信号名称 | 版本 | 说明 | v1 维度 | v2 维度 | v3 维度 |
|---------|------|------|---------|---------|---------|
| `ak_wavetrend_V260625` | V260625 | WaveTrend 多空/方向/交叉 | 8级区位 | 趋势方向 | 金叉死叉 |
| `ak_wavetrend_zone_V260724` | V260724 | WaveTrend 区域/长期趋势/方向 | 6级区位 | 长期趋势 | 多空方向 |
| `ak_dmi_state_V260714` | V260714 | DMI 趋势/多空/动量 | ADX强度 | ±DI强弱 | ADX斜率 |
| `ak_ic_state_V260718` | V260718 | IC 云图位置/云型/线态 | 8种位置 | 云型 | 8种线态 |
| `ak_ttm_squeeze_V260711` | V260711 | TTM 方向/交叉/挤压 | 多空方向 | 12种交叉 | 挤压状态 |
| `ak_osc_state_V260720` | V260720 | OSC 动量/温度/加速度 | 方向 | 5级温度 | 加速度 |
| `ak_osc_state_V260721` | V260721 | OSC 动量/百分比温度/加速度 | 方向 | 5级温度 | 加速度 |
| `ak_vol_ratio_V260721` | V260721 | VOL 量比 | 放量/缩量/平量 | — | — |

---

## Cargo Workspace 详解

| Crate | 路径 | 职责 | 依赖 |
|-------|------|------|------|
| **ak-core** | `crates/ak-core` | 核心算法：分时图、逐笔成交、集竞扫描、形态分类、虚假卖出、突破检测 | chrono |
| **ak-signals** | `crates/ak-signals` | 技术指标信号：WaveTrend / DMI / IC / TTM / OSC / VOL 信号计算与调度 | ak-core, ak-signal-macros, czsc-ta, talib-rs, inventory |
| **ak-signal-macros** | `crates/ak-signal-macros` | 过程宏：`#[signal]` 编译期信号注册 | proc-macro2, syn, quote |
| **ak-python** | `crates/ak-python` | PyO3 绑定层，将 Rust 函数暴露为 Python `_native` 模块 | ak-core, ak-signals, pyo3 |

### 信号注册机制

1. 用 `#[signal(category, name, template, opcode, param_kind)]` 标注信号函数
2. 过程宏自动生成 `SignalDescriptor` 常量 + `inventory::submit!` 注册
3. 运行时 `SIGNAL_REGISTRY` (LazyLock<HashMap>) 通过 `inventory::iter` 收集
4. `dispatch_signals` / `dispatch_one` 按名称分派
5. Python 绑定通过 `generate_ak_signals` / `generate_all_signals` 接入

---

## 构建与运行

```bash
# Rust 测试（全部 crate）
cargo test

# 仅测试核心算法
cargo test -p ak-core

# 仅测试信号库
cargo test -p ak-signals

# Python 构建
uv build

# 安装到当前环境
uv pip install .

# 查看已注册信号
python -c "from ak_rust import ak_signal_list; print(ak_signal_list())"
```

---

## 依赖关系

```
                  ┌──────────────┐
                  │  ak-signal-  │
                  │  macros      │
                  └──────┬───────┘
                         │ proc-macro
                  ┌──────▼───────┐
                  │  ak-signals  │──── czsc-ta, talib-rs
                  └──────┬───────┘
                         │
              ┌──────────▼──────────┐
              │      ak-core        │── chrono
              └──────────┬──────────┘
                         │
              ┌──────────▼──────────┐
              │     ak-python       │── pyo3
              │  (cdylib → .so)     │
              └─────────────────────┘
```

---

## 技术栈

- **Rust** — 核心算法，零成本抽象，类型安全
- **PyO3 (0.28)** — Rust ↔ Python 互操作（abi3-py310）
- **maturin** — Python 包构建与发布
- **TA-Lib (talib-rs 0.1.2)** — DMI / IC 技术指标计算
- **czsc-ta** — CZSC 兼容的纯 Rust TA 算子（SMA）
- **chrono (0.4)** — 时间序列处理
- **inventory (0.3)** — 编译期信号注册表收集
- **czsc** — Python 侧信号协同调度（可选依赖）
- **pandas** — 信号输出 DataFrame（可选依赖）

---

## 许可

Apache-2.0
