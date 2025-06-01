use anyhow::{Context, Result};
use bip32::{XPrv, DerivationPath};
use bip39::Mnemonic;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use std::str::FromStr;
use sha2::{Sha256, Digest};
use ripemd::{Ripemd160, Digest as RipemdDigest};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Wallet {
    pub name: String,
    pub mnemonic: String,
    pub network: String,
    pub addresses: Vec<WalletAddress>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WalletAddress {
    pub index: u32,
    pub address: String,
    pub public_key: String,
    pub private_key: String,
}

pub struct WalletManager {
    wallet_dir: PathBuf,
}

impl WalletManager {
    pub fn new(wallet_dir: PathBuf) -> Result<Self> {
        fs::create_dir_all(&wallet_dir)
            .context("Failed to create wallet directory")?;
        
        Ok(Self { wallet_dir })
    }
    
    /// Create a new wallet with a random mnemonic
    pub fn create_wallet(&self, name: &str, network_str: &str) -> Result<Wallet> {
        // Generate random entropy for mnemonic
        let mut entropy = [0u8; 16]; // 128 bits for 12 words
        getrandom::getrandom(&mut entropy)
            .map_err(|e| anyhow::anyhow!("Failed to generate random entropy: {}", e))?;
        
        let mnemonic = Mnemonic::from_entropy(&entropy)
            .map_err(|e| anyhow::anyhow!("Failed to generate mnemonic: {}", e))?;
        
        let wallet = Wallet {
            name: name.to_string(),
            mnemonic: mnemonic.to_string(),
            network: network_str.to_string(),
            addresses: Vec::new(),
        };
        
        self.save_wallet(&wallet)?;
        Ok(wallet)
    }
    
    /// Import a wallet from a mnemonic phrase
    pub fn import_wallet(&self, name: &str, mnemonic_str: &str, network_str: &str) -> Result<Wallet> {
        // Validate mnemonic
        let mnemonic = Mnemonic::parse(mnemonic_str)
            .map_err(|e| anyhow::anyhow!("Invalid mnemonic: {}", e))?;
        
        let wallet = Wallet {
            name: name.to_string(),
            mnemonic: mnemonic.to_string(),
            network: network_str.to_string(),
            addresses: Vec::new(),
        };
        
        self.save_wallet(&wallet)?;
        Ok(wallet)
    }
    
    /// Generate a new address for the wallet
    pub fn generate_address(&self, wallet: &mut Wallet) -> Result<WalletAddress> {
        let mnemonic = Mnemonic::parse(&wallet.mnemonic)
            .map_err(|e| anyhow::anyhow!("Invalid mnemonic: {}", e))?;
        
        let seed = mnemonic.to_seed("");
        
        // BIP44 path: m/44'/0'/0'/0/index
        // Note: Using coin type 0 for now, but Supernova should register its own coin type
        let index = wallet.addresses.len() as u32;
        let path = DerivationPath::from_str(&format!("m/44'/0'/0'/0/{}", index))
            .context("Failed to create derivation path")?;
        
        // Derive child key directly from seed
        let child_xprv = XPrv::derive_from_path(&seed, &path)
            .map_err(|e| anyhow::anyhow!("Failed to derive key: {:?}", e))?;
        
        // Get the signing key and create address
        let signing_key = child_xprv.private_key();
        let verifying_key = child_xprv.public_key().public_key();
        
        // Generate Supernova address
        let network_prefix = get_network_prefix(&wallet.network);
        let address = generate_supernova_address(&verifying_key.to_bytes(), network_prefix)?;
        
        // Export private key in hex format (Supernova format)
        let private_key_hex = hex::encode(signing_key.to_bytes());
        
        let wallet_address = WalletAddress {
            index,
            address,
            public_key: hex::encode(verifying_key.to_bytes()),
            private_key: private_key_hex,
        };
        
        wallet.addresses.push(wallet_address.clone());
        self.save_wallet(wallet)?;
        
        Ok(wallet_address)
    }
    
    /// List all wallets
    pub fn list_wallets(&self) -> Result<Vec<String>> {
        let mut wallets = Vec::new();
        
        for entry in fs::read_dir(&self.wallet_dir)? {
            let entry = entry?;
            let path = entry.path();
            
            if path.extension().and_then(|s| s.to_str()) == Some("json") {
                if let Some(name) = path.file_stem().and_then(|s| s.to_str()) {
                    wallets.push(name.to_string());
                }
            }
        }
        
        Ok(wallets)
    }
    
    /// Load a wallet by name
    pub fn load_wallet(&self, name: &str) -> Result<Wallet> {
        let path = self.wallet_path(name);
        let contents = fs::read_to_string(&path)
            .context("Failed to read wallet file")?;
        
        let wallet: Wallet = serde_json::from_str(&contents)
            .context("Failed to parse wallet file")?;
        
        Ok(wallet)
    }
    
    /// Save a wallet
    fn save_wallet(&self, wallet: &Wallet) -> Result<()> {
        let path = self.wallet_path(&wallet.name);
        let contents = serde_json::to_string_pretty(wallet)
            .context("Failed to serialize wallet")?;
        
        fs::write(&path, contents)
            .context("Failed to write wallet file")?;
        
        Ok(())
    }
    
    /// Get the path for a wallet file
    fn wallet_path(&self, name: &str) -> PathBuf {
        self.wallet_dir.join(format!("{}.json", name))
    }
    
    /// Export wallet private keys (dangerous!)
    pub fn export_private_keys(&self, wallet: &Wallet) -> Vec<(String, String)> {
        wallet.addresses
            .iter()
            .map(|addr| (addr.address.clone(), addr.private_key.clone()))
            .collect()
    }
    
    /// Get wallet info without sensitive data
    pub fn get_wallet_info(&self, wallet: &Wallet) -> WalletInfo {
        WalletInfo {
            name: wallet.name.clone(),
            network: wallet.network.clone(),
            address_count: wallet.addresses.len(),
            addresses: wallet.addresses
                .iter()
                .map(|addr| addr.address.clone())
                .collect(),
        }
    }
}

#[derive(Debug, Serialize)]
pub struct WalletInfo {
    pub name: String,
    pub network: String,
    pub address_count: usize,
    pub addresses: Vec<String>,
}

/// Generate a Supernova address from a public key
fn generate_supernova_address(public_key: &[u8], prefix: &str) -> Result<String> {
    // Step 1: SHA256 hash of the public key
    let sha256 = Sha256::digest(public_key);
    
    // Step 2: RIPEMD160 hash of the SHA256 hash
    let ripemd160 = Ripemd160::digest(&sha256);
    
    // Step 3: Add version byte (network prefix)
    let mut versioned = Vec::new();
    versioned.extend_from_slice(prefix.as_bytes());
    versioned.extend_from_slice(&ripemd160);
    
    // Step 4: Double SHA256 for checksum
    let checksum = Sha256::digest(&Sha256::digest(&versioned));
    
    // Step 5: Append first 4 bytes of checksum
    versioned.extend_from_slice(&checksum[..4]);
    
    // Step 6: Base58 encode
    Ok(bs58::encode(versioned).into_string())
}

/// Get the network prefix for Supernova addresses
fn get_network_prefix(network: &str) -> &'static str {
    match network.to_lowercase().as_str() {
        "mainnet" | "main" => "SN", // Supernova mainnet
        "testnet" | "test" => "ST", // Supernova testnet
        "devnet" | "dev" => "SD",   // Supernova devnet
        _ => "ST", // Default to testnet
    }
} 