// integrity/models.rs - Data models for integrity verification
use std::collections::HashMap;
use std::time::Duration;
use std::time::SystemTime;

use serde::{Deserialize, Serialize};

/// Levels of verification for integrity checks
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum VerificationLevel {
    /// Basic database structure only
    Basic,
    /// Database and blockchain consistency
    Standard,
    /// Full verification including UTXO set
    Full,
    /// Deep verification including cryptographic proofs
    Deep,
}

/// Result of an integrity verification
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerificationResult {
    /// Whether verification passed without critical issues
    pub success: bool,
    /// Time taken for verification
    pub duration: Duration,
    /// Issues found during verification
    pub issues: Vec<IntegrityIssue>,
    /// Time when verification was performed
    pub time: SystemTime,
    /// Level of verification performed
    pub level: VerificationLevel,
    /// Whether repair was attempted
    pub repairs_attempted: bool,
    /// Number of successful repairs
    pub repairs_successful: usize,
}

/// An integrity issue found during verification
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IntegrityIssue {
    /// Type of issue
    pub issue_type: IssueType,
    /// Severity of the issue
    pub severity: IssueSeverity,
    /// Description of the issue
    pub description: String,
    /// Location where the issue was found
    pub location: IssueLocation,
    /// Whether this issue can be automatically repaired
    pub repairable: bool,
}

impl IntegrityIssue {
    /// Create a new integrity issue
    pub fn new(
        issue_type: IssueType,
        severity: IssueSeverity,
        description: String,
        location: IssueLocation,
        repairable: bool,
    ) -> Self {
        Self {
            issue_type,
            severity,
            description,
            location,
            repairable,
        }
    }

    /// Check if this is a critical issue
    pub fn is_critical(&self) -> bool {
        self.severity == IssueSeverity::Critical
    }

    /// Check if this is a serious issue (Error or Critical)
    pub fn is_serious(&self) -> bool {
        self.severity == IssueSeverity::Error || self.severity == IssueSeverity::Critical
    }
}

/// Types of integrity issues
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum IssueType {
    /// Database structure issue
    Structure,
    /// Chain consistency issue
    ChainInconsistency,
    /// UTXO set inconsistency
    UtxoInconsistency,
    /// Missing reference (block, tx, etc.)
    MissingReference,
    /// Cryptographic verification failure
    CryptoVerification,
    /// Index inconsistency
    IndexInconsistency,
    /// Consensus rule violation
    ConsensusViolation,
}

/// Severity levels for integrity issues
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum IssueSeverity {
    /// Informational issues (no impact on functioning)
    Info,
    /// Warning issues (minor impact)
    Warning,
    /// Error issues (functional impact)
    Error,
    /// Critical issues (system cannot function properly)
    Critical,
}

/// Location information for an integrity issue
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum IssueLocation {
    /// Block-related issue
    Block {
        /// Block hash
        hash: [u8; 32],
        /// Optional height
        height: Option<u64>,
    },
    /// Transaction-related issue
    Transaction {
        /// Transaction hash
        hash: [u8; 32],
        /// Optional containing block
        block_hash: Option<[u8; 32]>,
    },
    /// UTXO-related issue
    Utxo {
        /// Transaction hash
        tx_hash: [u8; 32],
        /// Output index
        output_index: u32,
    },
    /// Chain-related issue (spans multiple blocks)
    Chain {
        /// Starting height (if known)
        start_height: Option<u64>,
        /// Ending height (if known)
        end_height: Option<u64>,
    },
    /// Database component issue
    Database {
        /// Component name
        component: String,
    },
}

/// Chain requirements for validation
#[derive(Debug, Clone)]
pub struct ChainRequirements {
    /// Minimum allowed difficulty
    pub min_difficulty: u32,
    /// Maximum allowed block size (bytes)
    pub max_block_size: usize,
    /// Maximum time between blocks (seconds)
    pub max_time_between_blocks: u64,
    /// Maximum transactions per block
    pub max_txs_per_block: usize,
    /// Whether to enforce strict input ordering
    pub strict_input_order: bool,
}

impl Default for ChainRequirements {
    fn default() -> Self {
        Self {
            min_difficulty: 1,
            max_block_size: 1_000_000, // 1MB
            max_time_between_blocks: 7200, // 2 hours
            max_txs_per_block: 10_000,
            strict_input_order: true,
        }
    }
}

/// Configuration for integrity verification
#[derive(Debug, Clone)]
pub struct IntegrityConfig {
    /// Whether to attempt automatic repairs
    pub auto_repair: bool,
    /// Maximum blocks to check in one batch
    pub max_batch_size: usize,
    /// Trusted checkpoints (height -> hash)
    pub trusted_checkpoints: HashMap<u64, [u8; 32]>,
    /// Chain requirements
    pub requirements: ChainRequirements,
    /// Whether to verify signatures (expensive)
    pub verify_signatures: bool,
    /// Sample size for UTXO verification (percentage)
    pub utxo_sample_percentage: usize,
}

impl Default for IntegrityConfig {
    fn default() -> Self {
        Self {
            auto_repair: true,
            max_batch_size: 1000,
            trusted_checkpoints: HashMap::new(),
            requirements: ChainRequirements::default(),
            verify_signatures: false, // Expensive, off by default
            utxo_sample_percentage: 10, // Check 10% of UTXOs
        }
    }
}