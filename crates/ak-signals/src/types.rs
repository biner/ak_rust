use std::collections::HashMap;
use crate::signals::{BarPoint, SignalOutput};

/// 信号元数据
pub struct AkSignalMeta {
    pub name: &'static str,
    pub template: &'static str,
    pub func: AkSignalFnRef,
}

pub enum AkSignalFnRef {
    Closure(Box<dyn Fn(&[BarPoint], &HashMap<String, String>) -> Vec<SignalOutput> + Send>),
}

inventory::collect!(AkSignalMeta);
