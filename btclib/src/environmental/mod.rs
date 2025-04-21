// Environmental impact measurement and mitigation module
// This module implements functionality to track and reduce the blockchain's carbon footprint

pub mod emissions;
pub mod treasury;
pub mod dashboard;

// Re-export commonly used types for convenience
pub use emissions::{EmissionsTracker, Region, EmissionFactor, Emissions};
pub use treasury::{EnvironmentalTreasury, EnvironmentalAssetType, EnvironmentalAssetPurchase};
pub use dashboard::EnvironmentalDashboard; 