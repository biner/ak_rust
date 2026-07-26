# ak_rust

**A 股量化分析工具箱** — 基于 Rust 实现的高性能逐笔成交、集合竞价、技术指标信号分析库。提供 Python 原生调用接口（通过 PyO3 / maturin 编译为原生扩展模块）。

---

## 项目结构

```
ak_rust/
├── Cargo.toml              # workspace 根
├── pyproject.toml          # Python 构建配置 (maturin)
├── ak_rust/                # Python 包
│   ├── __init__.py         # 导出所有 API
│   ├── _native.abi3.so     # 编译好的原生扩展
│   ├── _native/__init__.pyi  # 类型存根
│   └── signaling.py        # 信号统一调度层 (ak_rust + czsc 混合)
└── crates/
    ├── ak-core/            # 核心分析算法
    ├── ak-signals/         # 技术指标信号
    ├── ak-signal-macros/   # 信号注册过程宏
    └── ak-python/          # Python 绑定层
```

---

## 核心功能

### 1. 分时图分析 (`tick_analysis`)

对日内 tick 序列进行量价质量评分，识别大单成交、计算收盘位置、判断是否应撤单。

| 输出字段 | 说明 |
|---------|------|
| `close_pos` | 收盘价在当日区间中的百分位位置 |
| `day_chg_pct` | 当日涨跌幅 |
| `amplitude_pct` | 振幅 |
| `large_trade_count` | 大单成交笔数（≥均价×2 的成交） |
| `large_trade_vol_ratio` | 大单成交量占比 |
| `quality_score` | 量价质量综合评分（0~100） |
| `should_cancel` / `cancel_reason` | 是否建议撤单及原因 |

### 2. 逐笔成交分析 (`trans_analysis`)

对逐笔成交明细进行多维分析，包含买卖力量对比、大单/机构资金流向、分单检测、尾盘动量、尾盘竞价巨量检测、突破判定等。

| 输出字段 | 说明 |
|---------|------|
| `buy_vol_ratio` / `sell_vol_ratio` | 主动买卖成交量占比（排除中性盘） |
| `large_trade_vol_ratio` | 大单成交量占比（仅统计主动买卖盘） |
| `inst_buy_vol_ratio` / `inst_net_flow_ratio` | 机构买入占比 / 净流向 |
| `split_clusters` | 分单（拆单）检测数 |
| `matched_pairs` | 对倒交易匹配对数 |
| `last_30min_vol_ratio` | 尾盘 30 分钟成交量占比 |
| `last_30min_momentum` | 尾盘 30 分钟动量评分（分母为尾盘总量） |
| `is_breakout` / `confidence` / `conclusion` | 突破判定结果 |
| `details` | 详细描述文本 |

采用 v4 增强算法，与 v1/v2/v3 相比的核心改进：
- 买卖量分组仅统计主动买卖盘（排除中性盘干扰）
- 大单买入占比仅统计主动买卖盘（排除中性盘污染）
- 拆单/对倒惩罚阈值动态化，对大盘股更宽容
- 主力识别加入"单笔均量 > 3000 股"条件，避免散户集中成交误判
- 尾盘集合竞价巨量独立加分

### 3. 集合竞价分析 (`auction_scan` / `auction_false_sell`)

#### 竞价评分 (`auction_scan`)
对集合竞价阶段的 tick 序列进行多维度评分，输出综合得分、买卖盘对比、形态分类等。

| 输出字段 | 说明 |
|---------|------|
| `score` | 竞价综合评分（0~1） |
| `match` / `steps` | 是否达标 / 有效步数 |
| `neg_pos_alt` / `buy_rev` | 正负交替 / 买方反转次数 |
| `morphology` | 竞价形态名称 |
| `morphology_conf` | 形态置信度 |
| `morphology_scores` | 7 种形态得分数组 |

#### 七种竞价形态分类 (`morphology`)

| 编码 | 形态 | 说明 |
|------|------|------|
| B01 | 涨停试盘型 | 早段触板→回落 |
| A01 | 阶梯攀升型 | 逐级上移+放量 |
| A02 | 钓鱼线型 | 9:20 前高位→骤降 |
| B02 | 跌停撬板型 | 早段触跌停→回升 |
| YZ01 | 一字放量型 | 价格不动+大量成交 |
| YZ02 | 无量一字型 | 价格不动+地量 |
| ZD01 | 震荡分歧型 | 反复波动+方向不明 |

#### 虚假卖出检测 (`auction_false_sell`)
识别集合竞价阶段疑似虚假卖单（压单后撤单）行为，输出评分、负压比率、尾段峰值、回弹力度等。

### 4. 突破时刻分析 (`trans_breakout`)

在逐笔成交中检测关键价位突破行为，分析突破时的成交量、买卖方向、大单构成、价格维持度。

| 输出字段 | 说明 |
|---------|------|
| `breakout_n` / `breakout_vol_ratio` | 突破笔数 / 成交量占比 |
| `breakout_buy_ratio` | 突破时买方占比 |
| `breakout_large_buy/sell` | 突破时大单买卖笔数 |
| `price_sustain_ratio` | 价格维持度 |
| `is_strong` / `classification` | 强弱判定 / 分类标签 |
| `confidence` | 置信度 |

### 5. 技术指标信号 (`ak-signals`)

#### WaveTrend 通道 (`compute_wavetrend` / `wt_signal`)

czsc 原版算法：HLC/3 → EMA → CI → WT1 → WT2，输出三维信号：

- **v1 (zone)**: 超涨 / 多头极强 / 多头强势 / 多头 / 弱多 / 弱空 / 空头 / 空头强势 / 空头极强 / 超跌
- **v2 (direction)**: 增大 / 缩小 / 平稳
- **v3 (cross)**: 金叉 / 死叉 / 多头 / 空头

#### WaveTrend Zone (`ak_wavetrend_zone_V260724`)

六区域定位 + 长期趋势状态机 + 多空方向，综合评分：

- **v1**: 极度超卖 / 超卖 / 中性0轴下 / 中性0轴上 / 超买 / 极度超买
- **v2**: 长期看多 / 长期看空 / 其他
- **v3**: 多头 / 空头

#### DMI 状态 (`ak_dmi_state_V260714`)

基于 TA-Lib ±DI / ADX 的三维信号：

- **v1 (ADX)**: 盘整 / 酝酿 / 趋势 / 强趋势
- **v2 (±DI 差值)**: 多头强势 / 多头 / 缠绕 / 空头 / 空头强势
- **v3 (ADX 斜率)**: 上升 / 下降 / 平稳

#### IC 云图 (`ak_ic_state_V260718`)

Ichimoku Cloud 四线（CL/DL/A/B）位置/云型/线态三维信号：

- **v1 (价格位置)**: 云上 / 云下 / 云内 / 突破上 / 跌破下 / 测云顶 / 测云底 / 贯穿云
- **v2 (云型)**: 上升云 / 下降云
- **v3 (线态)**: 双强 / 双高 / 高影 / 线内 / 低影 / 双低 / 双弱 / CL金叉 / CL死叉

#### TTM Squeeze (`ak_ttm_squeeze_V260711`)

Bollinger Bands ± Keltner Channel 挤压/震动三维信号：

- **v1 (方向)**: 多头 / 空头
- **v2 (交叉事件)**: 金叉 / 死叉 / 二次上升 / 二次下降 / 正值翻转 / 负值翻转 / ...（共 12 种）
- **v3 (挤压状态)**: 挤压 / 震动 / 转挤压 / 转震动

### 6. 基础技术分析算子 (`ta`)

- `ema(values, n)` — 指数移动平均
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

### 集合竞价分析

```python
from ak_rust import auction_scan, auction_false_sell, auction_morphology_names

# 查看支持的形态分类
print(auction_morphology_names())

# 竞价评分
ticks = [
    {"time": "09:15:00", "price": 10.0, "matched": 100, "unmatched": 50},
    {"time": "09:20:00", "price": 10.2, "matched": 200, "unmatched": -30},
]
result = auction_scan(ticks, threshold=0.5, pre_close=9.80)

# 虚假卖出检测
result = auction_false_sell(ticks, threshold=0.5)
```

### WaveTrend 信号

```python
from ak_rust import compute_wavetrend, wt_signal, wt_classify_zone

prices = [100.0, 101.0, 102.0, ...]
wt = compute_wavetrend(prices, n1=10, n2=21, n3=4)
signal = wt_signal(prices, di=1, n1=10, n2=21, n3=4)
# signal = {"zone": "多头", "direction": "增大", "cross": "金叉"}
```

### 信号统一调度

```python
from ak_rust.signaling import generate_all_signals
from czsc import RawBar

bars = [RawBar(...), ...]
signals_config = [
    {"name": "ak_wavetrend_V260625", "freq": "日线",
     "params": {"n1": "50", "n2": "105", "n3": "20"}},
    {"name": "ak_dmi_state_V260714", "freq": "日线", "n2": 14},
]
sdf = generate_all_signals(bars, signals_config)
```

---

## 内置信号一览

| 信号名称 | 版本 | 说明 | v1 | v2 | v3 |
|---------|------|------|----|----|----|
| `ak_wavetrend_V260625` | V260625 | WaveTrend 多空/方向/交叉 | 多空区域 | 趋势方向 | 金叉死叉 |
| `ak_wavetrend_zone_V260724` | V250724 | WaveTrend 区域/长期趋势/方向 | 6级区位 | 长期趋势 | 多空方向 |
| `ak_dmi_state_V260714` | V250714 | DMI 趋势/方向/动量 | ADX强度 | ±DI强弱 | ADX斜率 |
| `ak_ic_state_V260718` | V250718 | IC 云图位置/云型/线态 | 8种位置 | 云型 | 8种线态 |
| `ak_ttm_squeeze_V260711` | V250711 | TTM 方向/交叉/挤压 | 多空方向 | 12种交叉 | 挤压状态 |

---

## 构建与运行

```bash
# Rust 测试
cargo test

# Python 构建
uv build

# 安装到当前环境
uv pip install .
```

---

## 技术栈

- **Rust** — 核心算法，零成本抽象
- **PyO3** — Rust ↔ Python 互操作
- **maturin** — Python 包构建
- **TA-Lib (talib-rs)** — DMI / IC 中的技术指标计算
- **chrono** — 时间序列处理
- **czsc** — Python 侧信号协同（可选依赖）

---

## 许可

MIT
