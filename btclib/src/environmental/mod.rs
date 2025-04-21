// Environmental impact measurement and mitigation module
// This module implements functionality to track and reduce the blockchain's carbon footprint

pub mod types;
pub mod miner_reporting;
pub mod treasury;
pub mod emissions;
pub mod dashboard;

// Re-export commonly used types for convenience
pub use types::{EnergySource, EmissionFactor, HardwareType, Region};
pub use miner_reporting::{MinerEnvironmentalInfo, MinerReportingManager, VerificationInfo, VerificationStatus, MinerEnvironmentalReport};
pub use treasury::{EnvironmentalTreasury, EnvironmentalAssetType, EnvironmentalAssetPurchase, TreasuryError};
pub use emissions::{EmissionsCalculator, NetworkEmissions, EmissionsTimePeriod};
pub use dashboard::{EnvironmentalDashboard, DashboardMetric, MetricTimeframe}; 