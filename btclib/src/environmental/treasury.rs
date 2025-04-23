use std::collections::HashMap;
use thiserror::Error;
use serde::{Serialize, Deserialize};
use chrono::{DateTime, Utc};
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

impl EnvironmentalTreasury {
    /// Create a new environmental treasury
    pub fn new(
        config: TreasuryConfig,
        emissions_calculator: Arc<EmissionsCalculator>,
    ) -> Self {
        // Initialize accounts
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
        
        let treasury = Self {
            config: RwLock::new(config.clone()),
            accounts: RwLock::new(accounts),
            impact: RwLock::new(EnvironmentalImpact {
                target_ratio: config.target_carbon_negative_ratio,
                ..Default::default()
            }),
            certificates: RwLock::new(Vec::new()),
            offsets: RwLock::new(Vec::new()),
            emissions_calculator,
            current_fee_percentage: RwLock::new(config.base_fee_allocation_percentage),
        };
        
        // Initialize impact metrics
        treasury.update_environmental_impact();
        
        treasury
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
        // Get current network emissions
        let network_emissions = match self.emissions_calculator.calculate_network_emissions() {
            Ok(emissions) => emissions,
            Err(e) => {
                eprintln!("Failed to calculate network emissions: {}", e);
                return;
            }
        };
        
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
                let offset_account = self.accounts.read().unwrap()
                    .get(&TreasuryAccountType::CarbonOffsets)
                    .unwrap();
                    
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
        let cert_account = self.accounts.read().unwrap()
            .get(&TreasuryAccountType::RenewableCertificates)
            .unwrap();
            
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