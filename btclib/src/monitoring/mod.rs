// Monitoring and metrics module
// Provides system and blockchain monitoring capabilities

pub use prometheus;

pub mod system;
pub mod blockchain;
pub mod network;
pub mod consensus;
pub mod mempool;
pub mod quantum_signature_benchmarks;
pub mod blockchain_metrics;

// Metrics error type
#[derive(Debug, thiserror::Error)]
pub enum MetricsError {
    /// Prometheus error
    #[error("Prometheus error: {0}")]
    Prometheus(#[from] prometheus::Error),
    
    /// Registry error
    #[error("Registry error: {0}")]
    Registry(String),
    
    /// Initialization error
    #[error("Metrics initialization error: {0}")]
    Initialization(String),
    
    /// Collection error
    #[error("Metrics collection error: {0}")]
    Collection(String),
}

/// Initialize the metrics registry
pub fn create_registry() -> Result<prometheus::Registry, MetricsError> {
    Ok(prometheus::Registry::new())
} 