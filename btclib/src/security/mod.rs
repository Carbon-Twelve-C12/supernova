// Security module for Supernova blockchain
// Implements comprehensive security measures including quantum resistance

pub mod quantum_security_audit;
pub mod environmental_audit;
pub mod quantum_canary;

// Re-export audit types
pub use quantum_security_audit::{
    QuantumSecurityAuditReport, QuantumSecurityAuditor,
    prepare_quantum_security_audit, validate_quantum_lightning_security,
    test_environmental_oracle_security,
};

pub use environmental_audit::{
    EnvironmentalSystemAuditReport, EnvironmentalSecurityAuditor,
    prepare_environmental_system_audit, validate_carbon_tracking_system,
    test_renewable_verification_security,
};

// Re-export quantum canary types
pub use quantum_canary::{
    QuantumCanarySystem, QuantumCanary, CanaryConfig, CanaryId,
    DeploymentStrategy, CanaryStatus, MonitoringResult, CanaryStatistics,
    CanaryError
}; 