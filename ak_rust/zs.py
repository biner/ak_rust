"""中枢（ZS）识别 —— ak_rust 统一入口（Rust 原生实现）。

算法移植自 czsc 的 get_zs_seq（纯数值逻辑，无外部依赖），
由 ak-core 实现、ak-python 暴露为 ak_rust._native.get_zs_seq。

输入输出均为纯 dict 列表（不依赖 czsc 类型）：

    输入: list[dict]，每项含 direction("Up"/"Down")、high、low、sdt、edt
    输出: list[dict]，每项含 zg/zd/zz/gg/dd、sdir/edir、sdt/edt

用法：
    from ak_rust import get_zs_seq
    zs_objs = get_zs_seq(bi_list)
"""

from __future__ import annotations

from ._native import get_zs_seq

__all__ = ["get_zs_seq"]
