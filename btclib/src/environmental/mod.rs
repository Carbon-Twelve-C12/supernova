// Environmental impact measurement and mitigation features module

// Export components for public API
pub mod emissions;
pub mod treasury;
pub mod dashboard;
pub mod miner_reporting;
pub mod emissions_factors;
pub mod hardware_types;
pub mod types;
pub mod api;

// Re-export key types for convenience
pub use emissions::{EmissionsTracker, Emissions, Region, HashRate, PoolId, PoolEnergyInfo};
pub use treasury::{EnvironmentalTreasury, EnvironmentalAssetType, EnvironmentalAssetPurchase};
pub use dashboard::{EnvironmentalDashboard, EnvironmentalMetrics, EmissionsTimePeriod};
pub use api::{
    EnvironmentalApi, 
    StandardEnvironmentalApi, 
    ThreadSafeEnvironmentalApi,
    EmissionsApiClient,
    EnvironmentalApiError,
    EnvironmentalResult,
    MinerEmissionsData,
    NetworkEmissionsData,
    RegionalEmissionsData,
    AssetPurchaseRecord,
    ReportingOptions,
}; 