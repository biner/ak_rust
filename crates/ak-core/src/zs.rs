//! 中枢（ZS）识别 —— 独立实现，算法移植自 czsc_signals::utils::cxt::get_zs_seq。
//!
//! 为保持 ak-core 零外部依赖，此处将 czsc 的核心算法（纯数值逻辑）
//! 复制为独立实现，输入输出使用本 crate 定义的轻量结构，避免依赖
//! czsc-core 的 BI/ZS 类型与 python feature。

use chrono::{DateTime, Utc};

/// 笔方向
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BiDirection {
    Up,
    Down,
}

/// 轻量笔结构（对齐 czsc BI 的核心字段）
#[derive(Debug, Clone)]
pub struct Bi {
    pub direction: BiDirection,
    pub high: f64,
    pub low: f64,
    pub sdt: DateTime<Utc>,
    pub edt: DateTime<Utc>,
}

impl Bi {
    pub fn start_dt(&self) -> DateTime<Utc> {
        self.sdt
    }

    pub fn end_dt(&self) -> DateTime<Utc> {
        self.edt
    }
}

/// 中枢（对齐 czsc ZS 的字段）
#[derive(Debug, Clone)]
pub struct Zs {
    pub sdt: DateTime<Utc>,
    pub edt: DateTime<Utc>,
    pub sdir: BiDirection,
    pub edir: BiDirection,
    pub zg: f64,
    pub zd: f64,
    pub zz: f64,
    pub gg: f64,
    pub dd: f64,
}

impl Zs {
    /// 从构成中枢的笔序列构建（移植自 czsc-core ZS::new）
    fn new(bis: &[Bi]) -> Self {
        let sdt = bis.first().unwrap().start_dt();
        let edt = bis.last().unwrap().end_dt();
        let sdir = bis.first().unwrap().direction;
        let edir = bis.last().unwrap().direction;
        let zg = bis
            .iter()
            .take(3)
            .map(|x| x.high)
            .fold(f64::INFINITY, f64::min);
        let zd = bis
            .iter()
            .take(3)
            .map(|x| x.low)
            .fold(f64::NEG_INFINITY, f64::max);
        let gg = bis
            .iter()
            .map(|x| x.high)
            .fold(f64::NEG_INFINITY, f64::max);
        let dd = bis
            .iter()
            .map(|x| x.low)
            .fold(f64::INFINITY, f64::min);
        let zz = zd + (zg - zd) * 0.5;
        Zs { sdt, edt, sdir, edir, zg, zd, zz, gg, dd }
    }
}

/// 从笔列表识别中枢序列。
///
/// 算法移植自 `czsc_signals::utils::cxt::get_zs_seq`：
/// 遍历笔，维护当前中枢；方向 Up 且高点低于中枢下沿，或方向 Down 且
/// 低点高于中枢上沿时，视为中枢破坏，新开一个中枢；否则并入当前中枢。
pub fn get_zs_seq(bis: &[Bi]) -> Vec<Zs> {
    let mut zs_list: Vec<Zs> = Vec::new();
    if bis.is_empty() {
        return zs_list;
    }

    let mut cur_bis: Vec<Bi> = Vec::new();
    for bi in bis {
        if cur_bis.is_empty() {
            cur_bis.push(bi.clone());
            continue;
        }

        let cur_zs = Zs::new(&cur_bis);
        let break_out = (bi.direction == BiDirection::Up && bi.high < cur_zs.zd)
            || (bi.direction == BiDirection::Down && bi.low > cur_zs.zg);
        if break_out {
            zs_list.push(Zs::new(&cur_bis));
            cur_bis.clear();
        }
        cur_bis.push(bi.clone());
    }
    if !cur_bis.is_empty() {
        zs_list.push(Zs::new(&cur_bis));
    }
    zs_list
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn bi(direction: BiDirection, high: f64, low: f64, day: u32) -> Bi {
        Bi {
            direction,
            high,
            low,
            sdt: Utc.with_ymd_and_hms(2026, 1, day, 9, 30, 0).unwrap(),
            edt: Utc.with_ymd_and_hms(2026, 1, day, 10, 0, 0).unwrap(),
        }
    }

    #[test]
    fn test_get_zs_seq_empty() {
        assert_eq!(get_zs_seq(&[]).len(), 0);
    }

    #[test]
    fn test_get_zs_seq_single_zs() {
        // 三笔连续重叠 → 一个中枢
        let bis = vec![
            bi(BiDirection::Down, 10.0, 9.0, 1),
            bi(BiDirection::Up, 11.0, 9.5, 2),
            bi(BiDirection::Down, 10.5, 9.2, 3),
        ];
        let zs = get_zs_seq(&bis);
        assert_eq!(zs.len(), 1);
        let z = &zs[0];
        assert_eq!(z.sdir, BiDirection::Down);
        assert_eq!(z.edir, BiDirection::Down);
        // zg = 前3笔 high 的 min = min(10.0, 11.0, 10.5) = 10.0
        assert!((z.zg - 10.0).abs() < 1e-9);
        // zd = 前3笔 low 的 max = max(9.0, 9.5, 9.2) = 9.5
        assert!((z.zd - 9.5).abs() < 1e-9);
    }

    #[test]
    fn test_get_zs_seq_breakout() {
        // 第一组三笔 + 大幅跳空导致破坏 → 两个中枢
        let bis = vec![
            bi(BiDirection::Down, 10.0, 9.0, 1),
            bi(BiDirection::Up, 11.0, 9.5, 2),
            bi(BiDirection::Down, 10.5, 9.2, 3),
            // Up 且 high < 前中枢 zd(9.5) → 破坏
            bi(BiDirection::Up, 8.0, 7.0, 4),
            bi(BiDirection::Down, 8.5, 7.2, 5),
            bi(BiDirection::Up, 9.0, 7.5, 6),
        ];
        let zs = get_zs_seq(&bis);
        assert_eq!(zs.len(), 2);
        assert!((zs[1].zd - 7.5).abs() < 1e-9);
    }
}
