// integrity.rs - Core data integrity verification module
use std::sync::Arc;
use std::time::{Instant, SystemTime};

use tokio::sync::Mutex;
use tracing::{info, warn};

use crate::storage::BlockchainDB;
use crate::storage::corruption::CorruptionHandler;
use crate::storage::integrity::models::{IntegrityConfig, IntegrityIssue, VerificationLevel, VerificationResult};
use crate::storage::integrity::verifiers::{BlockchainVerifier, CryptoVerifier, DatabaseVerifier, UtxoVerifier};
use crate::storage::integrity::repair::IntegrityRepairer;
use crate::storage::StorageError;

pub mod models;
pub mod verifiers;
pub mod repair;

/// Main integrity verification system for blockchain data
pub struct IntegrityVerifier {
    /// Reference to blockchain database
    db: Arc<BlockchainDB>,
    /// Corruption handler for repairs
    corruption_handler: Option<Arc<Mutex<CorruptionHandler>>>,
    /// Verification configuration
    config: IntegrityConfig,
    /// Last verification result
    last_result: Option<VerificationResult>,
}

impl IntegrityVerifier {
    /// Create a new integrity verifier with default configuration
    pub fn new(db: Arc<BlockchainDB>) -> Self {
        Self {
            db,
            corruption_handler: None,
            config: IntegrityConfig::default(),
            last_result: None,
        }
    }

    /// Create a new integrity verifier with specific configuration
    pub fn with_config(db: Arc<BlockchainDB>, config: IntegrityConfig) -> Self {
        Self {
            db,
            corruption_handler: None,
            config,
            last_result: None,
        }
    }

    /// Set a corruption handler for repairs
    pub fn with_corruption_handler(mut self, handler: Arc<Mutex<CorruptionHandler>>) -> Self {
        self.corruption_handler = Some(handler);
        self
    }

    /// Add a trusted checkpoint
    pub fn add_checkpoint(&mut self, height: u64, hash: [u8; 32]) {
        self.config.trusted_checkpoints.insert(height, hash);
    }

    /// Get the last verification result
    pub fn last_result(&self) -> Option<&VerificationResult> {
        self.last_result.as_ref()
    }

    /// Perform data integrity verification at specified level
    pub async fn verify(&mut self, level: VerificationLevel) -> Result<VerificationResult, StorageError> {
        let start_time = Instant::now();
        info!("Starting blockchain integrity verification at {:?} level", level);

        let mut issues = Vec::new();
        let mut repairs_attempted = false;
        let mut repairs_successful = 0;

        // Create verifiers for different subsystems
        let db_verifier = DatabaseVerifier::new(Arc::clone(&self.db));
        let blockchain_verifier = BlockchainVerifier::new(Arc::clone(&self.db), &self.config);
        let utxo_verifier = UtxoVerifier::new(Arc::clone(&self.db), &self.config);
        let crypto_verifier = CryptoVerifier::new(Arc::clone(&self.db), &self.config);

        // Always perform basic database structure verification
        db_verifier.verify(&mut issues).await?;

        // For Standard or higher, verify blockchain consistency
        if level >= VerificationLevel::Standard {
            blockchain_verifier.verify(&mut issues).await?;
        }

        // For Full or higher, verify UTXO set
        if level >= VerificationLevel::Full {
            utxo_verifier.verify(&mut issues).await?;
        }

        // For Deep only, verify cryptographic proofs
        if level == VerificationLevel::Deep {
            crypto_verifier.verify(&mut issues).await?;
        }

        // Sort issues by severity
        issues.sort_by(|a, b| b.severity.cmp(&a.severity));

        // Attempt repairs if configured and critical issues exist
        if self.config.auto_repair && self.has_serious_issues(&issues) {
            if let Some(handler) = &self.corruption_handler {
                repairs_attempted = true;

                // Create repairer
                let repairer = IntegrityRepairer::new(
                    Arc::clone(&self.db),
                    Arc::clone(handler),
                    self.config.clone(),
                );

                // Attempt repairs
                let repair_count = repairer.repair_issues(&issues).await?;
                repairs_successful = repair_count;

                // If repairs were successful, re-verify
                if repairs_successful > 0 {
                    info!("Re-verifying after {} successful repairs", repairs_successful);
                    issues.clear();

                    // Re-run verifications
                    db_verifier.verify(&mut issues).await?;

                    if level >= VerificationLevel::Standard {
                        blockchain_verifier.verify(&mut issues).await?;
                    }

                    if level >= VerificationLevel::Full {
                        utxo_verifier.verify(&mut issues).await?;
                    }

                    if level == VerificationLevel::Deep {
                        crypto_verifier.verify(&mut issues).await?;
                    }
                }
            }
        }

        // Create result
        let duration = start_time.elapsed();
        let success = !self.has_critical_issues(&issues);

        let result = VerificationResult {
            success,
            duration,
            issues,
            time: SystemTime::now(),
            level,
            repairs_attempted,
            repairs_successful,
        };

        // Store result and log summary
        self.last_result = Some(result.clone());

        if result.success {
            info!("Integrity verification completed successfully in {:.2?}", duration);
        } else {
            warn!(
                "Integrity verification found {} critical issues in {:.2?}",
                result.issues.iter().filter(|i| i.is_critical()).count(),
                duration
            );
        }

        Ok(result)
    }

    /// Check if there are any critical issues
    fn has_critical_issues(&self, issues: &[IntegrityIssue]) -> bool {
        issues.iter().any(|i| i.is_critical())
    }

    /// Check if there are any serious issues that warrant repair
    fn has_serious_issues(&self, issues: &[IntegrityIssue]) -> bool {
        issues.iter().any(|i| i.is_serious())
    }
}