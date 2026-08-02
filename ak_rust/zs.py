"""中枢（ZS）识别 —— ak_rust 统一入口，委托 czsc 的 Rust 原生实现。

czsc 的 Rust 核心（czsc-signals）已实现 `get_zs_seq`，此处仅提供 ak_rust
的入口包装，保持与 czsc 相同的输入输出语义：

    输入: list[czsc._native.BI]  （笔对象列表）
    输出: list[czsc._native.ZS]  （中枢对象列表，含 zg/zd/zz/gg/dd/sdir/edir/sdt/edt）

用法：
    from ak_rust import get_zs_seq
    zs_objs = get_zs_seq(bi_list)
"""

from __future__ import annotations

from typing import TYPE_CHECKING

if TYPE_CHECKING:
    from typing import Any


def get_zs_seq(bi_list) -> list:
    """从笔列表识别中枢序列（入口由 ak_rust 提供，实现委托 czsc Rust 核心）。

    参数:
        bi_list: 笔对象列表（czsc._native.BI 或兼容对象，需有 direction/high/low）

    返回:
        list[czsc._native.ZS] —— 中枢对象列表，每个对象含
        zg/zd/zz/gg/dd（数值）、sdir/edir（方向枚举）、sdt/edt（时间戳）、bis（构成笔）。

    异常:
        ImportError: czsc 未安装或其 _native 缺少 get_zs_seq 时抛出，
                     由调用方决定回退策略（如使用 Python 实现）。
    """
    from czsc._native import get_zs_seq as _czsc_get_zs_seq

    return _czsc_get_zs_seq(bi_list)


__all__ = ["get_zs_seq"]
