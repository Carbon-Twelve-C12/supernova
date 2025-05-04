// Environmental impact measurement and mitigation features module

// Export components for public API
pub mod emissions;
pub mod treasury;
pub mod dashboard;
pub mod miner_reporting;
pub mod governance;
pub mod transparency;
pub mod alerting;

// Re-export key types for convenience
pub use emissions::{EmissionsTracker, Emissions, Region, HashRate, PoolId, PoolEnergyInfo};
pub use treasury::{EnvironmentalTreasury, EnvironmentalAssetType, EnvironmentalAssetPurchase};
pub use dashboard::{EnvironmentalDashboard, EnvironmentalMetrics, EmissionsTimePeriod};
pub use governance::{EnvironmentalGovernance, GovernanceConfig, EnvironmentalProposal, ProposalStatus};
pub use transparency::{TransparencyDashboard, TransparencyReport, TransparencyLevel, EnvironmentalSummary};
pub use alerting::{EnvironmentalAlertingSystem, AlertingConfig, AlertRule, Alert, AlertSeverity, MetricType}; 