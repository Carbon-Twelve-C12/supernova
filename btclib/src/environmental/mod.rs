// Environmental subsystem for SuperNova blockchain
// Provides features for environmental monitoring, carbon offsetting, and ESG compliance

// Re-export all modules
pub mod api;
// pub mod alerting;  // Temporarily disabled for compilation
pub mod carbon_tracking;
pub mod dashboard;
pub mod emissions;
pub mod governance;
pub mod manual_verification;
pub mod miner_reporting;
pub mod oracle;
pub mod renewable_validation;
pub mod transparency;
pub mod treasury;
pub mod types;
pub mod verification;

// Re-export commonly used types with module-specific prefixes to avoid conflicts
pub use types::{Region as TypesRegion, EnergySource, EmissionFactor, EmissionsFactorType, HardwareType as TypesHardwareType};
pub use miner_reporting::{MinerEnvironmentalInfo, MinerReportingManager, MinerVerificationStatus};
pub use emissions::{EmissionsTracker, EmissionCalculator, Region as EmissionsRegion, VerificationStatus, Emissions};
pub use treasury::{EnvironmentalTreasury, EnvironmentalAssetType, EnvironmentalAssetPurchase};
pub use dashboard::{EnvironmentalDashboard, EnvironmentalMetrics, EmissionsTimePeriod};
pub use transparency::{TransparencyDashboard, TransparencyReport, TransparencyLevel};
pub use governance::{EnvironmentalGovernance, EnvironmentalProposal, ProposalStatus};
// pub use alerting::{AlertingSystem, Alert, AlertRule};  // Temporarily disabled for compilation
pub use verification::{RenewableCertificate, CarbonOffset, VerificationService};
pub use oracle::{EnvironmentalOracle, OracleError, OracleInfo, OracleSubmission};

// New Phase 3 modules
pub use carbon_tracking::{
    CarbonTracker, CarbonTrackingResult, OracleConsensusResult, 
    OracleDataPoint, EnvironmentalMetrics as CarbonMetrics, VerificationProof,
};
pub use renewable_validation::{
    RenewableEnergyValidator, RenewableValidationResult, ValidatedREC,
    GreenMiningIncentive, EnvironmentalImpact, EnvironmentalDashboard as RenewableDashboard,
};
pub use manual_verification::{
    ManualVerificationSystem, ManualVerificationRequest, ManualVerificationResult,
    VerificationType, ManualVerificationStatus, QuarterlyReport,
};

// Type aliases for common use
pub type HardwareType = types::HardwareType;
pub type Region = types::Region; 

// Re-export main types for easier access
pub use self::emissions::{
    BlockEnvironmentalData, 
    TransactionEnvironmentalData,
    RegionalEnergyData,
    NetworkHashrate,
    EmissionsConfig,
};

pub use self::treasury::{
    TreasuryConfig,
    TreasuryDistribution,
    TreasuryAllocation,
}; 