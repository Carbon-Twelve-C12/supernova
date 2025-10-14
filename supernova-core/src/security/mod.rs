// Security module for Supernova blockchain
// Implements comprehensive security measures including quantum resistance

pub mod environmental_audit;
pub mod quantum_canary;
pub mod quantum_security_audit;

// Re-export audit types
pub use quantum_security_audit::{
    prepare_quantum_security_audit, test_environmental_oracle_security,
    validate_quantum_lightning_security, QuantumSecurityAuditReport, QuantumSecurityAuditor,
};

pub use environmental_audit::{
    prepare_environmental_system_audit, test_renewable_verification_security,
    validate_carbon_tracking_system, EnvironmentalSecurityAuditor, EnvironmentalSystemAuditReport,
};

// Re-export quantum canary types
pub use quantum_canary::{
    CanaryConfig, CanaryError, CanaryId, CanaryStatistics, CanaryStatus, DeploymentStrategy,
    MonitoringResult, QuantumCanary, QuantumCanarySystem,
};
