//! Quantum-Safe Wallet Implementation
//!
//! This module provides a complete replacement for classical ECDSA-based wallets.
//! Every operation is designed to be quantum-resistant from the ground up.

use crate::crypto::quantum::{
    sign_quantum, QuantumError, QuantumKeyPair, QuantumParameters, QuantumScheme,
};
use crate::types::transaction::{SignatureSchemeType, Transaction, TransactionSignatureData};
use bip39::{Language, Mnemonic};
use rand::RngCore;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Quantum-safe HD wallet
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuantumWallet {
    /// Master quantum keypair
    master_keys: QuantumKeyPair,

    /// Derived quantum addresses
    addresses: HashMap<u32, QuantumAddress>,

    /// Current address index
    current_index: u32,

    /// Wallet metadata
    metadata: WalletMetadata,

    /// Hybrid mode - include classical keys during transition
    hybrid_mode: bool,

    /// Classical keys for hybrid mode (will be removed post-transition)
    #[serde(skip_serializing_if = "Option::is_none")]
    classical_keys: Option<ClassicalKeys>,
}

/// Quantum-safe address
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuantumAddress {
    /// Address index in HD derivation
    pub index: u32,

    /// Quantum public key
    pub quantum_pubkey: Vec<u8>,

    /// Human-readable address (bech32m encoded)
    pub address: String,

    /// Address type
    pub address_type: QuantumAddressType,

    /// Zero-knowledge proof of ownership
    pub ownership_proof: Option<Vec<u8>>,
}

/// Quantum address types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum QuantumAddressType {
    /// Pure quantum (post-quantum only)
    PureQuantum,
    /// Hybrid (quantum + classical)
    Hybrid,
    /// Stealth (with zero-knowledge proofs)
    Stealth,
    /// Threshold (multi-party quantum)
    Threshold(u8, u8), // (required, total)
}

/// Wallet metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WalletMetadata {
    /// Wallet name
    pub name: String,

    /// Creation timestamp
    pub created_at: u64,

    /// Quantum scheme used
    pub quantum_scheme: QuantumScheme,

    /// Security level
    pub security_level: u8,

    /// Network (mainnet/testnet)
    pub network: String,
}

/// Classical keys for hybrid mode transition
#[derive(Debug, Clone, Serialize, Deserialize)]
struct ClassicalKeys {
    /// BIP32 extended private key
    xprv: Vec<u8>,

    /// Derived classical addresses
    addresses: HashMap<u32, String>,
}

impl QuantumWallet {
    /// Create new quantum wallet from mnemonic
    pub fn from_mnemonic(
        mnemonic_str: &str,
        password: &str,
        network: &str,
        scheme: QuantumScheme,
        security_level: u8,
    ) -> Result<Self, WalletError> {
        // Parse mnemonic
        let mnemonic = Mnemonic::parse_in(Language::English, mnemonic_str)
            .map_err(|_| WalletError::InvalidMnemonic)?;

        // Generate seed with quantum-safe KDF
        let seed = Self::quantum_safe_seed_derivation(&mnemonic, password)?;

        // Generate master quantum keys
        let params = QuantumParameters {
            scheme,
            security_level,
        };
        let master_keys = QuantumKeyPair::from_seed(&seed, params)?;

        let metadata = WalletMetadata {
            name: "Quantum Wallet".to_string(),
            // `duration_since(UNIX_EPOCH)` only fails if the system clock
            // predates 1970 — fall back to 0 rather than failing wallet
            // construction; the timestamp is metadata, not key material.
            created_at: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or(std::time::Duration::ZERO)
                .as_secs(),
            quantum_scheme: scheme,
            security_level,
            network: network.to_string(),
        };

        Ok(Self {
            master_keys,
            addresses: HashMap::new(),
            current_index: 0,
            metadata,
            hybrid_mode: false,
            classical_keys: None,
        })
    }

    /// Quantum-safe seed derivation using Argon2
    fn quantum_safe_seed_derivation(
        mnemonic: &Mnemonic,
        password: &str,
    ) -> Result<[u8; 64], WalletError> {
        use argon2::{
            password_hash::{PasswordHasher, Salt},
            Argon2,
        };

        // Derive the seed from the mnemonic entropy AND the optional
        // passphrase, BIP39-style: the passphrase must contribute to the
        // resulting key material so that two different passphrases produce
        // two different seeds (and therefore different quantum keys). The
        // previous implementation discarded the passphrase entirely and
        // selected between two fixed salts, so every non-empty passphrase
        // yielded identical keys — defeating the passphrase protection the
        // API implies.
        //
        // Both inputs are secret and deterministic, so recovery from the
        // mnemonic + passphrase alone remains reproducible. The passphrase
        // is folded into the KDF *input* (the per-wallet secret is the
        // mnemonic entropy), while the salt is a fixed domain-separation
        // constant — this mirrors BIP39, where the salt is a deterministic
        // function of the passphrase rather than a random value.
        let mut kdf_input = mnemonic.to_entropy();
        kdf_input.extend_from_slice(password.as_bytes());

        // Fixed, deterministic salt for domain separation. A random salt
        // would break deterministic recovery; the secret entropy comes from
        // the mnemonic, not the salt. Salt strings are compile-time
        // constants and valid base64, so `from_b64` should not fail — but
        // propagate the error rather than panicking. A panic here on a
        // future invalid-salt edit would crash wallet construction with no
        // diagnostic; the typed error surfaces the misconfiguration to
        // callers.
        let salt = Salt::from_b64("quantumsupernova").map_err(|_| WalletError::SeedDerivationFailed)?;

        // Use Argon2id for quantum-resistant key derivation
        let argon2 = Argon2::default();
        let hash = argon2
            .hash_password(&kdf_input, salt)
            .map_err(|_| WalletError::SeedDerivationFailed)?;

        // Extract 64 bytes for seed
        let mut seed = [0u8; 64];
        // `PasswordHash::hash` is `Some` after a successful `hash_password`,
        // but the type is `Option` — propagate fail-loud rather than
        // unwrap, so a future Argon2 API change can't silently produce
        // an empty seed.
        let hash_binding = hash.hash.ok_or(WalletError::SeedDerivationFailed)?;
        let hash_bytes = hash_binding.as_bytes();
        seed[..hash_bytes.len().min(64)].copy_from_slice(&hash_bytes[..hash_bytes.len().min(64)]);

        Ok(seed)
    }

    /// Generate new quantum address
    pub fn new_address(&mut self) -> Result<QuantumAddress, WalletError> {
        let index = self.current_index;

        // Derive child quantum keys
        let child_keys = self.derive_quantum_keys(index)?;

        // Generate human-readable address
        let address =
            self.encode_quantum_address(&child_keys.public_key, QuantumAddressType::PureQuantum)?;

        let quantum_address = QuantumAddress {
            index,
            quantum_pubkey: child_keys.public_key.clone(),
            address: address.clone(),
            address_type: QuantumAddressType::PureQuantum,
            ownership_proof: None,
        };

        // Store address
        self.addresses.insert(index, quantum_address.clone());
        self.current_index += 1;

        Ok(quantum_address)
    }

    /// Generate stealth address with zero-knowledge proof
    pub fn new_stealth_address(&mut self) -> Result<QuantumAddress, WalletError> {
        let index = self.current_index;

        // Derive child keys
        let child_keys = self.derive_quantum_keys(index)?;

        // Generate zero-knowledge proof of ownership
        let zkp = self.generate_ownership_zkp(&child_keys)?;

        // Encode as stealth address
        let address =
            self.encode_quantum_address(&child_keys.public_key, QuantumAddressType::Stealth)?;

        let quantum_address = QuantumAddress {
            index,
            quantum_pubkey: child_keys.public_key.clone(),
            address,
            address_type: QuantumAddressType::Stealth,
            ownership_proof: Some(zkp),
        };

        self.addresses.insert(index, quantum_address.clone());
        self.current_index += 1;

        Ok(quantum_address)
    }

    /// Derive quantum keys for index
    fn derive_quantum_keys(&self, index: u32) -> Result<QuantumKeyPair, WalletError> {
        // Quantum-safe key derivation
        // Uses HKDF with SHA3-512 for child key generation
        use hkdf::Hkdf;
        use sha3::Sha3_512;

        let info = format!("quantum-hd/{}", index);
        let hkdf = Hkdf::<Sha3_512>::new(None, &self.master_keys.to_bytes());

        let mut okm = vec![0u8; 64];
        hkdf.expand(info.as_bytes(), &mut okm)
            .map_err(|_| WalletError::KeyDerivationFailed)?;

        // Generate child keys from derived material
        let params = QuantumParameters {
            scheme: self.metadata.quantum_scheme,
            security_level: self.metadata.security_level,
        };

        let mut seed = [0u8; 64];
        seed.copy_from_slice(&okm);

        QuantumKeyPair::from_seed(&seed, params).map_err(|_| WalletError::KeyDerivationFailed)
    }

    /// Encode quantum address (bech32m format)
    fn encode_quantum_address(
        &self,
        pubkey: &[u8],
        addr_type: QuantumAddressType,
    ) -> Result<String, WalletError> {
        use bech32::{self, ToBase32, Variant};

        // Create version byte based on address type
        let version = match addr_type {
            QuantumAddressType::PureQuantum => 0x10,
            QuantumAddressType::Hybrid => 0x11,
            QuantumAddressType::Stealth => 0x12,
            QuantumAddressType::Threshold(_, _) => 0x13,
        };

        // Create witness program
        let mut program = vec![version];

        // For quantum addresses, commit to the public key with SHA3-512
        // (Grover-resistant), taking the first 32 bytes. This mirrors the
        // consensus spend-binding commitment in
        // `types::transaction::pubkey_commitment` (SHA3-512(pubkey)[..32]);
        // the previous double-SHA256 (`hash256`) produced a classical hash
        // that could never match the SHA3-512 commitment presented at spend
        // time, making any output locked to such an address unspendable.
        let pubkey_hash = {
            use sha3::{Digest, Sha3_512};
            let mut hasher = Sha3_512::new();
            hasher.update(pubkey);
            hasher.finalize()[..32].to_vec()
        };
        program.extend_from_slice(&pubkey_hash);

        // Encode with bech32m
        let hrp = match self.metadata.network.as_str() {
            "mainnet" => "supernova",
            "testnet" => "tsupernova",
            _ => "supernova",
        };

        bech32::encode(hrp, program.to_base32(), Variant::Bech32m)
            .map_err(|_| WalletError::AddressEncodingFailed)
    }

    /// Generate an ownership proof for a stealth address.
    ///
    /// "Ownership" = knowledge of the secret key corresponding to the
    /// address's public key. We achieve that by signing a
    /// domain-separated ownership statement with `sign_quantum`; the
    /// signature is verifiable by anyone against the public key and
    /// cannot be produced without the secret. The previous
    /// implementation delegated to a stubbed `generate_zkp` that
    /// returned a SHA-256 hash of the statement — not a proof of
    /// anything — so this is a correctness fix, not just a rename.
    fn generate_ownership_zkp(&self, keys: &QuantumKeyPair) -> Result<Vec<u8>, WalletError> {
        const OWNERSHIP_STATEMENT: &[u8] = b"supernova:quantum-wallet-ownership:v1";
        sign_quantum(keys, OWNERSHIP_STATEMENT).map_err(|_| WalletError::ZkpGenerationFailed)
    }

    /// Sign transaction with quantum signatures
    pub fn sign_transaction(
        &self,
        tx: &mut Transaction,
        input_index: usize,
        address_index: u32,
    ) -> Result<(), WalletError> {
        // Get the quantum keys for this address
        let child_keys = self.derive_quantum_keys(address_index)?;

        // Create signature hash
        let sighash = self.transaction_signature_hash(tx, input_index)?;

        // Sign with quantum signature
        let quantum_sig = sign_quantum(&child_keys, &sighash)?;

        // If in hybrid mode, also create classical signature
        if self.hybrid_mode {
            // This would create a classical signature as well
            // Both signatures would be included in the witness
        }

        // Since we can't modify inputs directly, we need to set the signature data
        // on the transaction itself for quantum signatures
        tx.set_signature_data(TransactionSignatureData {
            scheme: SignatureSchemeType::Dilithium,
            security_level: self.metadata.security_level,
            data: quantum_sig,
            public_key: child_keys.public_key.clone(),
        });

        Ok(())
    }

    /// Create transaction signature hash
    fn transaction_signature_hash(
        &self,
        tx: &Transaction,
        input_index: usize,
    ) -> Result<Vec<u8>, WalletError> {
        // Quantum-safe signature hash using SHA3-512
        use sha3::{Digest, Sha3_512};

        let mut hasher = Sha3_512::new();

        // Hash transaction data
        hasher.update(tx.version().to_le_bytes());

        // Hash all outputs
        for output in tx.outputs() {
            hasher.update(output.value().to_le_bytes());
            hasher.update(output.script_pubkey());
        }

        // Hash the specific input being signed
        if let Some(input) = tx.inputs().get(input_index) {
            hasher.update(input.prev_tx_hash());
            hasher.update(input.prev_output_index().to_le_bytes());
        }

        Ok(hasher.finalize().to_vec())
    }

    /// Enable hybrid mode for transition period
    pub fn enable_hybrid_mode(
        &mut self,
        classical_mnemonic: Option<&str>,
    ) -> Result<(), WalletError> {
        self.hybrid_mode = true;

        // If classical mnemonic provided, derive classical keys
        if let Some(mnemonic) = classical_mnemonic {
            // This would derive classical BIP32 keys
            // Store them temporarily for hybrid signatures
        }

        Ok(())
    }

    /// Create multi-party quantum threshold address
    pub fn create_threshold_address(
        &mut self,
        participants: Vec<Vec<u8>>, // Other parties' quantum public keys
        required: u8,
    ) -> Result<QuantumAddress, WalletError> {
        if participants.is_empty() || required == 0 || required > participants.len() as u8 + 1 {
            return Err(WalletError::InvalidThresholdParams);
        }

        let index = self.current_index;
        let child_keys = self.derive_quantum_keys(index)?;

        // Create threshold script with all quantum public keys
        let mut all_pubkeys = vec![child_keys.public_key.clone()];
        all_pubkeys.extend(participants);

        // Sort pubkeys for deterministic address
        all_pubkeys.sort();

        // Create threshold address
        let address = self.encode_threshold_address(&all_pubkeys, required)?;

        let quantum_address = QuantumAddress {
            index,
            quantum_pubkey: child_keys.public_key.clone(),
            address,
            address_type: QuantumAddressType::Threshold(required, all_pubkeys.len() as u8),
            ownership_proof: None,
        };

        self.addresses.insert(index, quantum_address.clone());
        self.current_index += 1;

        Ok(quantum_address)
    }

    /// Encode threshold quantum address
    fn encode_threshold_address(
        &self,
        pubkeys: &[Vec<u8>],
        required: u8,
    ) -> Result<String, WalletError> {
        use sha3::{Digest, Sha3_512};

        // Create threshold script hash
        let mut hasher = Sha3_512::new();
        hasher.update([required]);
        hasher.update([pubkeys.len() as u8]);

        for pubkey in pubkeys {
            hasher.update(pubkey);
        }

        let script_hash = hasher.finalize();
        let addr_type = QuantumAddressType::Threshold(required, pubkeys.len() as u8);

        self.encode_quantum_address(&script_hash, addr_type)
    }

    /// Export wallet for backup (encrypted with quantum-safe encryption)
    pub fn export_encrypted(&self, password: &str) -> Result<String, WalletError> {
        // Serialize wallet
        let wallet_bytes =
            bincode::serialize(self).map_err(|_| WalletError::SerializationFailed)?;

        // Encrypt with quantum-safe algorithm (placeholder for Kyber/NTRU)
        let encrypted = self.quantum_encrypt(&wallet_bytes, password)?;

        // Encode as base64
        Ok(base64::encode(encrypted))
    }

    /// Quantum-safe encryption (placeholder)
    fn quantum_encrypt(&self, data: &[u8], password: &str) -> Result<Vec<u8>, WalletError> {
        // In production, use post-quantum KEM like Kyber
        // For now, use XChaCha20Poly1305 with Argon2 key derivation
        use chacha20poly1305::{
            aead::{Aead, KeyInit, OsRng},
            XChaCha20Poly1305, XNonce,
        };

        // Derive key from password
        let key = Self::derive_encryption_key(password)?;
        let cipher = XChaCha20Poly1305::new(&key.into());

        // Generate nonce
        let mut nonce_bytes = [0u8; 24];
        use rand::RngCore;
        OsRng.fill_bytes(&mut nonce_bytes);
        let nonce = XNonce::from_slice(&nonce_bytes);

        // Encrypt
        let ciphertext = cipher
            .encrypt(nonce, data)
            .map_err(|_| WalletError::EncryptionFailed)?;

        // Prepend nonce to ciphertext
        let mut result = nonce_bytes.to_vec();
        result.extend(ciphertext);

        Ok(result)
    }

    /// Derive encryption key from password
    fn derive_encryption_key(password: &str) -> Result<[u8; 32], WalletError> {
        use argon2::{
            password_hash::{PasswordHasher, Salt},
            Argon2,
        };

        // Compile-time-constant salt; propagate fail-loud rather than
        // panicking so a future invalid-salt edit produces a typed error.
        let salt =
            Salt::from_b64("quantumwalletencryption").map_err(|_| WalletError::KeyDerivationFailed)?;
        let argon2 = Argon2::default();

        let hash = argon2
            .hash_password(password.as_bytes(), salt)
            .map_err(|_| WalletError::KeyDerivationFailed)?;

        let mut key = [0u8; 32];
        // `PasswordHash::hash` is `Some` after success, but the type is
        // `Option` — propagate fail-loud so a future Argon2 API change
        // can't produce a silently-empty encryption key.
        let hash_binding = hash.hash.ok_or(WalletError::KeyDerivationFailed)?;
        let hash_bytes = hash_binding.as_bytes();
        key.copy_from_slice(&hash_bytes[..32]);

        Ok(key)
    }

    /// Generate a new 12-word mnemonic phrase
    pub fn generate_mnemonic() -> Result<String, WalletError> {
        let mut entropy = [0u8; 16];
        rand::thread_rng().fill_bytes(&mut entropy);
        let mnemonic =
            Mnemonic::from_entropy(&entropy).map_err(|_| WalletError::MnemonicGenerationFailed)?;
        Ok(mnemonic.to_string())
    }

    /// Import an encrypted wallet
    pub fn import_encrypted(encrypted_wallet: &str, password: &str) -> Result<Self, WalletError> {
        // Decode from base64
        let encrypted_bytes =
            base64::decode(encrypted_wallet).map_err(|_| WalletError::DecryptionFailed)?;

        // Decrypt wallet
        let wallet_bytes = Self::quantum_decrypt(&encrypted_bytes, password)?;

        // Deserialize wallet
        let wallet: QuantumWallet =
            bincode::deserialize(&wallet_bytes).map_err(|_| WalletError::DeserializationFailed)?;

        Ok(wallet)
    }

    /// Quantum-safe decryption (placeholder)
    fn quantum_decrypt(ciphertext: &[u8], password: &str) -> Result<Vec<u8>, WalletError> {
        // In production, use post-quantum KEM like Kyber
        // For now, use XChaCha20Poly1305 with Argon2 key derivation
        use chacha20poly1305::{
            aead::{Aead, KeyInit},
            XChaCha20Poly1305, XNonce,
        };

        if ciphertext.len() < 24 {
            return Err(WalletError::DecryptionFailed);
        }

        // Derive key from password
        let key = Self::derive_encryption_key(password)?;
        let cipher = XChaCha20Poly1305::new(&key.into());

        // Extract nonce and ciphertext
        let (nonce_bytes, ct) = ciphertext.split_at(24);
        let nonce = XNonce::from_slice(nonce_bytes);

        // Decrypt
        cipher
            .decrypt(nonce, ct)
            .map_err(|_| WalletError::DecryptionFailed)
    }
}

/// Wallet errors
#[derive(Debug, thiserror::Error)]
pub enum WalletError {
    #[error("Invalid mnemonic")]
    InvalidMnemonic,

    #[error("Seed derivation failed")]
    SeedDerivationFailed,

    #[error("Key derivation failed")]
    KeyDerivationFailed,

    #[error("Address encoding failed")]
    AddressEncodingFailed,

    #[error("ZKP generation failed")]
    ZkpGenerationFailed,

    #[error("Invalid threshold parameters")]
    InvalidThresholdParams,

    #[error("Serialization failed")]
    SerializationFailed,

    #[error("Deserialization failed")]
    DeserializationFailed,

    #[error("Encryption failed")]
    EncryptionFailed,

    #[error("Decryption failed")]
    DecryptionFailed,

    #[error("Mnemonic generation failed")]
    MnemonicGenerationFailed,

    #[error("Quantum error: {0}")]
    Quantum(#[from] QuantumError),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_quantum_wallet_creation() {
        let mnemonic = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";

        let wallet =
            QuantumWallet::from_mnemonic(mnemonic, "", "testnet", QuantumScheme::Dilithium, 3)
                .unwrap();

        assert_eq!(wallet.metadata.quantum_scheme, QuantumScheme::Dilithium);
        assert_eq!(wallet.metadata.security_level, 3);
        assert_eq!(wallet.current_index, 0);
    }

    #[test]
    fn test_quantum_address_generation() {
        let mnemonic = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";

        let mut wallet =
            QuantumWallet::from_mnemonic(mnemonic, "", "testnet", QuantumScheme::Dilithium, 3)
                .unwrap();

        let addr1 = wallet.new_address().unwrap();
        let addr2 = wallet.new_address().unwrap();

        assert_ne!(addr1.address, addr2.address);
        assert_eq!(addr1.index, 0);
        assert_eq!(addr2.index, 1);
        assert!(addr1.address.starts_with("tsupernova"));
    }

    #[test]
    fn test_stealth_address() {
        let mnemonic = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";

        let mut wallet =
            QuantumWallet::from_mnemonic(mnemonic, "", "mainnet", QuantumScheme::Dilithium, 3)
                .unwrap();

        let stealth_addr = wallet.new_stealth_address().unwrap();

        assert_eq!(stealth_addr.address_type, QuantumAddressType::Stealth);
        assert!(stealth_addr.ownership_proof.is_some());
        assert!(stealth_addr.address.starts_with("supernova"));
    }

    #[test]
    fn test_seed_derivation_depends_on_password() {
        let mnemonic_str = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";
        let mnemonic = Mnemonic::parse_in(Language::English, mnemonic_str).unwrap();

        let seed_empty = QuantumWallet::quantum_safe_seed_derivation(&mnemonic, "").unwrap();
        let seed_pw1 =
            QuantumWallet::quantum_safe_seed_derivation(&mnemonic, "correct horse").unwrap();
        let seed_pw2 =
            QuantumWallet::quantum_safe_seed_derivation(&mnemonic, "battery staple").unwrap();

        // A passphrase must actually change the derived seed.
        assert_ne!(
            seed_empty, seed_pw1,
            "empty and non-empty passphrases must derive different seeds"
        );
        // Two distinct passphrases must derive distinct seeds (the prior
        // implementation collapsed all non-empty passphrases to one seed).
        assert_ne!(
            seed_pw1, seed_pw2,
            "distinct passphrases must derive distinct seeds"
        );

        // Derivation must remain deterministic for recovery.
        let seed_pw1_again =
            QuantumWallet::quantum_safe_seed_derivation(&mnemonic, "correct horse").unwrap();
        assert_eq!(
            seed_pw1, seed_pw1_again,
            "same mnemonic + passphrase must derive the same seed"
        );
    }

    #[test]
    fn test_address_commits_to_sha3_512_not_double_sha256() {
        use bech32::FromBase32;

        let wallet = QuantumWallet::from_mnemonic(
            "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about",
            "",
            "testnet",
            QuantumScheme::Dilithium,
            3,
        )
        .unwrap();

        let pubkey: &[u8] = b"quantum-public-key-material-for-address-commitment";

        let address = wallet
            .encode_quantum_address(pubkey, QuantumAddressType::PureQuantum)
            .unwrap();

        // Decode the bech32m address back into its witness program.
        let (_hrp, data, _variant) = bech32::decode(&address).unwrap();
        let program = Vec::<u8>::from_base32(&data).unwrap();

        // program = [version_byte] ++ 32-byte pubkey commitment.
        assert_eq!(program.len(), 33, "program must be version byte + 32-byte hash");
        let commitment = &program[1..];

        // The committed hash must be SHA3-512(pubkey)[..32] — the same scheme
        // consensus spend-binding uses — so outputs locked to this address are
        // spendable. It must NOT be the classical double-SHA256 the code used
        // before this fix.
        let expected_sha3 = {
            use sha3::{Digest, Sha3_512};
            let mut hasher = Sha3_512::new();
            hasher.update(pubkey);
            hasher.finalize()[..32].to_vec()
        };
        assert_eq!(
            commitment, expected_sha3.as_slice(),
            "address must commit to SHA3-512(pubkey)[..32]"
        );

        let double_sha256 = crate::crypto::hash256(pubkey);
        assert_ne!(
            commitment,
            double_sha256.as_slice(),
            "address must not commit to classical double-SHA256"
        );
    }
}
