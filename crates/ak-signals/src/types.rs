//! ak_rust 信号系统编译期类型
//!
//! 与 `#[signal]` 过程宏配合，通过 `inventory` 实现编译期自动注册。

use crate::{BarPoint, SignalOutput};
use std::collections::HashMap;

/// ak_rust 信号函数签名
///
/// 接收裸 K 线序列 + 参数表，返回信号输出列表。
/// 与 CZSC 的 `(&CZSC, &ParamView, &mut TaCache)` 签名不同，
/// ak_rust 保持轻量，不依赖 `czsc-core`。
pub type AkSignalFn = fn(&[BarPoint], &HashMap<String, String>) -> Vec<SignalOutput>;

/// 信号描述符（编译期元数据）
///
/// 由 `#[signal(...)]` 宏生成，通过 `inventory` 自动收集，
/// 作为运行时注册和分派的单一元数据来源。
#[derive(Clone, Copy)]
pub struct SignalDescriptor {
    /// 信号类别：`kline` 或 `trader`
    pub category: &'static str,
    /// 信号名称（如 `ak_ttm_squeeze_V260711`）
    pub name: &'static str,
    /// 参数模板（与 Python/策略配置保持一致）
    pub template: &'static str,
    /// 内部操作码名称（用于执行层分派与冲突检测）
    pub opcode: &'static str,
    /// 参数类型标识
    pub param_kind: &'static str,
    /// 信号函数指针
    pub func: AkSignalFn,
}
