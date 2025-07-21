//! Error types for atomic swap operations

use thiserror::Error;

/// Main error type for atomic swap operations
#[derive(Error, Debug)]
pub enum AtomicSwapError {
    #[error("HTLC error: {0}")]
    HTLC(#[from] HTLCError),
    
    #[error("Swap error: {0}")]
    Swap(#[from] SwapError),
    
    #[error("Bitcoin adapter error: {0}")]
    BitcoinAdapter(#[from] BitcoinAdapterError),
    
    #[error("Monitoring error: {0}")]
    Monitor(#[from] MonitorError),
    
    #[error("Security error: {0}")]
    Security(#[from] SecurityError),
    
    #[error("Configuration error: {0}")]
    Config(String),
    
    #[error("Storage error: {0}")]
    Storage(String),
    
    #[error("Network error: {0}")]
    Network(String),
    
    #[error("Serialization error: {0}")]
    Serialization(#[from] bincode::Error),
    
    #[error("Other error: {0}")]
    Other(String),
}

impl HTLCError {
    pub fn Other(msg: String) -> Self {
        HTLCError::InvalidAmount(msg) // Using InvalidAmount as a general error for now
    }
}

/// HTLC-specific errors
#[derive(Error, Debug)]
pub enum HTLCError {
    #[error("Invalid hash preimage")]
    InvalidPreimage,
    
    #[error("Invalid signature")]
    InvalidSignature,
    
    #[error("Timeout not reached")]
    TimeoutNotReached,
    
    #[error("Already claimed")]
    AlreadyClaimed,
    
    #[error("Already refunded")]
    AlreadyRefunded,
    
    #[error("Invalid state transition from {from:?} to {to:?}")]
    InvalidStateTransition { from: String, to: String },
    
    #[error("Invalid timeout configuration")]
    InvalidTimeout,
    
    #[error("Invalid amount: {0}")]
    InvalidAmount(String),
}

/// Swap protocol errors
#[derive(Error, Debug)]
pub enum SwapError {
    #[error("Invalid swap state: {0}")]
    InvalidState(String),
    
    #[error("Swap not found: {0}")]
    SwapNotFound(String),
    
    #[error("Swap already exists: {0}")]
    SwapAlreadyExists(String),
    
    #[error("Timeout expired")]
    TimeoutExpired,
    
    #[error("Insufficient confirmations: need {required}, have {current}")]
    InsufficientConfirmations { required: u32, current: u32 },
    
    #[error("Fee calculation error: {0}")]
    FeeCalculation(String),
    
    #[error("Secret extraction failed")]
    SecretExtractionFailed,
    
    #[error("Chain reorganization detected")]
    ChainReorganization,
}

/// Bitcoin adapter errors
#[derive(Error, Debug)]
pub enum BitcoinAdapterError {
    #[error("RPC error: {0}")]
    RpcError(String),
    
    #[error("Script creation error: {0}")]
    ScriptError(String),
    
    #[error("Transaction parsing error: {0}")]
    ParseError(String),
    
    #[error("Network mismatch: expected {expected}, got {actual}")]
    NetworkMismatch { expected: String, actual: String },
    
    #[error("Insufficient funds")]
    InsufficientFunds,
    
    #[error("Bitcoin error: {0}")]
    BitcoinError(#[from] bitcoin::Error),
}

/// Monitoring errors
#[derive(Error, Debug)]
pub enum MonitorError {
    #[error("Connection lost to {chain}")]
    ConnectionLost { chain: String },
    
    #[error("Block stream error: {0}")]
    BlockStreamError(String),
    
    #[error("Event parsing error: {0}")]
    EventParseError(String),
    
    #[error("Sync error: {0}")]
    SyncError(String),
    
    #[error("Monitor not initialized")]
    NotInitialized,
    
    #[error("Swap not found")]
    SwapNotFound,
    
    #[error("Bitcoin RPC error: {0}")]
    BitcoinRpcError(String),
    
    #[error("Claim failed: {0}")]
    ClaimFailed(String),
    
    #[error("Refund failed: {0}")]
    RefundFailed(String),
    
    #[error("Secret not found")]
    SecretNotFound,
}

/// Security-related errors
#[derive(Error, Debug)]
pub enum SecurityError {
    #[error("Invalid timeout ordering")]
    InvalidTimeoutOrdering,
    
    #[error("Insufficient timeout delta")]
    InsufficientTimeoutDelta,
    
    #[error("Amount too low: minimum {min}")]
    AmountTooLow { min: u64 },
    
    #[error("Amount too high: maximum {max}")]
    AmountTooHigh { max: u64 },
    
    #[error("Rate limit exceeded")]
    RateLimitExceeded,
    
    #[error("Suspicious activity detected: {0}")]
    SuspiciousActivity(String),
    
    #[error("Cryptographic error: {0}")]
    CryptoError(String),
}

/// Secret extraction errors
#[derive(Error, Debug)]
pub enum ExtractionError {
    #[error("Secret not found in transaction")]
    SecretNotFound,
    
    #[error("Invalid secret length: expected 32, got {0}")]
    InvalidSecretLength(usize),
    
    #[error("Script parsing error: {0}")]
    ScriptParseError(String),
}

/// Cache-related errors
#[derive(Error, Debug)]
pub enum CacheError {
    #[error("Cache miss for {key}")]
    CacheMiss { key: String },
    
    #[error("Cache update failed: {0}")]
    UpdateFailed(String),
    
    #[error("Block fetch error: {0}")]
    BlockFetchError(String),
}

#[cfg(feature = "atomic-swap")]
/// Confidential swap errors
#[derive(Error, Debug)]
pub enum ConfidentialError {
    #[error("Commitment verification failed")]
    CommitmentMismatch,
    
    #[error("Range proof verification failed")]
    RangeProofFailed,
    
    #[error("Blinding factor error")]
    BlindingFactorError,
    
    #[error("Bulletproofs error: {0}")]
    BulletproofsError(String),
}

#[cfg(feature = "atomic-swap")]
/// Zero-knowledge swap errors
#[derive(Error, Debug)]
pub enum ZKSwapError {
    #[error("Proof generation failed: {0}")]
    ProofGenerationFailed(String),
    
    #[error("Proof verification failed: {0}")]
    VerificationFailed(String),
    
    #[error("Nullifier already spent")]
    NullifierSpent,
    
    #[error("Merkle proof invalid")]
    InvalidMerkleProof,
    
    #[error("Circuit synthesis error: {0}")]
    CircuitError(String),
    
    #[error("Setup error: {0}")]
    SetupError(String),
    
    #[error("Invalid input: {0}")]
    InvalidInput(String),
    
    #[error("Serialization error: {0}")]
    SerializationError(String),
    
    #[error("Deserialization error: {0}")]
    DeserializationError(String),
}

impl From<std::io::Error> for AtomicSwapError {
    fn from(err: std::io::Error) -> Self {
        AtomicSwapError::Other(err.to_string())
    }
}

impl From<serde_json::Error> for AtomicSwapError {
    fn from(err: serde_json::Error) -> Self {
        AtomicSwapError::Serialization(bincode::Error::from(bincode::ErrorKind::Custom(err.to_string())))
    }
} 