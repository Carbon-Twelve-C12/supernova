// Environmental subsystem for supernova blockchain
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
pub mod oracle_registry;
pub mod renewable_validation;
pub mod score_validation;
pub mod transparency;
pub mod treasury;
pub mod types;
pub mod verification;

// Re-export commonly used types with module-specific prefixes to avoid conflicts
pub use dashboard::{EmissionsTimePeriod, EnvironmentalDashboard, EnvironmentalMetrics};
pub use emissions::{
    EmissionCalculator, Emissions, EmissionsTracker, Region as EmissionsRegion, VerificationStatus,
};
pub use governance::{EnvironmentalGovernance, EnvironmentalProposal, ProposalStatus};
pub use miner_reporting::{MinerEnvironmentalInfo, MinerReportingManager, MinerVerificationStatus};
pub use transparency::{TransparencyDashboard, TransparencyLevel, TransparencyReport};
pub use treasury::{EnvironmentalAssetPurchase, EnvironmentalAssetType, EnvironmentalTreasury};
pub use types::{
    EmissionFactor, EmissionsFactorType, EnergySource, HardwareType as TypesHardwareType,
    Region as TypesRegion,
};
// pub use alerting::{AlertingSystem, Alert, AlertRule};  // Temporarily disabled for compilation
pub use oracle::{EnvironmentalOracle, OracleError, OracleInfo, OracleSubmission};
pub use oracle_registry::{
    ApplicationStatus, GovernanceVote, OracleApplication, OracleRegistry, RegisteredOracle,
    RegistryConfig, RegistryError, RegistryStats, SlashingCondition, SlashingConditionType,
    SlashingProposal,
};
pub use verification::{CarbonOffset, RenewableCertificate, VerificationService};

// New Phase 3 modules
pub use carbon_tracking::{
    CarbonTracker, CarbonTrackingResult, EnvironmentalMetrics as CarbonMetrics,
    OracleConsensusResult, OracleDataPoint, VerificationProof,
};
pub use manual_verification::{
    ManualVerificationRequest, ManualVerificationResult, ManualVerificationStatus,
    ManualVerificationSystem, QuarterlyReport, VerificationType,
};
pub use renewable_validation::{
    EnvironmentalDashboard as RenewableDashboard, EnvironmentalImpact, GreenMiningIncentive,
    RenewableEnergyValidator, RenewableValidationResult, ValidatedREC,
};

// Type aliases for common use
pub type HardwareType = types::HardwareType;
pub type Region = types::Region;

// Re-export main types for easier access
pub use self::emissions::{
    BlockEnvironmentalData, EmissionsConfig, NetworkHashrate, RegionalEnergyData,
    TransactionEnvironmentalData,
};

pub use self::treasury::{TreasuryAllocation, TreasuryConfig, TreasuryDistribution};
