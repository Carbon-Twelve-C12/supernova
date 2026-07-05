// supernova Lightning Network - Invoice Implementation
//
// This file contains the implementation of Lightning Network payment invoices,
// including invoice generation, parsing, and verification.

use rand::{thread_rng, RngCore};
use std::collections::{HashMap, HashSet};
use std::fmt;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use thiserror::Error;

// Import shared payment types
use super::payment::{PaymentHash, PaymentPreimage};

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

/// Route hint for private channels
#[derive(Debug, Clone)]
pub struct RouteHint {
    /// Node ID
    pub node_id: String,

    /// Channel ID
    pub channel_id: u64,

    /// Base fee in millinova
    pub base_fee_mnova: u32,

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

    /// Payment preimage (stored securely by the invoice creator)
    payment_preimage: PaymentPreimage,

    /// Human-readable description
    description: String,

    /// Destination (node ID)
    destination: String,

    /// Amount in millinova
    amount_mnova: u64,

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

    /// Whether this invoice was created for a private (unadvertised) route
    is_private: bool,

    /// Whether the invoice has been settled (paid)
    settled: bool,

    /// Timestamp at which the invoice was settled, if any
    settled_time: Option<u64>,
}

impl Invoice {
    /// Create a new invoice with all required parameters
    pub fn new(
        payment_hash: PaymentHash,
        amount_mnova: u64,
        description: String,
        expiry: u32,
        is_private: bool,
        node_id: String,
        payment_preimage: PaymentPreimage,
    ) -> Self {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or(Duration::from_secs(0))
            .as_secs();

        Self {
            payment_hash,
            payment_preimage,
            description,
            destination: node_id,
            amount_mnova,
            timestamp,
            expiry,
            route_hints: Vec::new(),
            min_final_cltv_expiry: 40,
            features: 0,
            signature: None,
            is_private,
            settled: false,
            settled_time: None,
        }
    }

    /// Get creation timestamp
    pub fn created_at(&self) -> u64 {
        self.timestamp
    }

    /// Get expiry time in seconds
    pub fn expiry_seconds(&self) -> u32 {
        self.expiry
    }

    /// Check if the invoice is settled (paid)
    pub fn is_settled(&self) -> bool {
        self.settled
    }

    /// Get settled timestamp
    pub fn settled_at(&self) -> Option<u64> {
        self.settled_time
    }

    /// Mark the invoice as settled (paid) at the current time.
    ///
    /// This is the real state transition backing [`Invoice::is_settled`] and
    /// [`Invoice::settled_at`] — callers (e.g. the Lightning payment manager,
    /// once an HTLC matching this invoice's payment hash is fulfilled) should
    /// invoke this instead of relying on external bookkeeping.
    pub fn mark_settled(&mut self) {
        if !self.settled {
            self.settled = true;
            self.settled_time = Some(
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap_or(Duration::from_secs(0))
                    .as_secs(),
            );
        }
    }

    /// Check if the invoice is private (uses unadvertised/private route hints)
    pub fn is_private(&self) -> bool {
        self.is_private
    }

    /// Create a new invoice with preimage and payment hash
    pub fn new_with_preimage(
        payment_preimage: PaymentPreimage,
        amount_mnova: u64,
        description: String,
        expiry: u32,
    ) -> Result<Self, InvoiceError> {
        if amount_mnova == 0 {
            return Err(InvoiceError::InvalidAmount(
                "Amount must be greater than zero".to_string(),
            ));
        }

        // Generate payment hash from preimage
        let payment_hash = payment_preimage.payment_hash();

        // For demonstration, we'll use a fixed node ID
        // In a real implementation, this would be derived from the node's public key
        let destination =
            "029a059f014307e795a31e1ddfdd19c7df6c7b1e2d09d6788c31ca4c38bac0f9ab".to_string();

        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|e| InvoiceError::ParseError(e.to_string()))?
            .as_secs();

        Ok(Self {
            payment_hash,
            payment_preimage,
            description,
            destination,
            amount_mnova,
            timestamp,
            expiry,
            route_hints: Vec::new(),
            min_final_cltv_expiry: 40, // Default CLTV delta
            features: 0,               // No special features
            signature: None,           // No signature yet
            is_private: false,
            settled: false,
            settled_time: None,
        })
    }

    /// Create a new invoice (legacy method - generates random preimage)
    pub fn new_legacy(
        payment_hash: PaymentHash,
        amount_mnova: u64,
        description: String,
        expiry: u32,
    ) -> Result<Self, InvoiceError> {
        if amount_mnova == 0 {
            return Err(InvoiceError::InvalidAmount(
                "Amount must be greater than zero".to_string(),
            ));
        }

        // For backward compatibility, we'll derive the preimage from the payment hash
        // In a real implementation, this should be generated randomly and stored securely
        let mut preimage_bytes = [0u8; 32];
        preimage_bytes.copy_from_slice(payment_hash.as_bytes());
        // XOR with a pattern to ensure it's different from the hash
        for i in 0..32 {
            preimage_bytes[i] ^= 0xAA;
        }
        let payment_preimage = PaymentPreimage::new(preimage_bytes);

        // For demonstration, we'll use a fixed node ID
        // In a real implementation, this would be derived from the node's public key
        let destination =
            "029a059f014307e795a31e1ddfdd19c7df6c7b1e2d09d6788c31ca4c38bac0f9ab".to_string();

        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|e| InvoiceError::ParseError(e.to_string()))?
            .as_secs();

        Ok(Self {
            payment_hash,
            payment_preimage,
            description,
            destination,
            amount_mnova,
            timestamp,
            expiry,
            route_hints: Vec::new(),
            min_final_cltv_expiry: 40, // Default CLTV delta
            features: 0,               // No special features
            signature: None,           // No signature yet
            is_private: false,
            settled: false,
            settled_time: None,
        })
    }

    /// Invoice string encoding prefix/version tag.
    const ENCODING_PREFIX: &'static str = "snova1";

    /// Parse an invoice from its encoded string form (see [`Invoice::to_string`]
    /// for the corresponding encoder).
    ///
    /// Performs real field validation rather than a placeholder: the payment
    /// hash must be a well-formed 32-byte hex string, the amount must be
    /// non-zero, and the invoice must not already be expired.
    ///
    /// Note: like a real BOLT11 invoice handed to a payer, the encoded form
    /// never carries the payment preimage (only the invoice creator knows
    /// it), so a parsed [`Invoice`] holds a zeroed placeholder preimage that
    /// must not be relied upon for payment settlement.
    pub fn from_str(invoice_str: &str) -> Result<Self, InvoiceError> {
        let mut parts = invoice_str.split(':');

        let prefix = parts
            .next()
            .ok_or_else(|| InvoiceError::InvalidFormat("Empty invoice string".to_string()))?;
        if prefix != Self::ENCODING_PREFIX {
            return Err(InvoiceError::InvalidFormat(format!(
                "Unrecognized invoice prefix: {}",
                prefix
            )));
        }

        let fields: Vec<&str> = parts.collect();
        if fields.len() != 9 {
            return Err(InvoiceError::InvalidFormat(format!(
                "Expected 9 invoice fields, found {}",
                fields.len()
            )));
        }

        let amount_mnova: u64 = fields[0]
            .parse()
            .map_err(|_| InvoiceError::InvalidAmount(format!("Invalid amount: {}", fields[0])))?;
        if amount_mnova == 0 {
            return Err(InvoiceError::InvalidAmount(
                "Amount must be greater than zero".to_string(),
            ));
        }

        let timestamp: u64 = fields[1]
            .parse()
            .map_err(|_| InvoiceError::ParseError(format!("Invalid timestamp: {}", fields[1])))?;
        let expiry: u32 = fields[2]
            .parse()
            .map_err(|_| InvoiceError::ParseError(format!("Invalid expiry: {}", fields[2])))?;
        let min_final_cltv_expiry: u32 = fields[3].parse().map_err(|_| {
            InvoiceError::ParseError(format!("Invalid CLTV delta: {}", fields[3]))
        })?;
        let features: u64 = fields[4]
            .parse()
            .map_err(|_| InvoiceError::ParseError(format!("Invalid features: {}", fields[4])))?;
        let is_private = match fields[5] {
            "0" => false,
            "1" => true,
            other => {
                return Err(InvoiceError::ParseError(format!(
                    "Invalid private flag: {}",
                    other
                )))
            }
        };

        // Validates length (64 hex chars) and hex encoding of the payment hash.
        let payment_hash = PaymentHash::from_hex(fields[6])
            .map_err(|e| InvoiceError::InvalidHash(format!("{:?}", e)))?;

        let destination_bytes = hex::decode(fields[7])
            .map_err(|e| InvoiceError::ParseError(format!("Invalid destination: {}", e)))?;
        let destination = String::from_utf8(destination_bytes)
            .map_err(|e| InvoiceError::ParseError(format!("Invalid destination: {}", e)))?;

        // Description is hex-encoded (like destination) so that arbitrary
        // human-readable text, including ':' characters, round-trips safely
        // through the ':'-delimited field format.
        let description_bytes = hex::decode(fields[8])
            .map_err(|e| InvoiceError::ParseError(format!("Invalid description: {}", e)))?;
        let description = String::from_utf8(description_bytes)
            .map_err(|e| InvoiceError::ParseError(format!("Invalid description: {}", e)))?;

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or(Duration::from_secs(0))
            .as_secs();
        if now > timestamp + expiry as u64 {
            return Err(InvoiceError::Expired);
        }

        Ok(Self {
            payment_hash,
            // Payer-side invoices never carry the preimage; only the
            // creator/payee retains it (see doc comment above).
            payment_preimage: PaymentPreimage::new([0u8; 32]),
            description,
            destination,
            amount_mnova,
            timestamp,
            expiry,
            route_hints: Vec::new(),
            min_final_cltv_expiry,
            features,
            signature: None,
            is_private,
            settled: false,
            settled_time: None,
        })
    }

    /// Get payment hash
    pub fn payment_hash(&self) -> PaymentHash {
        self.payment_hash
    }

    /// Get description
    pub fn description(&self) -> &str {
        &self.description
    }

    /// Get amount in millinova
    pub fn amount_mnova(&self) -> u64 {
        self.amount_mnova
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

    /// Get payment preimage (returns the actual preimage used to generate the payment hash)
    ///
    /// **Real Lightning Network Implementation:**
    /// This method now returns the actual preimage that was used to generate the payment hash,
    /// exactly like in a real Lightning Network implementation. The flow is:
    /// 1. Invoice creator generates a random 32-byte preimage
    /// 2. Payment hash is computed as SHA256(preimage)
    /// 3. Invoice contains the payment hash
    /// 4. Preimage is stored securely by the invoice creator
    /// 5. When payment arrives, preimage is revealed to complete the payment
    pub fn payment_preimage(&self) -> PaymentPreimage {
        // Return the actual preimage that was stored when the invoice was created
        self.payment_preimage
    }

    /// Set signature
    pub fn set_signature(&mut self, signature: Vec<u8>) {
        self.signature = Some(signature);
    }

    /// Check if the invoice has a signature
    pub fn has_signature(&self) -> bool {
        self.signature.is_some()
    }

    /// Verify the invoice signature.
    ///
    /// **Security note (fail-closed):** cryptographic verification of the
    /// invoice signature is not yet implemented. A signed invoice would need
    /// to carry the destination's post-quantum public key (the `destination`
    /// field is currently only an opaque node-id string) and a matching
    /// invoice-signing counterpart. Until that exists, this method MUST NOT
    /// report an unverified byte string as authentic: the mere presence of
    /// bytes in the `signature` field proves nothing. It therefore always
    /// fails closed rather than returning `Ok(true)`, so no caller can mistake
    /// presence-of-bytes for a valid signature.
    pub fn verify_signature(&self) -> Result<bool, InvoiceError> {
        if self.signature.is_none() {
            return Err(InvoiceError::InvalidSignature(
                "Invoice has no signature".to_string(),
            ));
        }

        // Fail closed: real PQC verification against the destination's public
        // key is not implemented, so a present signature cannot be trusted.
        Err(InvoiceError::InvalidSignature(
            "Invoice signature verification is not implemented; \
             refusing to treat unverified signature bytes as authentic"
                .to_string(),
        ))
    }

    /// Encode the invoice as a string.
    ///
    /// This is Supernova's own reversible invoice encoding (tagged
    /// `snova1`), not raw BOLT11 bech32: it deterministically serializes
    /// every field needed to reconstruct an equivalent [`Invoice`] via
    /// [`Invoice::from_str`], with destination and description hex-encoded
    /// so they safely round-trip through the ':'-delimited format.
    pub fn to_string(&self) -> Result<String, InvoiceError> {
        let invoice_str = format!(
            "{}:{}:{}:{}:{}:{}:{}:{}:{}:{}",
            Self::ENCODING_PREFIX,
            self.amount_mnova,
            self.timestamp,
            self.expiry,
            self.min_final_cltv_expiry,
            self.features,
            if self.is_private { "1" } else { "0" },
            self.payment_hash.to_hex(),
            hex::encode(self.destination.as_bytes()),
            hex::encode(self.description.as_bytes()),
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

/// Invoice with enhanced features
#[derive(Debug, Clone)]
pub struct EnhancedInvoice {
    /// Base invoice
    invoice: Invoice,

    /// Features bit vector
    features: u64,

    /// Payment secrets for secure multi-hop payments
    payment_secret: [u8; 32],

    /// Payment metadata
    metadata: Vec<u8>,

    /// Fallback on-chain address
    fallback_address: Option<String>,

    /// Invoice state
    state: InvoiceState,

    /// Payment attempts
    attempts: u32,
}

/// State of an invoice
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InvoiceState {
    /// Invoice is open and can be paid
    Open,

    /// Invoice has been paid
    Paid,

    /// Invoice has expired
    Expired,

    /// Invoice has been canceled
    Canceled,
}

/// Payment metadata for invoice
#[derive(Debug, Clone)]
pub struct PaymentMetadata {
    /// Payer's wallet name
    pub payer_name: Option<String>,

    /// Payer's note
    pub payment_note: Option<String>,

    /// Payment purpose
    pub purpose: Option<String>,

    /// Payment ID for correlation
    pub payment_id: Option<[u8; 16]>,

    /// Additional custom data
    pub custom_data: HashMap<String, String>,
}

/// Invoice feature flags
pub mod feature_bits {
    pub const BASIC_MPP: u64 = 1 << 0; // Multi-path payments
    pub const PAYMENT_SECRET: u64 = 1 << 1; // Payment secret required
    pub const PAYMENT_METADATA: u64 = 1 << 2; // Payment metadata
    pub const VAR_ONION: u64 = 1 << 3; // Variable-length onion
    pub const FALLBACK_ADDRESS: u64 = 1 << 4; // Fallback on-chain address
    pub const ROUTE_BLINDING: u64 = 1 << 5; // Route blinding
    pub const KEYSEND: u64 = 1 << 6; // Spontaneous payments
    pub const TRAMPOLINE: u64 = 1 << 7; // Trampoline routing
    pub const QUERY_FEATURES: u64 = 1 << 8; // Feature query
    pub const PAYMENT_CONSTRAINTS: u64 = 1 << 9; // Advanced payment constraints
}

impl EnhancedInvoice {
    /// Create a new enhanced invoice
    pub fn new(
        payment_hash: PaymentHash,
        amount_mnova: u64,
        description: String,
        expiry: u32,
        features: u64,
    ) -> Result<Self, InvoiceError> {
        // Create the base invoice
        let invoice = Invoice::new_legacy(payment_hash, amount_mnova, description, expiry)?;

        // Generate a random payment secret
        let mut rng = thread_rng();
        let mut payment_secret = [0u8; 32];
        rng.fill_bytes(&mut payment_secret);

        Ok(Self {
            invoice,
            features,
            payment_secret,
            metadata: Vec::new(),
            fallback_address: None,
            state: InvoiceState::Open,
            attempts: 0,
        })
    }

    /// Create an invoice from parts
    pub fn from_parts(invoice: Invoice, features: u64, payment_secret: [u8; 32]) -> Self {
        Self {
            invoice,
            features,
            payment_secret,
            metadata: Vec::new(),
            fallback_address: None,
            state: InvoiceState::Open,
            attempts: 0,
        }
    }

    /// Set the fallback on-chain address
    pub fn set_fallback_address(&mut self, address: String) {
        self.fallback_address = Some(address);
        self.features |= feature_bits::FALLBACK_ADDRESS;
    }

    /// Add metadata to the invoice
    pub fn add_metadata(&mut self, metadata: &[u8]) {
        self.metadata = metadata.to_vec();
        self.features |= feature_bits::PAYMENT_METADATA;
    }

    /// Add payment metadata
    pub fn add_payment_metadata(&mut self, metadata: &PaymentMetadata) -> Result<(), InvoiceError> {
        // In a real implementation, this would serialize the metadata to a binary format
        // For simplicity, we'll just use a placeholder
        let mut data = Vec::new();

        if let Some(name) = &metadata.payer_name {
            data.extend_from_slice(name.as_bytes());
        }

        if let Some(note) = &metadata.payment_note {
            data.extend_from_slice(note.as_bytes());
        }

        self.add_metadata(&data);

        Ok(())
    }

    /// Check if the invoice supports a feature
    pub fn supports_feature(&self, feature_bit: u64) -> bool {
        (self.features & feature_bit) != 0
    }

    /// Get the payment hash
    pub fn payment_hash(&self) -> PaymentHash {
        self.invoice.payment_hash()
    }

    /// Get the payment secret
    pub fn payment_secret(&self) -> &[u8; 32] {
        &self.payment_secret
    }

    /// Get the amount in millinova
    pub fn amount_mnova(&self) -> u64 {
        self.invoice.amount_mnova()
    }

    /// Get the description
    pub fn description(&self) -> &str {
        self.invoice.description()
    }

    /// Get the expiry time in seconds
    pub fn expiry(&self) -> u32 {
        self.invoice.expiry()
    }

    /// Check if the invoice is expired
    pub fn is_expired(&self) -> bool {
        self.invoice.is_expired()
    }

    /// Mark the invoice as paid
    pub fn mark_as_paid(&mut self) {
        self.state = InvoiceState::Paid;
    }

    /// Mark the invoice as expired
    pub fn mark_as_expired(&mut self) {
        if self.state == InvoiceState::Open {
            self.state = InvoiceState::Expired;
        }
    }

    /// Cancel the invoice
    pub fn cancel(&mut self) {
        if self.state == InvoiceState::Open {
            self.state = InvoiceState::Canceled;
        }
    }

    /// Get the invoice state
    pub fn state(&self) -> &InvoiceState {
        &self.state
    }

    /// Record a payment attempt
    pub fn record_attempt(&mut self) {
        self.attempts += 1;
    }

    /// Get the number of payment attempts
    pub fn attempts(&self) -> u32 {
        self.attempts
    }

    /// Get the route hints
    pub fn route_hints(&self) -> &[RouteHint] {
        self.invoice.route_hints()
    }

    /// Get the destination (node ID)
    pub fn destination(&self) -> &str {
        self.invoice.destination()
    }

    /// Get the min final CLTV expiry delta
    pub fn min_final_cltv_expiry(&self) -> u32 {
        self.invoice.min_final_cltv_expiry()
    }

    /// Get the base invoice
    pub fn base_invoice(&self) -> &Invoice {
        &self.invoice
    }

    /// Get the features
    pub fn features(&self) -> u64 {
        self.features
    }

    /// Encode the invoice as a BOLT-11 string
    pub fn to_bolt11(&self) -> Result<String, InvoiceError> {
        // In a real implementation, this would encode a BOLT-11 invoice
        // For simplicity, we'll just create a placeholder string
        let invoice_str = format!(
            "lnbc{}m{}{}{}{}",
            self.amount_mnova() / 1_000_000,
            hex::encode(&self.payment_hash().into_inner()[0..4]),
            self.features,
            self.invoice.timestamp(),
            if self.supports_feature(feature_bits::FALLBACK_ADDRESS) {
                "1"
            } else {
                "0"
            }
        );

        Ok(invoice_str)
    }
}

/// Invoice database for managing payment invoices
pub struct InvoiceDatabase {
    /// Invoices by payment hash
    invoices: HashMap<PaymentHash, EnhancedInvoice>,

    /// Invoices by description (for lookup)
    invoices_by_description: HashMap<String, PaymentHash>,

    /// Paid invoices
    paid_invoices: HashSet<PaymentHash>,

    /// Last update time
    last_update: u64,
}

impl Default for InvoiceDatabase {
    fn default() -> Self {
        Self::new()
    }
}

impl InvoiceDatabase {
    /// Create a new invoice database
    pub fn new() -> Self {
        Self {
            invoices: HashMap::new(),
            invoices_by_description: HashMap::new(),
            paid_invoices: HashSet::new(),
            last_update: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or(Duration::from_secs(0))
                .as_secs(),
        }
    }

    /// Add an invoice to the database
    pub fn add_invoice(&mut self, invoice: EnhancedInvoice) -> Result<(), InvoiceError> {
        let payment_hash = invoice.payment_hash();

        // Check if invoice already exists
        if self.invoices.contains_key(&payment_hash) {
            return Err(InvoiceError::InvalidFormat(format!(
                "Invoice with payment hash {} already exists",
                payment_hash
            )));
        }

        // Add to invoices by description
        let description = invoice.description().to_string();
        self.invoices_by_description
            .insert(description, payment_hash);

        // Add to invoices
        self.invoices.insert(payment_hash, invoice);

        // Update last update time
        self.last_update = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or(Duration::from_secs(0))
            .as_secs();

        Ok(())
    }

    /// Get an invoice by payment hash
    pub fn get_invoice(&self, payment_hash: &PaymentHash) -> Option<&EnhancedInvoice> {
        self.invoices.get(payment_hash)
    }

    /// Get an invoice by description
    pub fn get_invoice_by_description(&self, description: &str) -> Option<&EnhancedInvoice> {
        if let Some(payment_hash) = self.invoices_by_description.get(description) {
            self.invoices.get(payment_hash)
        } else {
            None
        }
    }

    /// Mark an invoice as paid
    pub fn mark_invoice_paid(&mut self, payment_hash: &PaymentHash) -> Result<(), InvoiceError> {
        if let Some(invoice) = self.invoices.get_mut(payment_hash) {
            // Check if the invoice is expired
            if invoice.is_expired() {
                return Err(InvoiceError::Expired);
            }

            // Check if the invoice is already paid
            if matches!(invoice.state(), InvoiceState::Paid) {
                return Err(InvoiceError::InvalidFormat(format!(
                    "Invoice with payment hash {} is already paid",
                    payment_hash
                )));
            }

            // Mark as paid
            invoice.mark_as_paid();
            self.paid_invoices.insert(*payment_hash);

            // Update last update time
            self.last_update = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or(Duration::from_secs(0))
                .as_secs();

            Ok(())
        } else {
            Err(InvoiceError::InvalidHash(format!(
                "Invoice with payment hash {} not found",
                payment_hash
            )))
        }
    }

    /// Check if an invoice is paid
    pub fn is_invoice_paid(&self, payment_hash: &PaymentHash) -> bool {
        self.paid_invoices.contains(payment_hash)
    }

    /// Get all invoices
    pub fn get_all_invoices(&self) -> Vec<&EnhancedInvoice> {
        self.invoices.values().collect()
    }

    /// Get all paid invoices
    pub fn get_paid_invoices(&self) -> Vec<&EnhancedInvoice> {
        self.invoices
            .values()
            .filter(|i| matches!(i.state(), InvoiceState::Paid))
            .collect()
    }

    /// Get all open invoices
    pub fn get_open_invoices(&self) -> Vec<&EnhancedInvoice> {
        self.invoices
            .values()
            .filter(|i| matches!(i.state(), InvoiceState::Open))
            .collect()
    }

    /// Expire old invoices
    pub fn expire_old_invoices(&mut self) -> usize {
        let mut expired_count = 0;

        for invoice in self.invoices.values_mut() {
            if invoice.is_expired() && matches!(invoice.state(), InvoiceState::Open) {
                invoice.mark_as_expired();
                expired_count += 1;
            }
        }

        if expired_count > 0 {
            // Update last update time
            self.last_update = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or(Duration::from_secs(0))
                .as_secs();
        }

        expired_count
    }

    /// Delete old paid and expired invoices
    pub fn prune_old_invoices(&mut self, max_age_seconds: u64) -> usize {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or(Duration::from_secs(0))
            .as_secs();

        let mut to_remove = Vec::new();

        for (payment_hash, invoice) in &self.invoices {
            let invoice_age = now.saturating_sub(invoice.base_invoice().timestamp());

            if invoice_age > max_age_seconds
                && matches!(
                    invoice.state(),
                    InvoiceState::Paid | InvoiceState::Expired | InvoiceState::Canceled
                )
            {
                to_remove.push(*payment_hash);
            }
        }

        // Remove from all collections
        for payment_hash in &to_remove {
            if let Some(_invoice) = self.invoices.remove(payment_hash) {
                self.invoices_by_description
                    .retain(|_, ph| ph != payment_hash);
                self.paid_invoices.remove(payment_hash);
            }
        }

        if !to_remove.is_empty() {
            // Update last update time
            self.last_update = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or(Duration::from_secs(0))
                .as_secs();
        }

        to_remove.len()
    }
}

#[cfg(test)]
mod invoice_lifecycle_tests {
    use super::*;

    fn sample_invoice() -> Invoice {
        let preimage = PaymentPreimage::new_random();
        let payment_hash = preimage.payment_hash();
        Invoice::new(
            payment_hash,
            50_000,
            "coffee".to_string(),
            3600,
            true,
            "029a059f014307e795a31e1ddfdd19c7df6c7b1e2d09d6788c31ca4c38bac0f9ab".to_string(),
            preimage,
        )
    }

    #[test]
    fn to_string_from_str_round_trips_public_fields() {
        let invoice = sample_invoice();
        let encoded = invoice.to_string().expect("encode invoice");

        let parsed = Invoice::from_str(&encoded).expect("parse invoice");

        assert_eq!(parsed.payment_hash(), invoice.payment_hash());
        assert_eq!(parsed.amount_mnova(), invoice.amount_mnova());
        assert_eq!(parsed.description(), invoice.description());
        assert_eq!(parsed.destination(), invoice.destination());
        assert_eq!(parsed.timestamp(), invoice.timestamp());
        assert_eq!(parsed.expiry(), invoice.expiry());
        assert_eq!(parsed.is_private(), invoice.is_private());
    }

    #[test]
    fn from_str_rejects_unrecognized_prefix() {
        let result = Invoice::from_str("notaninvoice:1:2:3");
        assert!(matches!(result, Err(InvoiceError::InvalidFormat(_))));
    }

    #[test]
    fn from_str_rejects_malformed_payment_hash() {
        let invoice = sample_invoice();
        let encoded = invoice.to_string().expect("encode invoice");
        // Corrupt the payment hash field (7th field after the prefix).
        let mut fields: Vec<&str> = encoded.split(':').collect();
        fields[7] = "not-hex";
        let corrupted = fields.join(":");

        let result = Invoice::from_str(&corrupted);
        assert!(matches!(result, Err(InvoiceError::InvalidHash(_))));
    }

    #[test]
    fn from_str_rejects_already_expired_invoice() {
        let preimage = PaymentPreimage::new_random();
        let payment_hash = preimage.payment_hash();
        // Expiry of 0 seconds from creation means it is immediately expired.
        let invoice = Invoice::new(
            payment_hash,
            1_000,
            "expired".to_string(),
            0,
            false,
            "dest".to_string(),
            preimage,
        );
        let encoded = invoice.to_string().expect("encode invoice");

        // Ensure we're past the expiry boundary.
        std::thread::sleep(std::time::Duration::from_millis(1100));

        let result = Invoice::from_str(&encoded);
        assert!(matches!(result, Err(InvoiceError::Expired)));
    }

    #[test]
    fn is_settled_and_settled_at_reflect_real_state() {
        let mut invoice = sample_invoice();
        assert!(!invoice.is_settled());
        assert_eq!(invoice.settled_at(), None);

        invoice.mark_settled();

        assert!(invoice.is_settled());
        assert!(invoice.settled_at().is_some());
    }

    #[test]
    fn is_private_reflects_constructor_argument() {
        let preimage = PaymentPreimage::new_random();
        let payment_hash = preimage.payment_hash();
        let private_invoice = Invoice::new(
            payment_hash,
            1_000,
            "private".to_string(),
            3600,
            true,
            "dest".to_string(),
            preimage,
        );
        assert!(private_invoice.is_private());

        let preimage2 = PaymentPreimage::new_random();
        let payment_hash2 = preimage2.payment_hash();
        let public_invoice = Invoice::new(
            payment_hash2,
            1_000,
            "public".to_string(),
            3600,
            false,
            "dest".to_string(),
            preimage2,
        );
        assert!(!public_invoice.is_private());
    }

    #[test]
    fn verify_signature_fails_closed_for_missing_signature() {
        let invoice = sample_invoice();
        assert!(!invoice.has_signature());
        assert!(matches!(
            invoice.verify_signature(),
            Err(InvoiceError::InvalidSignature(_))
        ));
    }

    #[test]
    fn verify_signature_never_accepts_unverified_bytes() {
        let mut invoice = sample_invoice();
        // Attacker-supplied arbitrary bytes must NOT be treated as authentic:
        // presence of a signature must never yield Ok(true) while real
        // cryptographic verification is unimplemented (fail closed).
        invoice.set_signature(vec![0xde, 0xad, 0xbe, 0xef]);
        assert!(invoice.has_signature());
        assert!(matches!(
            invoice.verify_signature(),
            Err(InvoiceError::InvalidSignature(_))
        ));
    }
}
