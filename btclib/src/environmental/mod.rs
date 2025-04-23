// Environmental impact measurement and mitigation features module

// Export components for public API
pub mod emissions;
pub mod treasury;
pub mod dashboard;
pub mod miner_reporting;

// Re-export key types for convenience
pub use emissions::{EmissionsTracker, Emissions, Region, HashRate, PoolId, PoolEnergyInfo};
pub use treasury::{EnvironmentalTreasury, EnvironmentalAssetType, EnvironmentalAssetPurchase};
pub use dashboard::{EnvironmentalDashboard, EnvironmentalMetrics, EmissionsTimePeriod}; 