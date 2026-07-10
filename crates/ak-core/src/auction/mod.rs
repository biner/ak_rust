pub mod scan;
pub mod breakout;
pub mod false_sell;
pub mod morphology;

pub use scan::{AuctionTick, AuctionScanner, ScanResult};
pub use breakout::{analyze_breakout, BreakoutResult};
pub use false_sell::{FalseSellDetector, FalseSellResult, FalseSellWeights};
pub use morphology::MORPHOLOGY_NAMES;

pub type Scanner = AuctionScanner;
