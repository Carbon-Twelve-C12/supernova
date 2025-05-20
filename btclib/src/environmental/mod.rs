// Environmental subsystem for SuperNova blockchain
// Provides features for environmental monitoring, carbon offsetting, and ESG compliance

// Re-export all modules
pub mod api;
pub mod alerting;
pub mod dashboard;
pub mod emissions;
pub mod governance;
pub mod miner_reporting;
pub mod transparency;
pub mod treasury;
pub mod types;
pub mod verification;

// Re-export commonly used types with module-specific prefixes to avoid conflicts
pub use types::{Region as TypesRegion, EnergySource, EmissionFactor, EmissionsFactorType, HardwareType as TypesHardwareType};
pub use miner_reporting::{MinerEnvironmentalInfo, MinerReportingManager, MinerVerificationStatus, TypesHardwareType as MinerHardwareType};
pub use emissions::{EmissionsTracker, EmissionCalculator, Region as EmissionsRegion};
pub use treasury::{EnvironmentalTreasury, EnvironmentalAssetType, EnvironmentalAssetPurchase, VerificationStatus};
pub use dashboard::{EnvironmentalDashboard, EnvironmentalMetrics, EmissionsTimePeriod};
pub use transparency::{TransparencyDashboard, TransparencyReport, TransparencyLevel};
pub use governance::{EnvironmentalGovernance, EnvironmentalProposal, ProposalStatus};
pub use alerting::{AlertingSystem, Alert, AlertRule};
pub use verification::{RenewableCertificate, CarbonOffset, VerificationService};

// Type aliases for common use
pub type HardwareType = types::HardwareType;
pub type Region = types::Region; 