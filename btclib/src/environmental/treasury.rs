use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use thiserror::Error;
use serde::{Serialize, Deserialize};
use chrono::{DateTime, Utc, NaiveDateTime};
use crate::types::transaction::Transaction;
use crate::environmental::emissions::{EmissionsCalculator, NetworkEmissions};
use crate::environmental::verification::{RenewableCertificate, CarbonOffset};
use std::sync::{Arc, RwLock};
use std::time::{SystemTime, UNIX_EPOCH};

/// Error types for environmental treasury operations
#[derive(Error, Debug)]
pub enum TreasuryError {
    #[error("Insufficient funds: required {0}, available {1}")]
    InsufficientFunds(u64, u64),
    
    #[error("Invalid miner registration: {0}")]
    InvalidMinerRegistration(String),
    
    #[error("Invalid asset purchase: {0}")]
    InvalidAssetPurchase(String),
    
    #[error("Invalid allocation parameters: {0}")]
    InvalidAllocation(String),
    
    #[error("Database error: {0}")]
    DatabaseError(String),
    
    #[error("Insufficient funds in treasury: {0}")]
    InsufficientFundsInTreasury(String),
    
    #[error("Invalid certificate: {0}")]
    InvalidCertificate(String),
    
    #[error("Invalid offset: {0}")]
    InvalidOffset(String),
    
    #[error("Operation unauthorized: {0}")]
    Unauthorized(String),
    
    #[error("Internal treasury error: {0}")]
    Internal(String),
    
    #[error("Data serialization error: {0}")]
    Serialization(String),
}

/// Type of environmental asset
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum EnvironmentalAssetType {
    /// Renewable Energy Certificate (MWh)
    RenewableEnergyCertificate,
    
    /// Carbon Credit (tonnes CO2e)
    CarbonOffset,
}

/// Status of verification for environmental claims
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum VerificationStatus {
    /// Verification is pending
    Pending,
    
    /// Verification has been approved
    Approved,
    
    /// Verification has been rejected
    Rejected,
}

/// Information about verification of environmental claims
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerificationInfo {
    /// Organization providing verification
    pub provider: String,
    
    /// Date of verification
    pub date: DateTime<Utc>,
    
    /// Reference identifier for the verification
    pub reference: String,
    
    /// Status of the verification
    pub status: VerificationStatus,
}

/// Information about a green miner
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GreenMinerInfo {
    /// Percentage of renewable energy used (0-100)
    pub renewable_percentage: f64,
    
    /// Verification information, if available
    pub verification: Option<VerificationInfo>,
    
    /// Date the miner was registered
    pub registration_date: DateTime<Utc>,
    
    /// Date the information was last updated
    pub last_updated: DateTime<Utc>,
}

/// Environmental asset purchase record
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnvironmentalAssetPurchase {
    /// Type of asset purchased
    pub asset_type: EnvironmentalAssetType,
    
    /// Amount of asset (MWh for RECs, tonnes CO2e for offsets)
    pub amount: f64,
    
    /// Cost in satoshis
    pub cost: u64,
    
    /// Date of purchase
    pub date: DateTime<Utc>,
    
    /// Provider of the asset
    pub provider: String,
    
    /// Reference identifier for the purchase
    pub reference: String,
    
    /// Environmental impact score (higher is better)
    pub impact_score: f64,
}

/// Treasury account types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TreasuryAccountType {
    /// Main treasury account
    Main,
    /// Renewable energy certificate purchases
    RenewableCertificates,
    /// Carbon offset purchases
    CarbonOffsets,
    /// Environmental grants and initiatives
    Grants,
    /// Operations and management
    Operations,
    /// Emergency reserve
    Reserve,
}

impl Hash for TreasuryAccountType {
    fn hash<H: Hasher>(&self, state: &mut H) {
        // Use the discriminant value for hashing
        (*self as u8).hash(state);
    }
}

/// Treasury allocation percentages
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TreasuryAllocation {
    /// Percentage for renewable certificates
    pub renewable_certificates: f64,
    /// Percentage for carbon offsets
    pub carbon_offsets: f64,
    /// Percentage for environmental grants
    pub grants: f64,
    /// Percentage for operations
    pub operations: f64,
    /// Percentage for emergency reserve
    pub reserve: f64,
}

impl Default for TreasuryAllocation {
    fn default() -> Self {
        Self {
            renewable_certificates: 50.0, // 50% for RECs (prioritized)
            carbon_offsets: 30.0,         // 30% for carbon offsets
            grants: 10.0,                 // 10% for environmental grants
            operations: 5.0,              // 5% for operations
            reserve: 5.0,                 // 5% for reserve
        }
    }
}

/// Treasury account balance and transaction history
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TreasuryAccount {
    /// Account type
    pub account_type: TreasuryAccountType,
    /// Current balance in satoshis
    pub balance: u64,
    /// Transactions for this account
    pub transactions: Vec<TreasuryTransaction>,
}

/// Treasury transaction record
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TreasuryTransaction {
    /// Transaction ID
    pub id: String,
    /// Transaction timestamp
    pub timestamp: u64,
    /// Transaction amount in satoshis
    pub amount: u64,
    /// Transaction description
    pub description: String,
    /// Associated certificate ID (if applicable)
    pub certificate_id: Option<String>,
    /// Associated offset ID (if applicable)
    pub offset_id: Option<String>,
    /// Transaction category
    pub category: TreasuryTransactionCategory,
}

/// Treasury transaction categories
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum TreasuryTransactionCategory {
    /// Fee deposit
    Deposit,
    /// Certificate purchase
    CertificatePurchase,
    /// Offset purchase
    OffsetPurchase,
    /// Grant allocation
    Grant,
    /// Operational expense
    Expense,
    /// Internal transfer between accounts
    Transfer,
}

/// Environmental impact metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnvironmentalImpact {
    /// CO2 equivalent emissions in tons
    pub emissions_tons_co2e: f64,
    /// CO2 equivalent offsets in tons (carbon offset credits)
    pub offset_tons_co2e: f64,
    /// Renewable energy in MWh (from certificates)
    pub renewable_energy_mwh: f64,
    /// Net impact (offset_tons_co2e - emissions_tons_co2e)
    pub net_impact_tons_co2e: f64,
    /// Carbon negative ratio (offset / emissions)
    pub carbon_negative_ratio: f64,
    /// Carbon negativity target ratio
    pub target_ratio: f64,
    /// Whether the network is currently carbon negative
    pub is_carbon_negative: bool,
    /// Last updated timestamp
    pub last_updated: u64,
}

impl Default for EnvironmentalImpact {
    fn default() -> Self {
        Self {
            emissions_tons_co2e: 0.0,
            offset_tons_co2e: 0.0,
            renewable_energy_mwh: 0.0,
            net_impact_tons_co2e: 0.0,
            carbon_negative_ratio: 0.0,
            target_ratio: 1.5, // Target is to offset 150% of emissions
            is_carbon_negative: false,
            last_updated: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
        }
    }
}

/// Treasury configuration options
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TreasuryConfig {
    /// Base fee allocation percentage for environmental treasury
    pub base_fee_allocation_percentage: f64,
    /// Dynamic fee adjustment based on carbon negativity
    pub enable_dynamic_fee_adjustment: bool,
    /// Minimum fee allocation percentage
    pub min_fee_allocation_percentage: f64,
    /// Maximum fee allocation percentage
    pub max_fee_allocation_percentage: f64,
    /// Target carbon negative ratio
    pub target_carbon_negative_ratio: f64,
    /// Treasury allocations
    pub allocation: TreasuryAllocation,
    /// Authorized signers (multi-sig governance)
    pub authorized_signers: Vec<String>,
    /// Required signatures for operations
    pub required_signatures: usize,
    /// Automatically purchase certificates and offsets
    pub auto_purchase: bool,
    /// Incentives multiplier for renewable energy
    pub renewable_incentive_multiplier: f64,
    /// Incentives multiplier for carbon offsets
    pub offset_incentive_multiplier: f64,
}

impl Default for TreasuryConfig {
    fn default() -> Self {
        Self {
            base_fee_allocation_percentage: 2.0,       // 2% of transaction fees by default
            enable_dynamic_fee_adjustment: true,       // Enable dynamic adjustment
            min_fee_allocation_percentage: 1.0,        // Minimum 1% allocation
            max_fee_allocation_percentage: 5.0,        // Maximum 5% allocation
            target_carbon_negative_ratio: 1.5,         // Target 150% offset
            allocation: TreasuryAllocation::default(),
            authorized_signers: Vec::new(),
            required_signatures: 3,                    // Require 3 signatures by default
            auto_purchase: true,                       // Auto-purchase by default
            renewable_incentive_multiplier: 2.0,       // 2x incentive for renewable certificates
            offset_incentive_multiplier: 1.2,          // 1.2x incentive for verified offsets
        }
    }
}

/// Environmental treasury system for managing carbon negativity
pub struct EnvironmentalTreasury {
    /// Treasury configuration
    config: RwLock<TreasuryConfig>,
    /// Account balances
    accounts: RwLock<HashMap<TreasuryAccountType, TreasuryAccount>>,
    /// Environmental impact metrics
    impact: RwLock<EnvironmentalImpact>,
    /// Registered renewable certificates
    certificates: RwLock<Vec<RenewableCertificate>>,
    /// Registered carbon offsets
    offsets: RwLock<Vec<CarbonOffset>>,
    /// Emissions calculator
    emissions_calculator: Arc<EmissionsCalculator>,
    /// Current fee allocation percentage (may be dynamically adjusted)
    current_fee_percentage: RwLock<f64>,
}

// Manual implementation of Clone for EnvironmentalTreasury
impl Clone for EnvironmentalTreasury {
    fn clone(&self) -> Self {
        // Create a new instance with cloned data
        Self {
            // Clone the inner values by acquiring read locks
            config: RwLock::new(self.config.read().unwrap().clone()),
            accounts: RwLock::new(self.accounts.read().unwrap().clone()),
            impact: RwLock::new(self.impact.read().unwrap().clone()),
            certificates: RwLock::new(self.certificates.read().unwrap().clone()),
            offsets: RwLock::new(self.offsets.read().unwrap().clone()),
            // Arc can be cloned directly
            emissions_calculator: self.emissions_calculator.clone(),
            current_fee_percentage: RwLock::new(*self.current_fee_percentage.read().unwrap()),
        }
    }
}

impl EnvironmentalTreasury {
    /// Create a new environmental treasury
    pub fn new(
        fee_allocation_percentage: f64,
        authorized_signers: Vec<String>,
        required_signatures: usize,
    ) -> Self {
        // Create basic configuration
        let config = TreasuryConfig {
            base_fee_allocation_percentage: fee_allocation_percentage,
            enable_dynamic_fee_adjustment: true,
            min_fee_allocation_percentage: 1.0,
            max_fee_allocation_percentage: 10.0,
            target_carbon_negative_ratio: 1.2,
            allocation: TreasuryAllocation::default(),
            authorized_signers,
            required_signatures,
            auto_purchase: true,
            renewable_incentive_multiplier: 2.0,
            offset_incentive_multiplier: 1.5,
        };
        
        // Create initial accounts
        let mut accounts = HashMap::new();
        for account_type in [
            TreasuryAccountType::Main,
            TreasuryAccountType::RenewableCertificates,
            TreasuryAccountType::CarbonOffsets,
            TreasuryAccountType::Grants,
            TreasuryAccountType::Operations,
            TreasuryAccountType::Reserve,
        ] {
            accounts.insert(account_type, TreasuryAccount {
                account_type,
                balance: 0,
                transactions: Vec::new(),
            });
        }
        
        Self {
            config: RwLock::new(config),
            accounts: RwLock::new(accounts),
            impact: RwLock::new(EnvironmentalImpact::default()),
            certificates: RwLock::new(Vec::new()),
            offsets: RwLock::new(Vec::new()),
            emissions_calculator: Arc::new(EmissionsCalculator::new()),
            current_fee_percentage: RwLock::new(fee_allocation_percentage),
        }
    }
    
    /// Process transaction fees, allocating a portion to the environmental treasury
    pub fn process_transaction_fees(&self, total_fees: u64) -> Result<u64, TreasuryError> {
        let fee_percentage = *self.current_fee_percentage.read().unwrap();
        let treasury_amount = (total_fees as f64 * fee_percentage / 100.0) as u64;
        
        if treasury_amount == 0 {
            return Ok(0);
        }
        
        // Add to main treasury account
        let mut accounts = self.accounts.write().unwrap();
        let main_account = accounts.get_mut(&TreasuryAccountType::Main).unwrap();
        main_account.balance += treasury_amount;
        
        // Record transaction
        let tx_id = format!("fee_{}", generate_id());
        main_account.transactions.push(TreasuryTransaction {
            id: tx_id,
            timestamp: current_timestamp(),
            amount: treasury_amount,
            description: format!("Transaction fee allocation ({}%)", fee_percentage),
            certificate_id: None,
            offset_id: None,
            category: TreasuryTransactionCategory::Deposit,
        });
        
        // Distribute funds to sub-accounts according to allocation percentages
        let config = self.config.read().unwrap();
        
        self.distribute_funds(
            TreasuryAccountType::Main,
            TreasuryAccountType::RenewableCertificates,
            (treasury_amount as f64 * config.allocation.renewable_certificates / 100.0) as u64,
            "Automatic allocation to renewable certificates",
            &mut accounts,
        )?;
        
        self.distribute_funds(
            TreasuryAccountType::Main,
            TreasuryAccountType::CarbonOffsets,
            (treasury_amount as f64 * config.allocation.carbon_offsets / 100.0) as u64,
            "Automatic allocation to carbon offsets",
            &mut accounts,
        )?;
        
        self.distribute_funds(
            TreasuryAccountType::Main,
            TreasuryAccountType::Grants,
            (treasury_amount as f64 * config.allocation.grants / 100.0) as u64,
            "Automatic allocation to environmental grants",
            &mut accounts,
        )?;
        
        self.distribute_funds(
            TreasuryAccountType::Main,
            TreasuryAccountType::Operations,
            (treasury_amount as f64 * config.allocation.operations / 100.0) as u64,
            "Automatic allocation to operations",
            &mut accounts,
        )?;
        
        self.distribute_funds(
            TreasuryAccountType::Main,
            TreasuryAccountType::Reserve,
            (treasury_amount as f64 * config.allocation.reserve / 100.0) as u64,
            "Automatic allocation to emergency reserve",
            &mut accounts,
        )?;
        
        // Auto-purchase certificates and offsets if enabled
        if config.auto_purchase {
            drop(accounts); // Release lock before calling auto-purchase
            self.auto_purchase_certificates_and_offsets()?;
        }
        
        // Update environmental impact
        self.update_environmental_impact();
        
        // Adjust fee percentage based on carbon negativity if enabled
        if config.enable_dynamic_fee_adjustment {
            self.adjust_fee_percentage();
        }
        
        Ok(treasury_amount)
    }
    
    /// Register a renewable energy certificate
    pub fn register_certificate(&self, certificate: RenewableCertificate) -> Result<(), TreasuryError> {
        // Validate certificate
        if certificate.amount_mwh <= 0.0 {
            return Err(TreasuryError::InvalidCertificate(
                "Certificate energy amount must be positive".to_string(),
            ));
        }
        
        // Add certificate to registry
        self.certificates.write().unwrap().push(certificate);
        
        // Update environmental impact
        self.update_environmental_impact();
        
        Ok(())
    }
    
    /// Register a carbon offset
    pub fn register_offset(&self, offset: CarbonOffset) -> Result<(), TreasuryError> {
        // Validate offset
        if offset.amount_tons_co2e <= 0.0 {
            return Err(TreasuryError::InvalidOffset(
                "Offset amount must be positive".to_string(),
            ));
        }
        
        // Add offset to registry
        self.offsets.write().unwrap().push(offset);
        
        // Update environmental impact
        self.update_environmental_impact();
        
        Ok(())
    }
    
    /// Purchase a renewable energy certificate
    pub fn purchase_certificate(
        &self,
        provider: &str,
        amount_mwh: f64,
        cost: u64,
        description: &str,
    ) -> Result<String, TreasuryError> {
        // Check available funds
        let mut accounts = self.accounts.write().unwrap();
        let cert_account = accounts.get_mut(&TreasuryAccountType::RenewableCertificates).unwrap();
        
        if cert_account.balance < cost {
            return Err(TreasuryError::InsufficientFundsInTreasury(format!(
                "Insufficient funds for certificate purchase: {} < {}",
                cert_account.balance, cost
            )));
        }
        
        // Create certificate
        let certificate_id = format!("cert_{}", generate_id());
        let certificate = RenewableCertificate {
            id: certificate_id.clone(),
            provider: provider.to_string(),
            amount_mwh,
            timestamp: current_timestamp(),
            description: description.to_string(),
            verification_status: true,
            cost,
        };
        
        // Deduct funds
        cert_account.balance -= cost;
        
        // Record transaction
        cert_account.transactions.push(TreasuryTransaction {
            id: format!("purchase_{}", generate_id()),
            timestamp: current_timestamp(),
            amount: cost,
            description: format!("Purchase of renewable energy certificate: {} MWh", amount_mwh),
            certificate_id: Some(certificate_id.clone()),
            offset_id: None,
            category: TreasuryTransactionCategory::CertificatePurchase,
        });
        
        // Register certificate
        drop(accounts); // Release lock before calling register_certificate
        self.register_certificate(certificate)?;
        
        Ok(certificate_id)
    }
    
    /// Purchase a carbon offset
    pub fn purchase_offset(
        &self,
        provider: &str,
        amount_tons_co2e: f64,
        cost: u64,
        description: &str,
    ) -> Result<String, TreasuryError> {
        // Check available funds
        let mut accounts = self.accounts.write().unwrap();
        let offset_account = accounts.get_mut(&TreasuryAccountType::CarbonOffsets).unwrap();
        
        if offset_account.balance < cost {
            return Err(TreasuryError::InsufficientFundsInTreasury(format!(
                "Insufficient funds for offset purchase: {} < {}",
                offset_account.balance, cost
            )));
        }
        
        // Create offset
        let offset_id = format!("offset_{}", generate_id());
        let offset = CarbonOffset {
            id: offset_id.clone(),
            provider: provider.to_string(),
            amount_tons_co2e,
            timestamp: current_timestamp(),
            description: description.to_string(),
            verification_status: true,
            cost,
        };
        
        // Deduct funds
        offset_account.balance -= cost;
        
        // Record transaction
        offset_account.transactions.push(TreasuryTransaction {
            id: format!("purchase_{}", generate_id()),
            timestamp: current_timestamp(),
            amount: cost,
            description: format!("Purchase of carbon offset: {} tons CO2e", amount_tons_co2e),
            certificate_id: None,
            offset_id: Some(offset_id.clone()),
            category: TreasuryTransactionCategory::OffsetPurchase,
        });
        
        // Register offset
        drop(accounts); // Release lock before calling register_offset
        self.register_offset(offset)?;
        
        Ok(offset_id)
    }
    
    /// Get current environmental impact metrics
    pub fn get_environmental_impact(&self) -> EnvironmentalImpact {
        self.impact.read().unwrap().clone()
    }
    
    /// Get account balances
    pub fn get_account_balances(&self) -> HashMap<TreasuryAccountType, u64> {
        let accounts = self.accounts.read().unwrap();
        accounts.iter()
            .map(|(account_type, account)| (*account_type, account.balance))
            .collect()
    }
    
    /// Get all registered certificates
    pub fn get_certificates(&self) -> Vec<RenewableCertificate> {
        self.certificates.read().unwrap().clone()
    }
    
    /// Get all registered offsets
    pub fn get_offsets(&self) -> Vec<CarbonOffset> {
        self.offsets.read().unwrap().clone()
    }
    
    /// Get current fee allocation percentage
    pub fn get_current_fee_percentage(&self) -> f64 {
        *self.current_fee_percentage.read().unwrap()
    }
    
    /// Update treasury configuration
    pub fn update_config(&self, config: TreasuryConfig) -> Result<(), TreasuryError> {
        // Validate configuration
        if config.base_fee_allocation_percentage < 0.0 || config.base_fee_allocation_percentage > 100.0 {
            return Err(TreasuryError::Internal(
                "Base fee allocation percentage must be between 0 and 100".to_string(),
            ));
        }
        
        if config.min_fee_allocation_percentage < 0.0 || config.min_fee_allocation_percentage > 100.0 {
            return Err(TreasuryError::Internal(
                "Minimum fee allocation percentage must be between 0 and 100".to_string(),
            ));
        }
        
        if config.max_fee_allocation_percentage < config.min_fee_allocation_percentage {
            return Err(TreasuryError::Internal(
                "Maximum fee allocation percentage must be greater than minimum".to_string(),
            ));
        }
        
        if config.target_carbon_negative_ratio < 1.0 {
            return Err(TreasuryError::Internal(
                "Target carbon negative ratio must be at least 1.0".to_string(),
            ));
        }
        
        // Update configuration
        *self.config.write().unwrap() = config.clone();
        
        // Update current fee percentage
        *self.current_fee_percentage.write().unwrap() = config.base_fee_allocation_percentage;
        
        // Update impact target ratio
        self.impact.write().unwrap().target_ratio = config.target_carbon_negative_ratio;
        
        // Update environmental impact with new configuration
        self.update_environmental_impact();
        
        Ok(())
    }
    
    /// Internal function to distribute funds between accounts
    fn distribute_funds(
        &self,
        from_account: TreasuryAccountType,
        to_account: TreasuryAccountType,
        amount: u64,
        description: &str,
        accounts: &mut HashMap<TreasuryAccountType, TreasuryAccount>,
    ) -> Result<(), TreasuryError> {
        if amount == 0 {
            return Ok(());
        }
        
        let from = accounts.get_mut(&from_account).unwrap();
        if from.balance < amount {
            return Err(TreasuryError::InsufficientFundsInTreasury(format!(
                "Insufficient funds for transfer: {} < {}",
                from.balance, amount
            )));
        }
        
        // Deduct from source account
        from.balance -= amount;
        
        // Add transfer record
        let tx_id = format!("transfer_{}", generate_id());
        from.transactions.push(TreasuryTransaction {
            id: tx_id.clone(),
            timestamp: current_timestamp(),
            amount,
            description: format!("Transfer to {}: {}", to_account as u8, description),
            certificate_id: None,
            offset_id: None,
            category: TreasuryTransactionCategory::Transfer,
        });
        
        // Add to destination account
        let to = accounts.get_mut(&to_account).unwrap();
        to.balance += amount;
        
        // Add receipt record
        to.transactions.push(TreasuryTransaction {
            id: format!("receipt_{}", tx_id),
            timestamp: current_timestamp(),
            amount,
            description: format!("Received from {}: {}", from_account as u8, description),
            certificate_id: None,
            offset_id: None,
            category: TreasuryTransactionCategory::Transfer,
        });
        
        Ok(())
    }
    
    /// Update environmental impact metrics
    fn update_environmental_impact(&self) {
        // Get current network emissions - direct call, not a Result
        let network_emissions = self.emissions_calculator.calculate_network_emissions();
        
        // Calculate total renewable energy
        let renewable_energy_mwh: f64 = self.certificates.read().unwrap()
            .iter()
            .filter(|cert| cert.verification_status)
            .map(|cert| cert.amount_mwh)
            .sum();
        
        // Calculate total offsets
        let offset_tons_co2e: f64 = self.offsets.read().unwrap()
            .iter()
            .filter(|offset| offset.verification_status)
            .map(|offset| offset.amount_tons_co2e)
            .sum();
        
        // Calculate net impact
        let emissions_tons_co2e = network_emissions.total_emissions_tons_co2e;
        let net_impact_tons_co2e = offset_tons_co2e - emissions_tons_co2e;
        
        // Calculate carbon negative ratio
        let carbon_negative_ratio = if emissions_tons_co2e > 0.0 {
            offset_tons_co2e / emissions_tons_co2e
        } else {
            0.0
        };
        
        let target_ratio = self.config.read().unwrap().target_carbon_negative_ratio;
        let is_carbon_negative = carbon_negative_ratio >= target_ratio;
        
        // Update impact metrics
        let mut impact = self.impact.write().unwrap();
        impact.emissions_tons_co2e = emissions_tons_co2e;
        impact.offset_tons_co2e = offset_tons_co2e;
        impact.renewable_energy_mwh = renewable_energy_mwh;
        impact.net_impact_tons_co2e = net_impact_tons_co2e;
        impact.carbon_negative_ratio = carbon_negative_ratio;
        impact.target_ratio = target_ratio;
        impact.is_carbon_negative = is_carbon_negative;
        impact.last_updated = current_timestamp();
    }
    
    /// Automatically purchase certificates and offsets to maintain carbon negativity
    fn auto_purchase_certificates_and_offsets(&self) -> Result<(), TreasuryError> {
        let impact = self.impact.read().unwrap().clone();
        
        // Check if we need to purchase more offsets to maintain carbon negativity
        if impact.carbon_negative_ratio < impact.target_ratio {
            let required_offset = impact.emissions_tons_co2e * impact.target_ratio - impact.offset_tons_co2e;
            if required_offset > 0.0 {
                // Determine how many offsets we can buy with current funds
                let accounts_lock = self.accounts.read().unwrap();
                let offset_account = accounts_lock.get(&TreasuryAccountType::CarbonOffsets).unwrap();
                let available_funds = offset_account.balance;
                
                // Estimate cost per ton of CO2e (simplified)
                let estimated_cost_per_ton = 500_000; // 500,000 satoshis per ton
                let max_purchase_amount = available_funds / estimated_cost_per_ton;
                
                if max_purchase_amount > 0 {
                    let purchase_amount = std::cmp::min(
                        required_offset as u64,
                        max_purchase_amount,
                    );
                    
                    // Purchase offset
                    self.purchase_offset(
                        "Auto-purchased Verified Carbon Standard",
                        purchase_amount as f64,
                        purchase_amount * estimated_cost_per_ton,
                        "Automatic purchase to maintain carbon negativity",
                    )?;
                }
            }
        }
        
        // Also allocate funds for renewable energy certificates
        let accounts_lock = self.accounts.read().unwrap();
        let cert_account = accounts_lock.get(&TreasuryAccountType::RenewableCertificates).unwrap();
        let available_funds = cert_account.balance;
        
        // Estimate cost per MWh of renewable energy (simplified)
        let estimated_cost_per_mwh = 100_000; // 100,000 satoshis per MWh
        let max_purchase_amount = available_funds / estimated_cost_per_mwh;
        
        if max_purchase_amount > 0 {
            // Purchase renewable energy certificate
            self.purchase_certificate(
                "Auto-purchased Renewable Energy Certificate",
                max_purchase_amount as f64,
                max_purchase_amount * estimated_cost_per_mwh,
                "Automatic purchase to support renewable energy",
            )?;
        }
        
        Ok(())
    }
    
    /// Adjust fee percentage based on carbon negativity
    fn adjust_fee_percentage(&self) {
        let impact = self.impact.read().unwrap();
        let config = self.config.read().unwrap();
        
        // If we're not carbon negative, increase the fee percentage
        let mut new_percentage = config.base_fee_allocation_percentage;
        
        if impact.carbon_negative_ratio < impact.target_ratio {
            // Calculate how much to increase based on how far we are from the target
            let shortfall_ratio = impact.target_ratio / impact.carbon_negative_ratio.max(0.01);
            
            // Apply a scaling factor to avoid too rapid changes
            let scaling_factor = 0.5;
            new_percentage = (config.base_fee_allocation_percentage * (1.0 + scaling_factor * (shortfall_ratio - 1.0)))
                .max(config.min_fee_allocation_percentage)
                .min(config.max_fee_allocation_percentage);
        } else if impact.carbon_negative_ratio > impact.target_ratio * 1.5 {
            // If we're well above the target, gradually reduce the fee percentage
            let surplus_ratio = impact.carbon_negative_ratio / impact.target_ratio;
            
            // Apply a scaling factor for gradual reduction
            let scaling_factor = 0.3;
            new_percentage = (config.base_fee_allocation_percentage / (1.0 + scaling_factor * (surplus_ratio - 1.0)))
                .max(config.min_fee_allocation_percentage);
        }
        
        // Update the current fee percentage
        *self.current_fee_percentage.write().unwrap() = new_percentage;
    }

    /// Transfer funds between treasury accounts
    pub fn transfer_between_accounts(
        &mut self,
        from_account: TreasuryAccountType,
        to_account: TreasuryAccountType,
        amount: u64
    ) -> Result<(), TreasuryError> {
        let mut accounts = self.accounts.write().unwrap();
        self.distribute_funds(from_account, to_account, amount, "Manual transfer", &mut accounts)
    }

    /// Update the fee allocation percentage
    pub fn update_fee_allocation_percentage(&mut self, percentage: f64) -> Result<(), TreasuryError> {
        if percentage < 0.0 || percentage > 100.0 {
            return Err(TreasuryError::InvalidAllocation(
                "Fee allocation percentage must be between 0 and 100".to_string()
            ));
        }

        let mut config = self.config.write().unwrap();
        config.base_fee_allocation_percentage = percentage;
        *self.current_fee_percentage.write().unwrap() = percentage;

        Ok(())
    }

    /// Purchase renewable energy certificates - wrapper for the existing purchase_certificate method
    pub fn purchase_renewable_certificates(
        &self,
        provider: &str,
        amount_mwh: f64,
        cost: u64
    ) -> Result<String, TreasuryError> {
        self.purchase_certificate(
            provider,
            amount_mwh,
            cost,
            "Manually purchased renewable energy certificate"
        )
    }

    /// Purchase carbon offsets - wrapper for the existing purchase_offset method
    pub fn purchase_carbon_offsets(
        &self,
        provider: &str,
        amount_tons_co2e: f64,
        cost: u64
    ) -> Result<String, TreasuryError> {
        self.purchase_offset(
            provider,
            amount_tons_co2e,
            cost,
            "Manually purchased carbon offset"
        )
    }

    /// Fund an environmental project from the Grants account
    pub fn fund_project(
        &self,
        project_name: &str,
        amount: u64,
        description: &str
    ) -> Result<String, TreasuryError> {
        // Check available funds
        let mut accounts = self.accounts.write().unwrap();
        let grants_account = accounts.get_mut(&TreasuryAccountType::Grants).unwrap();
        
        if grants_account.balance < amount {
            return Err(TreasuryError::InsufficientFundsInTreasury(format!(
                "Insufficient funds for project funding: {} < {}",
                grants_account.balance, amount
            )));
        }
        
        // Deduct funds
        grants_account.balance -= amount;
        
        // Generate project ID
        let project_id = format!("project_{}", generate_id());
        
        // Record transaction
        grants_account.transactions.push(TreasuryTransaction {
            id: format!("fund_{}", generate_id()),
            timestamp: current_timestamp(),
            amount,
            description: format!("Project funding: {} - {}", project_name, description),
            certificate_id: None,
            offset_id: None,
            category: TreasuryTransactionCategory::Grant,
        });
        
        Ok(project_id)
    }

    /// Get renewable energy certificates - alias for get_certificates for backward compatibility
    pub fn get_rec_certificates(&self) -> Vec<RenewableCertificate> {
        self.get_certificates()
    }

    /// Get carbon offsets - alias for get_offsets for backward compatibility
    pub fn get_carbon_offsets(&self) -> Vec<CarbonOffset> {
        self.get_offsets()
    }

    /// Get balance of a specific account
    pub fn get_balance(&self, account_type: TreasuryAccountType) -> u64 {
        let accounts = self.accounts.read().unwrap();
        match accounts.get(&account_type) {
            Some(account) => account.balance,
            None => 0,
        }
    }

    /// Get the current allocation configuration
    pub fn get_allocation(&self) -> TreasuryAllocation {
        self.config.read().unwrap().allocation.clone()
    }

    /// Get recent purchases of environmental assets
    pub fn get_recent_purchases(&self, limit: usize) -> Vec<EnvironmentalAssetPurchase> {
        let mut purchases = Vec::new();
        
        // Get certificate purchases
        let certificates = self.certificates.read().unwrap();
        for cert in certificates.iter().take(limit) {
            purchases.push(EnvironmentalAssetPurchase {
                asset_type: EnvironmentalAssetType::RenewableEnergyCertificate,
                amount: cert.amount_mwh,
                cost: cert.cost,
                date: DateTime::<Utc>::from_naive_utc_and_offset(
                    NaiveDateTime::from_timestamp_opt(cert.timestamp as i64, 0).unwrap_or_default(),
                    Utc,
                ),
                provider: cert.provider.clone(),
                reference: cert.id.clone(),
                impact_score: cert.amount_mwh * 0.1, // Simple impact score calculation
            });
        }
        
        // Get offset purchases
        let offsets = self.offsets.read().unwrap();
        for offset in offsets.iter().take(limit) {
            purchases.push(EnvironmentalAssetPurchase {
                asset_type: EnvironmentalAssetType::CarbonOffset,
                amount: offset.amount_tons_co2e,
                cost: offset.cost,
                date: DateTime::<Utc>::from_naive_utc_and_offset(
                    NaiveDateTime::from_timestamp_opt(offset.timestamp as i64, 0).unwrap_or_default(),
                    Utc,
                ),
                provider: offset.provider.clone(),
                reference: offset.id.clone(),
                impact_score: offset.amount_tons_co2e * 0.5, // Simple impact score calculation
            });
        }
        
        // Sort by date (most recent first)
        purchases.sort_by(|a, b| b.date.cmp(&a.date));
        
        // Limit to requested number
        purchases.truncate(limit);
        
        purchases
    }

    /// Get asset purchases - alias for get_recent_purchases
    pub fn get_asset_purchases(&self, limit: usize) -> Vec<EnvironmentalAssetPurchase> {
        self.get_recent_purchases(limit)
    }

    /// Calculate miner fee discount based on renewable energy percentage
    pub fn calculate_miner_fee_discount(&self, miner_id: &str) -> f64 {
        // Find certificates for this miner
        let certificates = self.certificates.read().unwrap();
        let miner_certificates: Vec<_> = certificates.iter()
            .filter(|cert| cert.provider == miner_id && cert.verification_status)
            .collect();
            
        // Calculate total renewable energy amount
        let total_renewable_mwh: f64 = miner_certificates.iter()
            .map(|cert| cert.amount_mwh)
            .sum();
            
        // Apply discount tiers
        if total_renewable_mwh >= 95.0 {
            10.0 // 10% discount for 95%+ renewable
        } else if total_renewable_mwh >= 75.0 {
            5.0 // 5% discount for 75-94% renewable
        } else if total_renewable_mwh >= 25.0 {
            2.0 // 2% discount for 25-74% renewable
        } else {
            0.0 // No discount for <25% renewable
        }
    }

    /// Register a miner that uses renewable energy
    pub fn register_green_miner(
        &mut self,
        miner_id: &str,
        renewable_percentage: f64,
        verification: Option<VerificationInfo>
    ) -> Result<(), TreasuryError> {
        // Validate parameters
        if renewable_percentage < 0.0 || renewable_percentage > 100.0 {
            return Err(TreasuryError::InvalidMinerRegistration(
                "Renewable percentage must be between 0 and 100".to_string()
            ));
        }
        
        // In a full implementation, we would register this miner in a database
        // For now, just log it
        println!("Registered green miner {} with {}% renewable energy", 
                 miner_id, renewable_percentage);
        
        // Return success
        Ok(())
    }
    
    /// Process block allocation for the environmental treasury
    pub fn process_block_allocation(&mut self, total_fees: u64) -> TreasuryAllocation {
        // Process transaction fees, which updates all the internal accounts
        if let Ok(amount) = self.process_transaction_fees(total_fees) {
            println!("Allocated {} satoshis to environmental treasury", amount);
        }
        
        // Return the current allocation percentages
        self.get_allocation()
    }
    
    /// Purchase prioritized environmental assets based on current impact
    pub fn purchase_prioritized_assets(
        &mut self,
        total_amount: u64,
        rec_allocation_percentage: f64
    ) -> Result<Vec<EnvironmentalAssetPurchase>, TreasuryError> {
        if rec_allocation_percentage < 0.0 || rec_allocation_percentage > 100.0 {
            return Err(TreasuryError::InvalidAssetPurchase(
                "REC allocation percentage must be between 0 and 100".to_string()
            ));
        }
        
        let mut purchases = Vec::new();
        
        // Calculate amounts for RECs and carbon offsets
        let rec_amount = (total_amount as f64 * rec_allocation_percentage / 100.0) as u64;
        let offset_amount = total_amount - rec_amount;
        
        // Process REC purchase if applicable
        if rec_amount > 0 {
            let rec_mwh = (rec_amount as f64 / 100_000.0).max(1.0); // Simple conversion
            if let Ok(cert_id) = self.purchase_certificate(
                "Renewable Energy Marketplace",
                rec_mwh,
                rec_amount,
                "Prioritized purchase of renewable energy certificates"
            ) {
                // Find the certificate in our registry
                let certificates = self.certificates.read().unwrap();
                if let Some(cert) = certificates.iter().find(|c| c.id == cert_id) {
                    purchases.push(EnvironmentalAssetPurchase {
                        asset_type: EnvironmentalAssetType::RenewableEnergyCertificate,
                        amount: cert.amount_mwh,
                        cost: cert.cost,
                        date: DateTime::<Utc>::from_naive_utc_and_offset(
                            NaiveDateTime::from_timestamp_opt(cert.timestamp as i64, 0).unwrap_or_default(),
                            Utc,
                        ),
                        provider: cert.provider.clone(),
                        reference: cert.id.clone(),
                        impact_score: 1.0,
                    });
                }
            }
        }
        
        // Process carbon offset purchase if applicable
        if offset_amount > 0 {
            let offset_tons = (offset_amount as f64 / 200_000.0).max(0.5); // Simple conversion
            if let Ok(offset_id) = self.purchase_offset(
                "Verified Carbon Standard",
                offset_tons,
                offset_amount,
                "Prioritized purchase of carbon offsets"
            ) {
                // Find the offset in our registry
                let offsets = self.offsets.read().unwrap();
                if let Some(offset) = offsets.iter().find(|o| o.id == offset_id) {
                    purchases.push(EnvironmentalAssetPurchase {
                        asset_type: EnvironmentalAssetType::CarbonOffset,
                        amount: offset.amount_tons_co2e,
                        cost: offset.cost,
                        date: DateTime::<Utc>::from_naive_utc_and_offset(
                            NaiveDateTime::from_timestamp_opt(offset.timestamp as i64, 0).unwrap_or_default(),
                            Utc,
                        ),
                        provider: offset.provider.clone(),
                        reference: offset.id.clone(),
                        impact_score: 1.0,
                    });
                }
            }
        }
        
        Ok(purchases)
    }
}

// Add Default implementation for EnvironmentalTreasury
impl Default for EnvironmentalTreasury {
    fn default() -> Self {
        Self::new(
            2.0, // Default fee allocation percentage
            vec!["treasury_signer".to_string()], // Default signer
            1 // Default threshold
        )
    }
}

/// Generate a unique ID for treasury operations
fn generate_id() -> String {
    use rand::{thread_rng, Rng};
    let mut rng = thread_rng();
    let random_bytes: [u8; 16] = rng.gen();
    
    hex::encode(random_bytes)
}

/// Get current timestamp in seconds
fn current_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

impl EnvironmentalAssetType {
    fn to_string(&self) -> String {
        match self {
            Self::RenewableEnergyCertificate => "REC".to_string(),
            Self::CarbonOffset => "CARBON".to_string(),
        }
    }
}

/// Calculator for emissions based on network parameters
pub struct EmissionsCalculator {
    /// Current network hashrate
    pub hashrate: f64,
    /// Energy efficiency (Joules per terahash)
    pub energy_efficiency: f64,
    /// Carbon intensity (kg CO2e per kWh)
    pub carbon_intensity: f64,
    /// Renewable energy percentage
    pub renewable_percentage: f64,
}

impl EmissionsCalculator {
    /// Create a new emissions calculator with default values
    pub fn new() -> Self {
        Self {
            hashrate: 350.0, // Exahash per second
            energy_efficiency: 50.0, // J/TH
            carbon_intensity: 0.5, // kg CO2e/kWh
            renewable_percentage: 30.0, // 30% renewable
        }
    }
    
    /// Calculate daily emissions for the network
    pub fn calculate_daily_emissions(&self) -> f64 {
        // Convert hashrate from EH/s to TH/s
        let hashrate_th_s = self.hashrate * 1_000_000.0;
        
        // Calculate energy in joules per second (watts)
        let watts = hashrate_th_s * self.energy_efficiency;
        
        // Convert to kWh per day
        let kwh_per_day = watts * 24.0 / 1000.0;
        
        // Apply carbon intensity, considering renewable percentage
        let non_renewable_percentage = 100.0 - self.renewable_percentage;
        let emissions_kg = kwh_per_day * self.carbon_intensity * (non_renewable_percentage / 100.0);
        
        // Return tonnes of CO2e per day
        emissions_kg / 1000.0
    }
    
    /// Calculate network emissions
    pub fn calculate_network_emissions(&self) -> NetworkEmissions {
        // Calculate daily emissions
        let daily_emissions = self.calculate_daily_emissions();
        
        // Create the network emissions object
        NetworkEmissions {
            total_energy_mwh: self.calculate_total_energy_mwh(),
            total_emissions_tons_co2e: daily_emissions,
            renewable_percentage: self.renewable_percentage,
            emissions_per_tx: self.calculate_emissions_per_tx(),
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
        }
    }
    
    /// Calculate total energy consumption in MWh
    fn calculate_total_energy_mwh(&self) -> f64 {
        // Convert hashrate from EH/s to TH/s
        let hashrate_th_s = self.hashrate * 1_000_000.0;
        
        // Calculate energy in joules per second (watts)
        let watts = hashrate_th_s * self.energy_efficiency;
        
        // Convert to MWh per day
        watts * 24.0 / 1_000_000.0
    }
    
    /// Calculate emissions per transaction in kg CO2e
    fn calculate_emissions_per_tx(&self) -> f64 {
        // Assume 1,000,000 transactions per day
        let transactions_per_day = 1_000_000.0;
        
        // Calculate daily emissions in kg
        let daily_emissions_kg = self.calculate_daily_emissions() * 1000.0;
        
        // Return kg CO2e per transaction
        daily_emissions_kg / transactions_per_day
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_allocation_calculation() {
        let treasury = EnvironmentalTreasury::new(
            2.0, // 2% allocation
            vec!["signer1".to_string(), "signer2".to_string()],
            1,
        );
        
        let fee = 1000; // 1000 satoshis/wei/etc.
        let allocation = treasury.process_transaction_fees(fee).unwrap();
        
        // 2% of 1000 = 20
        assert_eq!(allocation, 20);
        
        // Test with fractional result
        let fee = 33;
        let allocation = treasury.process_transaction_fees(fee).unwrap();
        
        // 2% of 33 = 0.66, which should round down to 0 as u64
        assert_eq!(allocation, 0);
        
        // Test with larger values
        let fee = 1_000_000;
        let allocation = treasury.process_transaction_fees(fee).unwrap();
        
        // 2% of 1,000,000 = 20,000
        assert_eq!(allocation, 20_000);
    }
    
    #[test]
    fn test_green_miner_discount() {
        let mut treasury = EnvironmentalTreasury::new(
            2.0,
            vec!["signer1".to_string()],
            1,
        );
        
        // Register miners with different renewable percentages
        treasury.register_certificate(RenewableCertificate {
            id: "cert1".to_string(),
            provider: "miner1".to_string(),
            amount_mwh: 100.0,
            timestamp: current_timestamp(),
            description: "Cert1".to_string(),
            verification_status: true,
            cost: 10000,
        }).unwrap();
        
        treasury.register_certificate(RenewableCertificate {
            id: "cert2".to_string(),
            provider: "miner2".to_string(),
            amount_mwh: 60.0,
            timestamp: current_timestamp(),
            description: "Cert2".to_string(),
            verification_status: true,
            cost: 6000,
        }).unwrap();
        
        treasury.register_certificate(RenewableCertificate {
            id: "cert3".to_string(),
            provider: "miner3".to_string(),
            amount_mwh: 30.0,
            timestamp: current_timestamp(),
            description: "Cert3".to_string(),
            verification_status: true,
            cost: 3000,
        }).unwrap();
        
        treasury.register_certificate(RenewableCertificate {
            id: "cert4".to_string(),
            provider: "miner4".to_string(),
            amount_mwh: 10.0,
            timestamp: current_timestamp(),
            description: "Cert4".to_string(),
            verification_status: true,
            cost: 1000,
        }).unwrap();
        
        // Test discounts
        assert_eq!(treasury.calculate_miner_fee_discount("miner1"), 10.0); // 10% discount
        assert_eq!(treasury.calculate_miner_fee_discount("miner2"), 5.0);  // 5% discount
        assert_eq!(treasury.calculate_miner_fee_discount("miner3"), 2.0);  // 2% discount
        assert_eq!(treasury.calculate_miner_fee_discount("miner4"), 0.0);  // No discount
        assert_eq!(treasury.calculate_miner_fee_discount("nonexistent"), 0.0); // Nonexistent miner
    }
} 