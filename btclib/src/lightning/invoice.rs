// SuperNova Lightning Network - Invoice Implementation
//
// This file contains the implementation of Lightning Network payment invoices,
// including invoice generation, parsing, and verification.

use std::time::{SystemTime, UNIX_EPOCH, Duration};
use std::str::FromStr;
use std::fmt;
use thiserror::Error;
use sha2::{Sha256, Digest};
use rand::{thread_rng, Rng};

/// Error types for invoice operations
#[derive(Debug, Error)]
pub enum InvoiceError {
    #[error("Invalid invoice format: {0}")]
    InvalidFormat(String),
    
    #[error("Invalid payment hash: {0}")]
    InvalidHash(String),
    
    #[error("Invalid amount: {0}")]
    InvalidAmount(String),
    
    #[error("Expired invoice")]
    Expired,
    
    #[error("Missing required field: {0}")]
    MissingField(String),
    
    #[error("Invalid character in invoice: {0}")]
    InvalidCharacter(char),
    
    #[error("Parsing error: {0}")]
    ParseError(String),
    
    #[error("Invalid signature: {0}")]
    InvalidSignature(String),
    
    #[error("Unsupported feature bit: {0}")]
    UnsupportedFeature(u32),
}

/// Payment hash
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PaymentHash([u8; 32]);

impl PaymentHash {
    /// Create a new payment hash
    pub fn new(hash: [u8; 32]) -> Self {
        Self(hash)
    }
    
    /// Generate a random payment hash
    pub fn random() -> Self {
        let mut rng = thread_rng();
        let mut hash = [0u8; 32];
        rng.fill(&mut hash);
        Self(hash)
    }
    
    /// Get the raw hash bytes
    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

impl FromStr for PaymentHash {
    type Err = InvoiceError;
    
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s.len() != 64 {
            return Err(InvoiceError::InvalidHash(
                format!("Payment hash must be 64 hex characters, got {}", s.len())
            ));
        }
        
        let mut hash = [0u8; 32];
        hex::decode_to_slice(s, &mut hash)
            .map_err(|e| InvoiceError::InvalidHash(e.to_string()))?;
        
        Ok(Self(hash))
    }
}

impl fmt::Display for PaymentHash {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", hex::encode(self.0))
    }
}

/// Payment preimage
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PaymentPreimage([u8; 32]);

impl PaymentPreimage {
    /// Create a new payment preimage
    pub fn new(preimage: [u8; 32]) -> Self {
        Self(preimage)
    }
    
    /// Generate a random preimage
    pub fn random() -> Self {
        let mut rng = thread_rng();
        let mut preimage = [0u8; 32];
        rng.fill(&mut preimage);
        Self(preimage)
    }
    
    /// Get payment hash from preimage
    pub fn hash(&self) -> PaymentHash {
        let mut hasher = Sha256::new();
        hasher.update(self.0);
        let result = hasher.finalize();
        
        let mut hash = [0u8; 32];
        hash.copy_from_slice(&result);
        
        PaymentHash(hash)
    }
    
    /// Get the raw preimage bytes
    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

impl FromStr for PaymentPreimage {
    type Err = InvoiceError;
    
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s.len() != 64 {
            return Err(InvoiceError::InvalidHash(
                format!("Payment preimage must be 64 hex characters, got {}", s.len())
            ));
        }
        
        let mut preimage = [0u8; 32];
        hex::decode_to_slice(s, &mut preimage)
            .map_err(|e| InvoiceError::InvalidHash(e.to_string()))?;
        
        Ok(Self(preimage))
    }
}

impl fmt::Display for PaymentPreimage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", hex::encode(self.0))
    }
}

/// Route hint for private channels
#[derive(Debug, Clone)]
pub struct RouteHint {
    /// Node ID
    pub node_id: String,
    
    /// Channel ID
    pub channel_id: u64,
    
    /// Base fee in millisatoshis
    pub base_fee_msat: u32,
    
    /// Fee rate in parts per million
    pub fee_rate_millionths: u32,
    
    /// CLTV expiry delta
    pub cltv_expiry_delta: u16,
}

/// Invoice structure
#[derive(Debug, Clone)]
pub struct Invoice {
    /// Payment hash
    payment_hash: PaymentHash,
    
    /// Human-readable description
    description: String,
    
    /// Destination (node ID)
    destination: String,
    
    /// Amount in millisatoshis
    amount_msat: u64,
    
    /// Creation timestamp
    timestamp: u64,
    
    /// Expiry time in seconds from creation
    expiry: u32,
    
    /// Route hints for private channels
    route_hints: Vec<RouteHint>,
    
    /// Min final CLTV expiry delta
    min_final_cltv_expiry: u32,
    
    /// Invoice features
    features: u64,
    
    /// Invoice signature
    signature: Option<Vec<u8>>,
}

impl Invoice {
    /// Create a new invoice
    pub fn new(
        payment_hash: PaymentHash,
        amount_msat: u64,
        description: String,
        expiry: u32,
    ) -> Result<Self, InvoiceError> {
        if amount_msat == 0 {
            return Err(InvoiceError::InvalidAmount(
                "Amount must be greater than zero".to_string()
            ));
        }
        
        // For demonstration, we'll use a fixed node ID
        // In a real implementation, this would be derived from the node's public key
        let destination = "029a059f014307e795a31e1ddfdd19c7df6c7b1e2d09d6788c31ca4c38bac0f9ab".to_string();
        
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|e| InvoiceError::ParseError(e.to_string()))?
            .as_secs();
        
        Ok(Self {
            payment_hash,
            description,
            destination,
            amount_msat,
            timestamp,
            expiry,
            route_hints: Vec::new(),
            min_final_cltv_expiry: 40, // Default CLTV delta
            features: 0,               // No special features
            signature: None,           // No signature yet
        })
    }
    
    /// Parse an invoice from a string
    pub fn from_str(invoice_str: &str) -> Result<Self, InvoiceError> {
        // In a real implementation, this would parse a BOLT-11 invoice
        // For simplicity, we'll just return an error
        Err(InvoiceError::InvalidFormat(
            "Invoice parsing not implemented".to_string()
        ))
    }
    
    /// Get payment hash
    pub fn payment_hash(&self) -> PaymentHash {
        self.payment_hash
    }
    
    /// Get description
    pub fn description(&self) -> &str {
        &self.description
    }
    
    /// Get amount in millisatoshis
    pub fn amount_msat(&self) -> u64 {
        self.amount_msat
    }
    
    /// Get destination (node ID)
    pub fn destination(&self) -> &str {
        &self.destination
    }
    
    /// Check if the invoice is expired
    pub fn is_expired(&self) -> bool {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or(Duration::from_secs(0))
            .as_secs();
        
        now > self.timestamp + self.expiry as u64
    }
    
    /// Get route hints
    pub fn route_hints(&self) -> &[RouteHint] {
        &self.route_hints
    }
    
    /// Add a route hint
    pub fn add_route_hint(&mut self, hint: RouteHint) {
        self.route_hints.push(hint);
    }
    
    /// Get expiry time in seconds
    pub fn expiry(&self) -> u32 {
        self.expiry
    }
    
    /// Get creation timestamp
    pub fn timestamp(&self) -> u64 {
        self.timestamp
    }
    
    /// Get min final CLTV expiry delta
    pub fn min_final_cltv_expiry(&self) -> u32 {
        self.min_final_cltv_expiry
    }
    
    /// Set signature
    pub fn set_signature(&mut self, signature: Vec<u8>) {
        self.signature = Some(signature);
    }
    
    /// Check if the invoice has a signature
    pub fn has_signature(&self) -> bool {
        self.signature.is_some()
    }
    
    /// Verify the invoice signature
    pub fn verify_signature(&self) -> Result<bool, InvoiceError> {
        // In a real implementation, this would verify the signature
        // For simplicity, we'll just return Ok if a signature exists
        if self.signature.is_some() {
            Ok(true)
        } else {
            Err(InvoiceError::InvalidSignature(
                "Invoice has no signature".to_string()
            ))
        }
    }
    
    /// Encode the invoice as a string
    pub fn to_string(&self) -> Result<String, InvoiceError> {
        // In a real implementation, this would encode a BOLT-11 invoice
        // For simplicity, we'll just create a placeholder string
        let invoice_str = format!(
            "lnbc{}{}{}",
            self.amount_msat / 1000,
            self.payment_hash,
            self.timestamp
        );
        
        Ok(invoice_str)
    }
}

impl fmt::Display for Invoice {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.to_string() {
            Ok(s) => write!(f, "{}", s),
            Err(_) => write!(f, "Invoice({})", self.payment_hash),
        }
    }
} 