// supernova Lightning Network - Wallet Implementation
//
// This file contains the wallet implementation for the Lightning Network,
// including key management, invoice handling, and payment processing.

use crate::crypto::quantum::{QuantumKeyPair, QuantumScheme};
use crate::lightning::channel::ChannelId;
use crate::lightning::invoice::{Invoice, InvoiceError};
use crate::lightning::payment::{PaymentHash, PaymentPreimage};

use rand::{thread_rng, Rng, RngCore};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::time::SystemTime;
use thiserror::Error;

/// Error types for Lightning wallet operations
#[derive(Debug, Error)]
pub enum WalletError {
    #[error("Key management error: {0}")]
    KeyError(String),

    #[error("Insufficient funds: {0}")]
    InsufficientFunds(String),

    #[error("Invoice error: {0}")]
    InvoiceError(#[from] InvoiceError),

    #[error("Payment error: {0}")]
    PaymentError(String),

    #[error("Channel error: {0}")]
    ChannelError(String),

    #[error("Database error: {0}")]
    DatabaseError(String),

    #[error("Cryptographic error: {0}")]
    CryptoError(String),
}

/// Key derivation scheme for Lightning wallet
#[derive(Debug, Clone)]
pub enum KeyDerivation {
    /// BIP32 derivation path
    Bip32(String),

    /// Quantum-resistant key derivation
    Quantum {
        scheme: QuantumScheme,
        seed: Vec<u8>,
    },
}

/// Key manager for Lightning wallet
pub struct KeyManager {
    /// Master seed
    master_seed: Vec<u8>,

    /// Key derivation scheme
    derivation_scheme: KeyDerivation,

    /// Node private key
    node_private_key: Vec<u8>,

    /// Cached derived keys
    cached_keys: HashMap<String, Vec<u8>>,

    /// Quantum key pairs if enabled
    quantum_keys: Option<HashMap<String, QuantumKeyPair>>,
}

impl KeyManager {
    /// Create a new key manager with the specified seed
    pub fn new(
        seed: Vec<u8>,
        use_quantum: bool,
        quantum_scheme: Option<QuantumScheme>,
    ) -> Result<Self, WalletError> {
        // Generate node private key from seed
        let mut hasher = Sha256::new();
        hasher.update(&seed);
        hasher.update(b"node_key");
        let result = hasher.finalize();

        let mut node_private_key = vec![0u8; 32];
        node_private_key.copy_from_slice(&result[..32]);

        // Create derivation scheme
        let derivation_scheme = if use_quantum {
            if let Some(scheme) = quantum_scheme {
                KeyDerivation::Quantum {
                    scheme,
                    seed: seed.clone(),
                }
            } else {
                return Err(WalletError::KeyError(
                    "Quantum derivation requested but no scheme provided".to_string(),
                ));
            }
        } else {
            // Default to BIP32 derivation
            KeyDerivation::Bip32("m/84'/0'/0'/0/0".to_string())
        };

        Ok(Self {
            master_seed: seed,
            derivation_scheme,
            node_private_key,
            cached_keys: HashMap::new(),
            quantum_keys: if use_quantum {
                Some(HashMap::new())
            } else {
                None
            },
        })
    }

    /// Get node private key
    pub fn node_private_key(&self) -> &[u8] {
        &self.node_private_key
    }

    /// Derive a new key for a specific purpose
    pub fn derive_key(&mut self, purpose: &str) -> Result<Vec<u8>, WalletError> {
        // Check cache first
        if let Some(key) = self.cached_keys.get(purpose) {
            return Ok(key.clone());
        }

        // Derive new key
        match &self.derivation_scheme {
            KeyDerivation::Bip32(_path) => {
                // In a real implementation, this would use BIP32 derivation
                // For now, we'll just hash the seed with the purpose
                let mut hasher = Sha256::new();
                hasher.update(&self.master_seed);
                hasher.update(purpose.as_bytes());
                let result = hasher.finalize();

                let mut derived_key = vec![0u8; 32];
                derived_key.copy_from_slice(&result[..32]);

                // Cache the key
                self.cached_keys
                    .insert(purpose.to_string(), derived_key.clone());

                Ok(derived_key)
            }
            KeyDerivation::Quantum { scheme, seed: _ } => {
                if let Some(quantum_keys) = &mut self.quantum_keys {
                    // For quantum keys, we need to generate a new key pair
                    // In a real implementation, this would derive from the seed deterministically
                    let _rng = thread_rng();

                    // Create QuantumParameters with scheme and security level
                    let quantum_params = crate::crypto::quantum::QuantumParameters {
                        scheme: *scheme,
                        security_level: 3, // Medium security level by default
                    };

                    let quantum_keypair =
                        QuantumKeyPair::generate(quantum_params).map_err(|e| {
                            WalletError::CryptoError(format!(
                                "Failed to generate quantum keypair: {:?}",
                                e
                            ))
                        })?;

                    // Store the key pair
                    quantum_keys.insert(purpose.to_string(), quantum_keypair.clone());

                    // Return the private key bytes directly from the secret_key field
                    Ok(quantum_keypair.secret_key.clone())
                } else {
                    Err(WalletError::KeyError(
                        "Quantum keys not initialized".to_string(),
                    ))
                }
            }
        }
    }

    /// Get a quantum key pair for a specific purpose
    pub fn get_quantum_keypair(&self, purpose: &str) -> Option<&QuantumKeyPair> {
        self.quantum_keys.as_ref()?.get(purpose)
    }
}

/// Payment status
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PaymentStatus {
    /// Payment is pending
    Pending,

    /// Payment is in progress
    InProgress,

    /// Payment succeeded
    Succeeded,

    /// Payment failed
    Failed(String),
}

/// Payment information
#[derive(Debug, Clone)]
pub struct Payment {
    /// Payment hash
    hash: PaymentHash,

    /// Payment amount in millinova
    amount_mnova: u64,

    /// Payment description
    description: String,

    /// Creation time
    creation_time: SystemTime,

    /// Payment status
    status: PaymentStatus,

    /// Payment preimage if payment succeeded
    preimage: Option<PaymentPreimage>,

    /// Associated channel
    channel_id: Option<ChannelId>,
}

/// Main Lightning wallet implementation
pub struct LightningWallet {
    /// Key manager
    key_manager: KeyManager,

    /// On-chain balance
    on_chain_balance: u64,

    /// Channel balances
    channel_balances: HashMap<ChannelId, u64>,

    /// Generated invoices
    invoices: HashMap<PaymentHash, Invoice>,

    /// Payments
    payments: HashMap<PaymentHash, Payment>,

    /// Payment preimages
    preimages: HashMap<PaymentHash, PaymentPreimage>,
}

impl LightningWallet {
    /// Create a new Lightning wallet
    pub fn new(
        seed: Vec<u8>,
        use_quantum: bool,
        quantum_scheme: Option<QuantumScheme>,
    ) -> Result<Self, WalletError> {
        let key_manager = KeyManager::new(seed, use_quantum, quantum_scheme)?;

        Ok(Self {
            key_manager,
            on_chain_balance: 0,
            channel_balances: HashMap::new(),
            invoices: HashMap::new(),
            payments: HashMap::new(),
            preimages: HashMap::new(),
        })
    }

    /// Create a test wallet with a specified balance (for testing only)
    pub fn new_test_wallet(balance: u64) -> Self {
        // Use a random seed for testing
        let mut rng = thread_rng();
        let mut seed = vec![0u8; 32];
        for byte in seed.iter_mut() {
            *byte = rng.gen();
        }

        let key_manager = KeyManager::new(seed, false, None)
            .expect("Failed to create key manager for test wallet");

        Self {
            key_manager,
            on_chain_balance: balance,
            channel_balances: HashMap::new(),
            invoices: HashMap::new(),
            payments: HashMap::new(),
            preimages: HashMap::new(),
        }
    }

    /// Create a Lightning wallet from an existing node wallet
    pub fn from_node_wallet(node_wallet: &impl WalletConversion) -> Result<Self, WalletError> {
        let seed = node_wallet.get_seed()?;
        let use_quantum = node_wallet.uses_quantum_keys();
        let quantum_scheme = node_wallet.get_quantum_scheme();

        Self::new(seed, use_quantum, quantum_scheme)
    }

    /// Get the total balance (on-chain + channels)
    pub fn get_balance(&self) -> u64 {
        let on_chain = self.on_chain_balance;
        let channels = self.channel_balances.values().sum::<u64>();
        on_chain + channels
    }

    /// Get the on-chain balance
    pub fn get_on_chain_balance(&self) -> u64 {
        self.on_chain_balance
    }

    /// Update the on-chain balance
    pub fn update_on_chain_balance(&mut self, balance: u64) {
        self.on_chain_balance = balance;
    }

    /// Update a channel balance
    pub fn update_channel_balance(&mut self, channel_id: ChannelId, balance: u64) {
        self.channel_balances.insert(channel_id, balance);
    }

    /// Create an invoice for receiving payment
    pub fn create_invoice(
        &mut self,
        amount_mnova: u64,
        description: &str,
        expiry_seconds: u32,
    ) -> Result<Invoice, WalletError> {
        // Generate a random preimage first (like in a real implementation)
        let mut rng = thread_rng();
        let mut preimage_bytes = [0u8; 32];
        rng.fill_bytes(&mut preimage_bytes);

        let preimage = PaymentPreimage::new(preimage_bytes);

        // Create invoice with preimage - payment hash will be derived automatically
        let invoice = Invoice::new_with_preimage(
            preimage,
            amount_mnova,
            description.to_string(),
            expiry_seconds,
        )?;

        let payment_hash = invoice.payment_hash();

        // Store the invoice and preimage
        self.invoices.insert(payment_hash, invoice.clone());
        self.preimages.insert(payment_hash, preimage);

        Ok(invoice)
    }

    /// Pay an invoice
    pub fn pay_invoice(&mut self, invoice: &Invoice) -> Result<PaymentPreimage, WalletError> {
        let payment_hash = invoice.payment_hash();
        let amount_mnova = invoice.amount_mnova();

        // Check if we have enough balance
        if self.on_chain_balance < amount_mnova / 1000 {
            return Err(WalletError::InsufficientFunds(format!(
                "Insufficient on-chain balance: {} < {}",
                self.on_chain_balance,
                amount_mnova / 1000
            )));
        }

        // Validate invoice
        if invoice.is_expired() {
            return Err(WalletError::InvoiceError(
                crate::lightning::invoice::InvoiceError::Expired,
            ));
        }

        // Check if we already have the preimage for this payment hash
        if let Some(existing_preimage) = self.preimages.get(&payment_hash) {
            return Ok(*existing_preimage);
        }

        // In a production Lightning Network implementation, this would:
        // 1. Find a route to the destination using the router
        // 2. Create HTLCs along the route with proper onion routing
        // 3. Send the payment and wait for the preimage
        // 4. Handle failures and retry with alternative routes

        // For this implementation, we'll simulate the payment process
        // by deriving the preimage from the invoice's stored preimage
        let preimage = invoice.payment_preimage();

        // Verify that the preimage matches the payment hash
        let computed_hash = preimage.payment_hash();
        if computed_hash != payment_hash {
            return Err(WalletError::PaymentError(
                "Invoice preimage does not match payment hash".to_string(),
            ));
        }

        // Create payment record
        let payment = Payment {
            hash: payment_hash,
            amount_mnova,
            description: invoice.description().to_string(),
            creation_time: SystemTime::now(),
            status: PaymentStatus::Succeeded,
            preimage: Some(preimage),
            channel_id: None,
        };

        // Update our balance (simulate payment sent)
        self.on_chain_balance -= amount_mnova / 1000;

        // Store the payment
        self.payments.insert(payment_hash, payment);

        // Store the preimage for future reference
        self.preimages.insert(payment_hash, preimage);

        Ok(preimage)
    }

    /// Get all invoices
    pub fn get_invoices(&self) -> Vec<&Invoice> {
        self.invoices.values().collect()
    }

    /// Get all payments
    pub fn get_payments(&self) -> Vec<&Payment> {
        self.payments.values().collect()
    }

    /// Get a specific invoice by payment hash
    pub fn get_invoice(&self, payment_hash: &PaymentHash) -> Option<&Invoice> {
        self.invoices.get(payment_hash)
    }

    /// Get a specific payment by payment hash
    pub fn get_payment(&self, payment_hash: &PaymentHash) -> Option<&Payment> {
        self.payments.get(payment_hash)
    }

    /// Check if a payment hash has been paid
    pub fn is_paid(&self, payment_hash: &PaymentHash) -> bool {
        if let Some(payment) = self.payments.get(payment_hash) {
            payment.status == PaymentStatus::Succeeded
        } else {
            false
        }
    }

    /// Get the preimage for a payment hash (if we have it)
    pub fn get_preimage(&self, payment_hash: &PaymentHash) -> Option<&PaymentPreimage> {
        self.preimages.get(payment_hash)
    }

    /// Check if we have an invoice for a payment hash
    pub fn has_invoice(&self, payment_hash: &[u8; 32]) -> bool {
        let hash = PaymentHash::new(*payment_hash);
        self.invoices.contains_key(&hash)
    }

    /// Mark an invoice as paid
    pub fn mark_invoice_paid(&mut self, payment_hash: &[u8; 32]) -> Result<(), WalletError> {
        let hash = PaymentHash::new(*payment_hash);

        if !self.invoices.contains_key(&hash) {
            return Err(WalletError::InvoiceError(InvoiceError::InvalidHash(
                "Invoice not found".to_string(),
            )));
        }

        // Create a payment record for the received payment
        // Safe: we just verified contains_key above
        let invoice = match self.invoices.get(&hash) {
            Some(inv) => inv,
            None => {
                return Err(WalletError::InvoiceError(InvoiceError::InvalidHash(
                    "Invoice not found".to_string(),
                )));
            }
        };
        let payment = Payment {
            hash,
            amount_mnova: invoice.amount_mnova(),
            description: invoice.description().to_string(),
            creation_time: SystemTime::now(),
            status: PaymentStatus::Succeeded,
            preimage: self.preimages.get(&hash).cloned(),
            channel_id: None,
        };

        // Update our balance (we received payment)
        self.on_chain_balance += invoice.amount_mnova() / 1000;

        // Store the payment
        self.payments.insert(hash, payment);

        Ok(())
    }

    /// Create a funding transaction for a channel
    pub fn create_funding_transaction(
        &self,
        amount: u64,
        _channel_id: &ChannelId,
    ) -> Result<crate::types::transaction::Transaction, WalletError> {
        // In a real implementation, this would create a proper funding transaction
        // For now, we'll create a placeholder transaction
        use crate::types::transaction::{Transaction, TransactionInput, TransactionOutput};

        let inputs = vec![TransactionInput::new(
            [0u8; 32],  // Previous transaction hash
            0,          // Output index
            vec![],     // Script signature
            0xffffffff, // Sequence
        )];

        let outputs = vec![TransactionOutput::new(
            amount,
            vec![], // Script pubkey (would be 2-of-2 multisig in real implementation)
        )];

        Ok(Transaction::new(
            2, // Version
            inputs, outputs, 0, // Lock time
        ))
    }
}

/// Trait for converting a node wallet to a Lightning wallet
pub trait WalletConversion {
    /// Get the wallet seed
    fn get_seed(&self) -> Result<Vec<u8>, WalletError>;

    /// Check if the wallet uses quantum keys
    fn uses_quantum_keys(&self) -> bool;

    /// Get the quantum scheme if applicable
    fn get_quantum_scheme(&self) -> Option<QuantumScheme>;
}
