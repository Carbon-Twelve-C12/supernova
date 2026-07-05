use super::backup_warning::{BackupMetadata, BackupStatus, BackupWarning, SeedPhraseVerifier};
use super::password_strength::PasswordStrengthChecker;
use bip39::{Language, Mnemonic};
use bitcoin as btc_compat; // Bitcoin-compatible
use btc_compat::{
    bip32::{ChildNumber, DerivationPath, Xpriv},
    network::Network,
    secp256k1::Secp256k1,
    Address, PrivateKey,
};
use chrono::Utc;
use supernova_core::storage::utxo_set::UtxoSet;
use rand::rngs::OsRng;
use rand::RngCore;
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, path::PathBuf, str::FromStr};
use thiserror::Error;
use aes_gcm::{
    aead::{Aead, KeyInit, OsRng as AesOsRng},
    Aes256Gcm, Nonce as AesNonce,
};
use argon2::{Argon2, Algorithm, Version, Params, PasswordHasher};
use argon2::password_hash::{SaltString, PasswordHash};
use zeroize::Zeroizing;

/// Write wallet material to `path` with owner-only (0o600) permissions.
///
/// SECURITY (R5-97): The default `std::fs::write` honors the process umask,
/// which typically yields world-readable (0o644) files. Wallet files can
/// contain the BIP39 mnemonic or a cleartext WIF private key, so any local user
/// could read them. On Unix we create/truncate the file with mode 0o600 and
/// additionally reset the permissions in case the file pre-existed with looser
/// bits. On non-Unix platforms this falls back to a plain write.
fn write_wallet_file_secure(path: &std::path::Path, contents: &[u8]) -> std::io::Result<()> {
    use std::io::Write;
    #[cfg(unix)]
    {
        use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .mode(0o600)
            .open(path)?;
        // Reset perms explicitly: `.mode()` only applies when the file is newly
        // created, so an existing 0o644 wallet would otherwise keep loose bits.
        file.set_permissions(std::fs::Permissions::from_mode(0o600))?;
        file.write_all(contents)?;
        file.flush()?;
        Ok(())
    }
    #[cfg(not(unix))]
    {
        std::fs::write(path, contents)
    }
}

#[derive(Error, Debug)]
pub enum HDWalletError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
    #[error("Invalid mnemonic: {0}")]
    InvalidMnemonic(String),
    #[error("Account not found: {0}")]
    AccountNotFound(String),
    #[error("Address not found: {0}")]
    AddressNotFound(String),
    #[error("Compatibility error: {0}")]
    Compatibility(String),
    #[error("Address parsing error: {0}")]
    AddressParsing(String),
    #[error("Encryption error: {0}")]
    EncryptionError(String),
    #[error("Decryption error: {0}")]
    DecryptionError(String),
    #[error("Password too weak: {0}")]
    PasswordTooWeak(String),
    #[error("Backup verification failed: {0}")]
    BackupVerificationFailed(String),
    #[error("Key derivation error: {0}")]
    KeyDerivationError(String),
}
// SECURITY FIX (P2-008): Encrypted Wallet Backup Structure
// ============================================================================

/// Encrypted wallet backup format
/// 
/// SECURITY: Stores encrypted wallet data with salt for key derivation.
/// Format is JSON-serializable for portability while maintaining security.
#[derive(Debug, Serialize, Deserialize)]
struct EncryptedBackup {
    /// Salt for Argon2 key derivation (base64 encoded)
    salt: String,
    /// Random nonce for AES256-GCM (12 bytes, base64 encoded)
    /// SECURITY: Must be unique per encryption - never reuse with same key
    #[serde(default)]
    nonce: String,
    /// AES256-GCM encrypted wallet data
    ciphertext: Vec<u8>,
    /// Backup format version (2 = random nonce)
    #[serde(default = "default_version")]
    version: u32,
}

/// Default version for legacy backups without version field
fn default_version() -> u32 { 1 }

/// HD Wallet with secure mnemonic storage
///
/// SECURITY FIX (P1-002): Mnemonic now uses Zeroizing<String> wrapper
/// to ensure the master secret is securely erased from memory when dropped.
/// ZeroizeOnDrop ensures all sensitive data is cleared on struct destruction.
/// Note: Clone is implemented because Zeroizing<T> implements Clone when T: Clone.
/// Each clone gets its own zeroized copy that will be cleared on drop.
#[derive(Clone, Serialize, Deserialize)]
pub struct HDWallet {
    /// Master mnemonic - wrapped in Zeroizing for secure memory erasure
    #[serde(serialize_with = "serialize_zeroizing", deserialize_with = "deserialize_zeroizing")]
    mnemonic: Zeroizing<String>,
    network: Network,
    accounts: HashMap<String, HDAccount>,
    wallet_path: PathBuf,
    #[serde(default)]
    backup_metadata: BackupMetadata,
}

// SECURITY FIX (R3-61): Manual Debug impl that redacts the master mnemonic.
//
// The derived Debug would forward through `Zeroizing<String>` (whose own
// derived Debug prints the inner String) and leak the plaintext seed phrase
// into any `{:?}`/`dbg!`/log/panic output. This impl never prints the mnemonic.
impl std::fmt::Debug for HDWallet {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HDWallet")
            .field("mnemonic", &"<redacted>")
            .field("network", &self.network)
            .field("accounts", &self.accounts)
            .field("wallet_path", &self.wallet_path)
            .field("backup_metadata", &self.backup_metadata)
            .finish()
    }
}

/// Custom serialization for Zeroizing<String>
fn serialize_zeroizing<S>(value: &Zeroizing<String>, serializer: S) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    serializer.serialize_str(value.as_str())
}

/// Custom deserialization for Zeroizing<String>
fn deserialize_zeroizing<'de, D>(deserializer: D) -> Result<Zeroizing<String>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let s = String::deserialize(deserializer)?;
    Ok(Zeroizing::new(s))
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HDAccount {
    pub name: String,
    pub account_type: AccountType,
    pub addresses: Vec<HDAddress>,
    /// BIP44 account index (the `account'` level of m/44'/coin'/account').
    ///
    /// SECURITY (R3-60): Stored so that every address in this account can be
    /// deterministically re-derived from the wallet mnemonic.
    #[serde(default)]
    pub account_index: u32,
    /// Next unused BIP44 address index (the `index` level of the external chain).
    #[serde(default)]
    pub next_index: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HDAddress {
    pub address: String,
    pub is_used: bool,
    /// BIP44 address index used to derive this address from the mnemonic seed.
    ///
    /// SECURITY (R3-60): Persisted so the corresponding signing key can be
    /// re-derived on demand; without it, funds sent here would be unspendable.
    #[serde(default)]
    pub index: u32,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum AccountType {
    Legacy,
    SegWit,
    NativeSegWit,
}

impl HDWallet {
    pub fn new(network: Network, wallet_path: PathBuf) -> Result<Self, HDWalletError> {
        // Generate entropy for a 12-word mnemonic (128 bits = 16 bytes)
        // SECURITY FIX (P0-006): Use OsRng instead of thread_rng for cryptographic entropy
        let mut entropy = [0u8; 16];
        OsRng.fill_bytes(&mut entropy);

        // Create mnemonic from entropy
        let mnemonic = Mnemonic::from_entropy(&entropy)
            .map_err(|e| HDWalletError::InvalidMnemonic(e.to_string()))?;

        Ok(Self {
            mnemonic: Zeroizing::new(mnemonic.to_string()),
            network,
            accounts: HashMap::new(),
            wallet_path,
            backup_metadata: BackupMetadata::new(),
        })
    }

    pub fn from_mnemonic(
        mnemonic: &str,
        network: Network,
        wallet_path: PathBuf,
    ) -> Result<Self, HDWalletError> {
        Mnemonic::parse_in_normalized(Language::English, mnemonic)
            .map_err(|e| HDWalletError::InvalidMnemonic(e.to_string()))?;
        Ok(Self {
            mnemonic: Zeroizing::new(mnemonic.to_string()),
            network,
            accounts: HashMap::new(),
            wallet_path,
            backup_metadata: BackupMetadata::new(),
        })
    }

    /// Save wallet with encryption
    /// 
    /// SECURITY FIX (P2-008): Encrypts wallet backup with Argon2id + ChaCha20-Poly1305.
    /// Previous implementation stored mnemonic in plaintext JSON.
    ///
    /// # Security Design
    /// - Argon2id for password-based key derivation (resistant to GPU/ASIC attacks)
    /// - AES256-GCM for authenticated encryption
    /// - Random salt per wallet
    /// - Nonce for each encryption operation
    /// - Zeroization of sensitive material
    ///
    /// # Arguments
    /// * `password` - Password to encrypt the wallet
    ///
    /// # Returns
    /// * `Ok(())` - Wallet encrypted and saved
    /// * `Err(HDWalletError)` - Encryption or save failed
    pub fn save_encrypted(&self, password: &str) -> Result<(), HDWalletError> {
        // Step 0: Validate password strength (quantum-resistant requirement)
        let checker = PasswordStrengthChecker::new();
        if let Err(suggestions) = checker.validate(password) {
            return Err(HDWalletError::PasswordTooWeak(
                format!(
                    "Password does not meet quantum-resistant requirements. Suggestions: {}",
                    suggestions.join("; ")
                )
            ));
        }

        // Step 1: Serialize wallet data
        // SECURITY FIX (R5-96): The pretty-printed JSON contains the plaintext
        // mnemonic; wrap it in Zeroizing so the buffer is wiped on drop (including
        // on every early-error return below) instead of lingering in freed heap.
        let json = Zeroizing::new(serde_json::to_string_pretty(self)?);
        let plaintext = json.as_bytes();
        
        // Step 2: Generate salt for key derivation
        let salt = SaltString::generate(&mut AesOsRng);
        
        // Step 3: Derive encryption key using Argon2id
        let argon2 = Argon2::new(
            Algorithm::Argon2id,
            Version::V0x13,
            Params::new(65536, 3, 4, None)
                .map_err(|e| HDWalletError::KeyDerivationError(e.to_string()))?,
        );
        
        let password_hash = argon2
            .hash_password(password.as_bytes(), &salt)
            .map_err(|e| HDWalletError::KeyDerivationError(e.to_string()))?;
        
        let hash_output = password_hash.hash
            .ok_or_else(|| HDWalletError::KeyDerivationError("No hash produced".to_string()))?;
        
        // SECURITY FIX (R5-96): Hold the derived AES key in Zeroizing so it is
        // wiped on drop along ALL paths, including the early-error returns from
        // cipher init / encryption / serialization below (the previous manual
        // key.zeroize() only ran on the success path).
        let mut key = Zeroizing::new([0u8; 32]);
        key.copy_from_slice(&hash_output.as_bytes()[..32]);

        // Step 4: Create cipher and encrypt with random nonce
        let cipher = Aes256Gcm::new_from_slice(key.as_slice())
            .map_err(|e| HDWalletError::EncryptionError(e.to_string()))?;

        // SECURITY: Generate cryptographically random 12-byte nonce
        // AES-GCM requires unique nonce per encryption - reuse breaks security
        let mut nonce_bytes = [0u8; 12];
        OsRng.fill_bytes(&mut nonce_bytes);
        let nonce = AesNonce::from_slice(&nonce_bytes);

        let ciphertext = cipher
            .encrypt(nonce, plaintext)
            .map_err(|e| HDWalletError::EncryptionError(e.to_string()))?;

        // Step 5: Package encrypted data with salt and nonce
        use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
        let encrypted_backup = EncryptedBackup {
            salt: salt.to_string(),
            nonce: BASE64.encode(nonce_bytes),
            ciphertext,
            version: 2,  // Version 2: random nonce
        };
        
        // Step 6: Write encrypted backup to disk (owner-only perms, R5-97)
        let backup_json = serde_json::to_string(&encrypted_backup)?;
        write_wallet_file_secure(&self.wallet_path, backup_json.as_bytes())?;

        // `key` (Zeroizing) and `json` (Zeroizing) are wiped automatically on drop.
        Ok(())
    }

    /// Load wallet from encrypted backup
    /// 
    /// SECURITY: Decrypts wallet backup using password-derived key.
    ///
    /// # Arguments
    /// * `wallet_path` - Path to encrypted wallet file
    /// * `password` - Password to decrypt
    ///
    /// # Returns
    /// * `Ok(HDWallet)` - Decrypted wallet
    /// * `Err(HDWalletError)` - Decryption failed (wrong password or corrupted file)
    pub fn load_encrypted(wallet_path: PathBuf, password: &str) -> Result<Self, HDWalletError> {
        // Step 1: Read encrypted backup
        let backup_json = std::fs::read_to_string(&wallet_path)?;
        let encrypted_backup: EncryptedBackup = serde_json::from_str(&backup_json)?;
        
        // Step 2: Derive decryption key from password
        let salt = SaltString::from_b64(&encrypted_backup.salt)
            .map_err(|e| HDWalletError::KeyDerivationError(e.to_string()))?;
        
        let argon2 = Argon2::new(
            Algorithm::Argon2id,
            Version::V0x13,
            Params::new(65536, 3, 4, None)
                .map_err(|e| HDWalletError::KeyDerivationError(e.to_string()))?,
        );
        
        let password_hash = argon2
            .hash_password(password.as_bytes(), &salt)
            .map_err(|e| HDWalletError::KeyDerivationError(e.to_string()))?;
        
        let hash_output = password_hash.hash
            .ok_or_else(|| HDWalletError::KeyDerivationError("No hash produced".to_string()))?;
        
        // SECURITY FIX (R5-96): Zeroizing wraps the derived AES key so it is wiped
        // on drop along every path, including the early-error returns below (the
        // previous manual key.zeroize() only ran on the success path).
        let mut key = Zeroizing::new([0u8; 32]);
        key.copy_from_slice(&hash_output.as_bytes()[..32]);

        // Step 3: Decrypt using stored nonce
        let cipher = Aes256Gcm::new_from_slice(key.as_slice())
            .map_err(|e| HDWalletError::DecryptionError(e.to_string()))?;

        // SECURITY: Decode nonce from backup (version 2+)
        // For backwards compatibility with version 1 (hardcoded nonce)
        use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
        let nonce_bytes: Vec<u8> = if encrypted_backup.version >= 2 {
            BASE64.decode(&encrypted_backup.nonce)
                .map_err(|e| HDWalletError::DecryptionError(
                    format!("Invalid nonce encoding: {}", e)
                ))?
        } else {
            // Legacy version 1 compatibility (deprecated hardcoded nonce)
            b"unique nonce".to_vec()
        };
        // SECURITY (R5-91): The nonce comes from an untrusted backup file. A
        // truncated, oversized, or missing (`#[serde(default)]` -> empty) nonce
        // field must not reach `AesNonce::from_slice`, which panics unless the
        // slice is exactly 12 bytes. Validate the length and return a
        // DecryptionError instead of crashing the wallet.
        let nonce_arr: [u8; 12] = <[u8; 12]>::try_from(nonce_bytes.as_slice())
            .map_err(|_| HDWalletError::DecryptionError(
                "Invalid nonce length - corrupted backup".to_string()
            ))?;
        let nonce = AesNonce::from(nonce_arr);

        let plaintext = cipher
            .decrypt(&nonce, encrypted_backup.ciphertext.as_ref())
            .map_err(|_| HDWalletError::DecryptionError(
                "Decryption failed - wrong password or corrupted backup".to_string()
            ))?;
        
        // Step 4: Deserialize wallet
        // SECURITY FIX (R5-96): The decrypted `plaintext` buffer contains the
        // plaintext mnemonic. `String::from_utf8` reuses that same allocation, so
        // wrapping the resulting JSON in Zeroizing wipes the master-secret bytes
        // on drop (including the serde-error early return) rather than leaking
        // them into freed heap.
        let json = Zeroizing::new(
            String::from_utf8(plaintext)
                .map_err(|e| HDWalletError::DecryptionError(e.to_string()))?,
        );
        let wallet: Self = serde_json::from_str(&json)?;

        // `key` (Zeroizing) and `json` (Zeroizing) are wiped automatically on drop.
        Ok(wallet)
    }

    /// Legacy plaintext save (DEPRECATED - use save_encrypted)
    ///
    /// SECURITY WARNING: This method stores the wallet in plaintext.
    /// Use save_encrypted() for production deployments.
    ///
    /// SECURITY FIX (R5-90): Never silently downgrade an encrypted wallet to
    /// plaintext. `create_account`/`get_new_address`/`verify_backup` call this
    /// method internally to persist mutations; if the wallet on disk was written
    /// by `save_encrypted()`, blindly rewriting it as plaintext JSON would (a)
    /// leak the BIP39 mnemonic in cleartext and (b) destroy the encrypted copy.
    /// If the target file already holds an `EncryptedBackup`, refuse the write
    /// and require the caller to persist via `save_encrypted()` instead. Fresh
    /// wallets and existing plaintext wallets are unaffected.
    #[deprecated(since = "1.0.0", note = "Use save_encrypted() instead for security")]
    pub fn save(&self) -> Result<(), HDWalletError> {
        if let Ok(existing) = std::fs::read_to_string(&self.wallet_path) {
            if serde_json::from_str::<EncryptedBackup>(&existing).is_ok() {
                return Err(HDWalletError::EncryptionError(
                    "refusing to overwrite an encrypted wallet with a plaintext save; \
                     use save_encrypted() to persist an encrypted wallet"
                        .to_string(),
                ));
            }
        }
        let json = serde_json::to_string_pretty(self)?;
        // Owner-only perms (R5-97): plaintext save contains the mnemonic.
        write_wallet_file_secure(&self.wallet_path, json.as_bytes())?;
        Ok(())
    }

    /// Legacy plaintext load (DEPRECATED - use load_encrypted)
    /// 
    /// SECURITY WARNING: This method loads plaintext wallets.
    /// Use load_encrypted() for production deployments.
    #[deprecated(since = "1.0.0", note = "Use load_encrypted() instead for security")]
    pub fn load(wallet_path: PathBuf) -> Result<Self, HDWalletError> {
        let json = std::fs::read_to_string(&wallet_path)?;
        let wallet: Self = serde_json::from_str(&json)?;
        Ok(wallet)
    }

    pub fn create_account(
        &mut self,
        name: String,
        account_type: AccountType,
    ) -> Result<(), HDWalletError> {
        // Assign a stable, collision-free BIP44 account index so every address
        // in this account is deterministically re-derivable from the mnemonic.
        let account_index = self
            .accounts
            .values()
            .map(|a| a.account_index)
            .max()
            .map(|m| m + 1)
            .unwrap_or(0);

        let account = HDAccount {
            name: name.clone(),
            account_type,
            addresses: Vec::new(),
            account_index,
            next_index: 0,
        };

        self.accounts.insert(name, account);
        self.save()?;
        Ok(())
    }

    /// BIP44 coin type for the wallet's network (0' = mainnet, 1' = test networks).
    fn coin_type(&self) -> u32 {
        match self.network {
            Network::Bitcoin => 0,
            _ => 1,
        }
    }

    /// Derive the secp256k1 private key for a BIP44 external-chain address.
    ///
    /// SECURITY FIX (R3-60): Keys are now derived deterministically from the
    /// wallet mnemonic along the path `m/44'/coin'/account'/0/index` rather than
    /// generated randomly and discarded. This makes every address recoverable
    /// from the seed and its signing key re-derivable on demand, so funds sent
    /// to a generated address are spendable.
    fn derive_external_private_key(
        &self,
        account_index: u32,
        address_index: u32,
    ) -> Result<PrivateKey, HDWalletError> {
        let mnemonic = Mnemonic::parse_in_normalized(Language::English, self.mnemonic.as_str())
            .map_err(|e| HDWalletError::InvalidMnemonic(e.to_string()))?;
        // SECURITY FIX (R5-96): The 64-byte BIP39 master seed is a top-level
        // secret; hold it in Zeroizing so it is wiped on drop instead of being
        // left in freed stack/heap memory after derivation.
        let seed = Zeroizing::new(mnemonic.to_seed(""));

        let secp = Secp256k1::new();
        let master = Xpriv::new_master(self.network, &seed[..])
            .map_err(|e| HDWalletError::KeyDerivationError(e.to_string()))?;

        let path: DerivationPath = vec![
            ChildNumber::from_hardened_idx(44)
                .map_err(|e| HDWalletError::KeyDerivationError(e.to_string()))?,
            ChildNumber::from_hardened_idx(self.coin_type())
                .map_err(|e| HDWalletError::KeyDerivationError(e.to_string()))?,
            ChildNumber::from_hardened_idx(account_index)
                .map_err(|e| HDWalletError::KeyDerivationError(e.to_string()))?,
            // External chain (receiving addresses)
            ChildNumber::from_normal_idx(0)
                .map_err(|e| HDWalletError::KeyDerivationError(e.to_string()))?,
            ChildNumber::from_normal_idx(address_index)
                .map_err(|e| HDWalletError::KeyDerivationError(e.to_string()))?,
        ]
        .into();

        let child = master
            .derive_priv(&secp, &path)
            .map_err(|e| HDWalletError::KeyDerivationError(e.to_string()))?;

        Ok(PrivateKey::new(child.private_key, self.network))
    }

    /// Re-derive the signing key for a previously generated address.
    ///
    /// SECURITY (R3-60): Enables spending funds sent to addresses produced by
    /// [`get_new_address`]. The key is deterministically reconstructed from the
    /// mnemonic seed; it is never stored on disk.
    pub fn derive_address_private_key(
        &self,
        account_name: &str,
        address_index: u32,
    ) -> Result<PrivateKey, HDWalletError> {
        let account = self
            .accounts
            .get(account_name)
            .ok_or_else(|| HDWalletError::AccountNotFound(account_name.to_string()))?;
        self.derive_external_private_key(account.account_index, address_index)
    }

    fn address_for_pubkey(
        &self,
        account_type: AccountType,
        public_key: &btc_compat::PublicKey,
    ) -> Result<Address, HDWalletError> {
        Ok(match account_type {
            AccountType::Legacy => Address::p2pkh(public_key, self.network),
            AccountType::SegWit => Address::p2shwpkh(public_key, self.network)
                .map_err(|e| HDWalletError::Compatibility(e.to_string()))?,
            AccountType::NativeSegWit => Address::p2wpkh(public_key, self.network)
                .map_err(|e| HDWalletError::Compatibility(e.to_string()))?,
        })
    }

    pub fn get_new_address(&mut self, account_name: &str) -> Result<HDAddress, HDWalletError> {
        // Read the account's derivation metadata without holding a mutable
        // borrow of `self` across the (immutable) derivation call below.
        let (account_index, account_type, address_index) = {
            let account = self
                .accounts
                .get(account_name)
                .ok_or_else(|| HDWalletError::AccountNotFound(account_name.to_string()))?;
            (account.account_index, account.account_type, account.next_index)
        };

        // SECURITY FIX (R3-60): Deterministic BIP44 derivation from the mnemonic
        // seed instead of a random, discarded key.
        let secp = Secp256k1::new();
        let private_key = self.derive_external_private_key(account_index, address_index)?;
        let public_key = private_key.public_key(&secp);
        let address = self.address_for_pubkey(account_type, &public_key)?;

        let hd_address = HDAddress {
            address: address.to_string(),
            is_used: false,
            index: address_index,
        };

        let account = self
            .accounts
            .get_mut(account_name)
            .ok_or_else(|| HDWalletError::AccountNotFound(account_name.to_string()))?;
        account.addresses.push(hd_address.clone());
        account.next_index = address_index + 1;
        self.save()?;
        Ok(hd_address)
    }

    pub fn get_balance(
        &self,
        account_name: &str,
        utxo_set: &UtxoSet,
    ) -> Result<u64, HDWalletError> {
        let account = self
            .accounts
            .get(account_name)
            .ok_or_else(|| HDWalletError::AccountNotFound(account_name.to_string()))?;

        let mut balance = 0;
        for hd_address in &account.addresses {
            let address = Address::from_str(&hd_address.address)
                .map_err(|e| HDWalletError::AddressParsing(e.to_string()))?;
            let address = address.assume_checked();
            balance += utxo_set.get_balance(address.script_pubkey().as_bytes());
        }

        Ok(balance)
    }

    pub fn get_total_balance(&self, utxo_set: &UtxoSet) -> Result<u64, HDWalletError> {
        let mut total = 0;
        for account_name in self.accounts.keys() {
            total += self.get_balance(account_name, utxo_set)?;
        }
        Ok(total)
    }

    pub fn list_accounts(&self) -> Vec<(u32, &HDAccount)> {
        self.accounts
            .iter()
            .enumerate()
            .map(|(i, (_, account))| (i as u32, account))
            .collect()
    }

    pub fn get_address_count(&self) -> usize {
        self.accounts
            .values()
            .map(|account| account.addresses.len())
            .sum()
    }

    /// Get mnemonic as string reference
    ///
    /// SECURITY NOTE: This returns a reference to the zeroizing mnemonic.
    /// The caller should not store this reference beyond its immediate use.
    pub fn get_mnemonic(&self) -> &str {
        self.mnemonic.as_str()
    }

    /// Get backup status
    pub fn backup_status(&self) -> BackupStatus {
        self.backup_metadata.status
    }

    /// Get backup metadata
    pub fn backup_metadata(&self) -> &BackupMetadata {
        &self.backup_metadata
    }

    /// Acknowledge backup (user has seen seed phrase)
    pub fn acknowledge_backup(&mut self) {
        self.backup_metadata.acknowledge();
    }

    /// Verify backup (user has verified seed phrase)
    pub fn verify_backup(&mut self, skip_check: bool) -> Result<(), HDWalletError> {
        if skip_check {
            self.backup_metadata.verify();
            return Ok(());
        }

        SeedPhraseVerifier::verify_interactive(&self.mnemonic, false)
            .map_err(|e| HDWalletError::BackupVerificationFailed(e))?;
        
        self.backup_metadata.verify();
        self.save()?;
        Ok(())
    }

    /// Check if backup reminder is needed
    pub fn needs_backup_reminder(&self) -> bool {
        self.backup_metadata.needs_reminder()
    }

    /// Check if backup is overdue
    pub fn is_backup_overdue(&self) -> bool {
        self.backup_metadata.is_overdue()
    }

    /// Display backup warning if needed
    pub fn check_and_display_backup_warning(&self) {
        if self.backup_metadata.status == BackupStatus::Verified {
            return;
        }

        if self.backup_metadata.is_overdue() {
            let days = Utc::now()
                .signed_duration_since(self.backup_metadata.created_at)
                .num_days();
            BackupWarning::display_overdue_warning(days);
        } else if self.backup_metadata.needs_reminder() {
            let days = Utc::now()
                .signed_duration_since(self.backup_metadata.created_at)
                .num_days();
            BackupWarning::display_reminder(days);
        }
    }
}

impl HDAccount {
    pub fn add_address(&mut self, address: HDAddress) {
        self.addresses.push(address);
    }
}

impl HDAddress {
    pub fn get_address(&self) -> &str {
        &self.address
    }
}

impl std::str::FromStr for AccountType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "legacy" => Ok(AccountType::Legacy),
            "segwit" => Ok(AccountType::SegWit),
            "native_segwit" => Ok(AccountType::NativeSegWit),
            _ => Err(format!("Invalid account type: {}", s)),
        }
    }
}

#[cfg(test)]
mod hd_derivation_tests {
    use super::*;

    // Standard BIP39 all-zero-entropy test vector.
    const TEST_MNEMONIC: &str =
        "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";

    /// R3-60: addresses must be a deterministic function of the mnemonic, not
    /// random. Two independent wallets built from the same seed must produce the
    /// same address sequence.
    #[test]
    fn get_new_address_is_deterministic_from_mnemonic() {
        let dir1 = tempfile::tempdir().unwrap();
        let dir2 = tempfile::tempdir().unwrap();
        let mut w1 =
            HDWallet::from_mnemonic(TEST_MNEMONIC, Network::Testnet, dir1.path().join("w.json"))
                .unwrap();
        let mut w2 =
            HDWallet::from_mnemonic(TEST_MNEMONIC, Network::Testnet, dir2.path().join("w.json"))
                .unwrap();
        w1.create_account("acct".to_string(), AccountType::NativeSegWit)
            .unwrap();
        w2.create_account("acct".to_string(), AccountType::NativeSegWit)
            .unwrap();

        let a1 = w1.get_new_address("acct").unwrap();
        let b1 = w1.get_new_address("acct").unwrap();
        let a2 = w2.get_new_address("acct").unwrap();
        let b2 = w2.get_new_address("acct").unwrap();

        // Same seed => identical address sequence (proves seed derivation).
        assert_eq!(a1.address, a2.address);
        assert_eq!(b1.address, b2.address);
        // Successive indices are distinct and monotonically tracked.
        assert_ne!(a1.address, b1.address);
        assert_eq!(a1.index, 0);
        assert_eq!(b1.index, 1);
    }

    /// R3-60: the signing key for a generated address must be re-derivable from
    /// the seed and must map back to that exact address, otherwise funds sent to
    /// it would be unspendable.
    #[test]
    fn address_signing_key_is_recoverable_from_seed() {
        let dir = tempfile::tempdir().unwrap();
        let mut w =
            HDWallet::from_mnemonic(TEST_MNEMONIC, Network::Testnet, dir.path().join("w.json"))
                .unwrap();
        w.create_account("acct".to_string(), AccountType::NativeSegWit)
            .unwrap();
        let addr = w.get_new_address("acct").unwrap();

        let priv_key = w.derive_address_private_key("acct", addr.index).unwrap();
        let secp = Secp256k1::new();
        let public_key = priv_key.public_key(&secp);
        let rederived = Address::p2wpkh(&public_key, Network::Testnet).unwrap();

        assert_eq!(rederived.to_string(), addr.address);
    }

    /// R5-90: internal persistence must never silently downgrade an encrypted
    /// wallet to plaintext. After `save_encrypted()`, an address-generating call
    /// (which persists via the deprecated plaintext `save()`) must NOT rewrite
    /// the on-disk file as cleartext JSON containing the mnemonic — the
    /// encrypted file must remain byte-for-byte intact.
    #[test]
    fn encrypted_wallet_is_not_downgraded_to_plaintext() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("wallet.json");
        let mut w =
            HDWallet::from_mnemonic(TEST_MNEMONIC, Network::Testnet, path.clone()).unwrap();

        // Persist as an encrypted backup (quantum-resistant password required).
        let password = "Xq9!vTp#Lm7$Rw4&ZkBnHjCdFgVs";
        w.save_encrypted(password).unwrap();

        let before = std::fs::read_to_string(&path).unwrap();
        // Sanity: the on-disk file is an EncryptedBackup and leaks no seed word.
        assert!(serde_json::from_str::<EncryptedBackup>(&before).is_ok());
        for word in TEST_MNEMONIC.split_whitespace() {
            assert!(!before.contains(word), "encrypted file leaked seed word: {word}");
        }

        // Address generation persists via the deprecated plaintext save(), which
        // must now refuse to clobber the encrypted file rather than leak the seed.
        w.create_account("acct".to_string(), AccountType::NativeSegWit)
            .unwrap_err();

        let after = std::fs::read_to_string(&path).unwrap();
        assert_eq!(before, after, "encrypted wallet file was rewritten");
        for word in TEST_MNEMONIC.split_whitespace() {
            assert!(
                !after.contains(word),
                "encrypted wallet silently downgraded to plaintext, leaked: {word}"
            );
        }

        // The encrypted wallet is still decryptable and intact.
        let reloaded = HDWallet::load_encrypted(path, password).unwrap();
        assert_eq!(reloaded.get_mnemonic(), TEST_MNEMONIC);
    }

    /// SECURITY (R5-97): Wallet files must be created owner-only (0o600) so a
    /// local unprivileged user cannot read the mnemonic / private key. This
    /// covers both the encrypted backup and the deprecated plaintext save.
    #[cfg(unix)]
    #[test]
    fn wallet_files_are_written_owner_only() {
        use std::os::unix::fs::PermissionsExt;

        // Encrypted backup path.
        let dir = tempfile::tempdir().unwrap();
        let enc_path = dir.path().join("wallet.enc");
        let w = HDWallet::from_mnemonic(TEST_MNEMONIC, Network::Testnet, enc_path.clone())
            .unwrap();
        w.save_encrypted("Xq9!vTp#Lm7$Rw4&ZkBnHjCdFgVs").unwrap();
        let mode = std::fs::metadata(&enc_path).unwrap().permissions().mode();
        assert_eq!(mode & 0o777, 0o600, "encrypted wallet perms = {:o}", mode & 0o777);

        // Deprecated plaintext save path (fresh plaintext wallet).
        let plain_path = dir.path().join("wallet.json");
        let w2 = HDWallet::from_mnemonic(TEST_MNEMONIC, Network::Testnet, plain_path.clone())
            .unwrap();
        #[allow(deprecated)]
        w2.save().unwrap();
        let mode = std::fs::metadata(&plain_path).unwrap().permissions().mode();
        assert_eq!(mode & 0o777, 0o600, "plaintext wallet perms = {:o}", mode & 0o777);

        // A pre-existing file with loose perms must be tightened on rewrite.
        let loose_path = dir.path().join("loose.json");
        std::fs::write(&loose_path, b"{}").unwrap();
        std::fs::set_permissions(&loose_path, std::fs::Permissions::from_mode(0o644)).unwrap();
        let w3 = HDWallet::from_mnemonic(TEST_MNEMONIC, Network::Testnet, loose_path.clone())
            .unwrap();
        #[allow(deprecated)]
        w3.save().unwrap();
        let mode = std::fs::metadata(&loose_path).unwrap().permissions().mode();
        assert_eq!(mode & 0o777, 0o600, "rewritten wallet perms = {:o}", mode & 0o777);
    }

    /// A plaintext wallet (or a brand-new one) must still persist normally via
    /// the deprecated save() path — the R5-90 guard only blocks the encrypted
    /// -> plaintext downgrade.
    #[test]
    fn plaintext_save_still_works_for_plaintext_wallet() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("wallet.json");
        let mut w =
            HDWallet::from_mnemonic(TEST_MNEMONIC, Network::Testnet, path.clone()).unwrap();

        // Fresh file: create_account persists via save() without error.
        w.create_account("acct".to_string(), AccountType::NativeSegWit)
            .unwrap();
        // Existing plaintext file: generating an address persists again cleanly.
        let addr = w.get_new_address("acct").unwrap();
        assert_eq!(addr.index, 0);

        // The on-disk file is a plaintext HDWallet (not an EncryptedBackup) and
        // reflects the persisted address.
        let json = std::fs::read_to_string(&path).unwrap();
        assert!(serde_json::from_str::<EncryptedBackup>(&json).is_err());
        let reloaded: HDWallet = serde_json::from_str(&json).unwrap();
        assert_eq!(reloaded.get_address_count(), 1);
    }

    /// R5-96: wrapping the transient plaintext JSON, the derived AES key, and the
    /// decrypted buffer in `Zeroizing` (for defence-in-depth memory hygiene) must
    /// not alter the encryption/decryption behaviour: a `save_encrypted` ->
    /// `load_encrypted` roundtrip must still faithfully recover the wallet.
    #[test]
    fn zeroizing_wrappers_preserve_encrypted_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("wallet.json");
        let mut w =
            HDWallet::from_mnemonic(TEST_MNEMONIC, Network::Testnet, path.clone()).unwrap();
        w.create_account("acct".to_string(), AccountType::NativeSegWit)
            .unwrap();

        let password = "Xq9!vTp#Lm7$Rw4&ZkBnHjCdFgVs";
        w.save_encrypted(password).unwrap();

        let reloaded = HDWallet::load_encrypted(path, password).unwrap();
        assert_eq!(reloaded.get_mnemonic(), TEST_MNEMONIC);
        assert!(reloaded.accounts.contains_key("acct"));

        // The seed-derivation path (also now Zeroizing the 64-byte BIP39 seed)
        // still produces the same signing key before and after the roundtrip.
        let k1 = w.derive_address_private_key("acct", 0).unwrap();
        let k2 = reloaded.derive_address_private_key("acct", 0).unwrap();
        assert_eq!(k1.to_bytes(), k2.to_bytes());
    }

    /// R3-61: the Debug representation of an HDWallet must never contain the
    /// plaintext mnemonic. A leak here would expose the master seed phrase in
    /// any log, panic, or `{:?}` output.
    #[test]
    fn debug_output_redacts_mnemonic() {
        let dir = tempfile::tempdir().unwrap();
        let w =
            HDWallet::from_mnemonic(TEST_MNEMONIC, Network::Testnet, dir.path().join("w.json"))
                .unwrap();

        let debug = format!("{:?}", w);
        // No seed word must appear in the Debug output.
        for word in TEST_MNEMONIC.split_whitespace() {
            assert!(
                !debug.contains(word),
                "Debug output leaked mnemonic word: {word}"
            );
        }
        assert!(debug.contains("<redacted>"));
    }

    /// R5-91: `load_encrypted` must not panic on a malformed backup whose nonce
    /// field decodes to a length other than 12 bytes. A truncated (or empty,
    /// via `#[serde(default)]`) nonce previously reached the panicking
    /// `AesNonce::from_slice`; it must now surface a `DecryptionError` instead.
    #[test]
    fn load_encrypted_rejects_short_nonce_without_panic() {
        use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};

        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("wallet.json");
        let mut w =
            HDWallet::from_mnemonic(TEST_MNEMONIC, Network::Testnet, path.clone()).unwrap();

        let password = "Xq9!vTp#Lm7$Rw4&ZkBnHjCdFgVs";
        w.save_encrypted(password).unwrap();

        // Corrupt the on-disk backup: replace the 12-byte nonce with a 4-byte one.
        let json = std::fs::read_to_string(&path).unwrap();
        let mut backup: EncryptedBackup = serde_json::from_str(&json).unwrap();
        backup.nonce = BASE64.encode([0u8; 4]);
        std::fs::write(&path, serde_json::to_string(&backup).unwrap()).unwrap();

        // Must return an error, not panic.
        match HDWallet::load_encrypted(path.clone(), password) {
            Err(HDWalletError::DecryptionError(_)) => {}
            other => panic!("expected DecryptionError for short nonce, got {other:?}"),
        }

        // An empty nonce (as a missing/defaulted field would produce) is also rejected.
        let json = std::fs::read_to_string(&path).unwrap();
        let mut backup: EncryptedBackup = serde_json::from_str(&json).unwrap();
        backup.nonce = String::new();
        std::fs::write(&path, serde_json::to_string(&backup).unwrap()).unwrap();
        assert!(matches!(
            HDWallet::load_encrypted(path, password),
            Err(HDWalletError::DecryptionError(_))
        ));
    }
}
