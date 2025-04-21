use std::sync::Arc;
use crate::config::{Config, QuantumConfig, ZkpConfig, EnvironmentalConfig};
use crate::crypto::quantum::{QuantumScheme, QuantumKeyPair, QuantumParameters, QuantumError};
use crate::crypto::zkp::{ZkpParams, ZkpType, Commitment, ZeroKnowledgeProof};
use crate::types::transaction::{Transaction, TransactionInput, TransactionOutput};
use crate::types::extended_transaction::{
    QuantumTransaction, ConfidentialTransaction, 
    QuantumTransactionBuilder, ConfidentialTransactionBuilder
};
use crate::transaction_processor::{TransactionProcessor, TransactionType, TransactionProcessorError};
use crate::environmental::emissions::{EmissionsTracker, Emissions, EmissionsError, Region, HashRate, PoolId, PoolEnergyInfo};
use crate::environmental::treasury::{EnvironmentalTreasury, EnvironmentalAssetType, TreasuryError, VerificationInfo};
use crate::environmental::dashboard::{EnvironmentalDashboard, EnvironmentalMetrics, EmissionsTimePeriod, DashboardOptions};
use chrono::{DateTime, Utc};

/// Error types for the blockchain API
#[derive(Debug, thiserror::Error)]
pub enum ApiError {
    #[error("Quantum error: {0}")]
    Quantum(#[from] QuantumError),
    
    #[error("Transaction error: {0}")]
    Transaction(#[from] TransactionProcessorError),
    
    #[error("Emissions error: {0}")]
    Emissions(#[from] EmissionsError),
    
    #[error("Treasury error: {0}")]
    Treasury(#[from] TreasuryError),
    
    #[error("Configuration error: {0}")]
    Config(String),
    
    #[error("Feature not enabled: {0}")]
    FeatureNotEnabled(String),
}

/// High-level API for working with quantum signatures and confidential transactions
pub struct CryptoAPI {
    /// Global configuration
    config: Config,
    /// Emissions tracker for environmental features
    emissions_tracker: Option<EmissionsTracker>,
    /// Environmental treasury
    treasury: Option<EnvironmentalTreasury>,
    /// Environmental dashboard
    dashboard: Option<EnvironmentalDashboard>,
}

impl CryptoAPI {
    /// Create a new API instance with the given configuration
    pub fn new(config: Config) -> Self {
        let mut api = Self {
            config,
            emissions_tracker: None,
            treasury: None,
            dashboard: None,
        };
        
        // Initialize environmental components if enabled
        if api.config.environmental.enabled {
            api.init_environmental_components();
        }
        
        api
    }
    
    /// Initialize environmental components
    fn init_environmental_components(&mut self) {
        // Create emissions tracker
        let emissions_tracker = EmissionsTracker::new(self.config.environmental.emissions.clone());
        
        // Create treasury with default allocation percentage
        let treasury = EnvironmentalTreasury::new(
            self.config.environmental.treasury_allocation_percentage,
            vec![], // Empty authorized signers list for now
            1,      // Single signature required for now
        );
        
        // Create dashboard with the components
        let dashboard = EnvironmentalDashboard::new(
            emissions_tracker.clone(),
            treasury.clone(),
        );
        
        self.emissions_tracker = Some(emissions_tracker);
        self.treasury = Some(treasury);
        self.dashboard = Some(dashboard);
    }
    
    /// Generate a quantum-resistant key pair with the default settings
    pub fn generate_quantum_keypair<R: rand::CryptoRng + rand::RngCore>(
        &self,
        rng: &mut R,
    ) -> Result<QuantumKeyPair, QuantumError> {
        if !self.config.crypto.quantum.enabled {
            return Err(QuantumError::UnsupportedScheme);
        }
        
        let params = QuantumParameters {
            security_level: self.config.crypto.quantum.security_level,
            scheme: self.config.crypto.quantum.default_scheme,
            use_compression: false,
        };
        
        QuantumKeyPair::generate(self.config.crypto.quantum.default_scheme, Some(params))
    }
    
    /// Generate a quantum-resistant key pair with custom parameters
    pub fn generate_quantum_keypair_with_params<R: rand::CryptoRng + rand::RngCore>(
        &self,
        rng: &mut R,
        params: QuantumParameters,
    ) -> Result<QuantumKeyPair, QuantumError> {
        if !self.config.crypto.quantum.enabled {
            return Err(QuantumError::UnsupportedScheme);
        }
        
        QuantumKeyPair::generate(params.scheme, Some(params))
    }
    
    /// Sign a transaction using a quantum-resistant signature
    pub fn sign_quantum_transaction(
        &self,
        transaction: &Transaction,
        keypair: &QuantumKeyPair,
    ) -> Result<QuantumTransaction, QuantumError> {
        if !self.config.crypto.quantum.enabled {
            return Err(QuantumError::UnsupportedScheme);
        }
        
        // Get the transaction hash
        let tx_hash = transaction.hash();
        
        // Sign the transaction hash
        let signature = keypair.sign(&tx_hash)?;
        
        // Create the quantum transaction
        Ok(QuantumTransaction::new(
            transaction.clone(),
            keypair.parameters.scheme,
            keypair.parameters.security_level,
            signature,
        ))
    }
    
    /// Create a confidential transaction
    ///
    /// This method creates a confidential transaction that hides the output amounts,
    /// returning both the transaction and the blinding factors used.
    ///
    /// # Arguments
    /// * `inputs` - The transaction inputs
    /// * `outputs` - The transaction outputs as (amount, pub_key_script) pairs
    /// * `rng` - A cryptographically secure random number generator
    ///
    /// # Returns
    /// * `Result<(ConfidentialTransaction, Vec<Vec<u8>>), TransactionProcessorError>` - 
    ///   The confidential transaction and the blinding factors, or an error
    ///
    /// # Security considerations
    /// The returned blinding factors are critical secrets that must be stored securely.
    /// Loss of a blinding factor will prevent spending the corresponding output.
    pub fn create_confidential_transaction<R: rand::CryptoRng + rand::RngCore>(
        &self,
        inputs: Vec<TransactionInput>,
        outputs: Vec<(u64, Vec<u8>)>, // (amount, pub_key_script)
        rng: &mut R,
    ) -> Result<(ConfidentialTransaction, Vec<Vec<u8>>), TransactionProcessorError> {
        if !self.config.crypto.zkp.enabled {
            return Err(TransactionProcessorError::InvalidTransaction(
                "Confidential transactions are not enabled".to_string(),
            ));
        }
        
        if outputs.len() > self.config.crypto.zkp.max_range_proofs {
            return Err(TransactionProcessorError::InvalidTransaction(
                format!(
                    "Too many outputs: {} (max {})",
                    outputs.len(),
                    self.config.crypto.zkp.max_range_proofs
                ),
            ));
        }
        
        // Create ZKP parameters
        let zkp_params = ZkpParams {
            proof_type: self.config.crypto.zkp.default_scheme,
            security_level: self.config.crypto.zkp.security_level,
        };
        
        // Create a builder
        let builder = ConfidentialTransactionBuilder::new(zkp_params);
        
        // Create the transaction
        let result = builder.create_transaction(
            1, // version
            inputs,
            outputs,
            0, // lock_time
            rng,
        ).map_err(|e| TransactionProcessorError::InvalidTransaction(e.to_string()))?;
        
        Ok(result)
    }
    
    /// Verify a quantum transaction
    pub fn verify_quantum_transaction(
        &self,
        transaction: &QuantumTransaction,
        public_key: &[u8],
    ) -> Result<bool, QuantumError> {
        if !self.config.crypto.quantum.enabled {
            return Err(QuantumError::UnsupportedScheme);
        }
        
        transaction.verify_signature(public_key)
    }
    
    /// Verify a confidential transaction
    pub fn verify_confidential_transaction(
        &self,
        transaction: &ConfidentialTransaction,
    ) -> Result<bool, TransactionProcessorError> {
        if !self.config.crypto.zkp.enabled {
            return Err(TransactionProcessorError::InvalidTransaction(
                "Confidential transactions are not enabled".to_string(),
            ));
        }
        
        // Verify all range proofs
        if !transaction.verify_range_proofs() {
            return Err(TransactionProcessorError::InvalidRangeProof);
        }
        
        // Additional verification would be done here in a real implementation
        
        Ok(true)
    }
    
    /// Create a commitment to a value
    pub fn commit_to_value<R: rand::CryptoRng + rand::RngCore>(
        &self,
        value: u64,
        rng: &mut R,
    ) -> (Commitment, Vec<u8>) {
        if !self.config.crypto.zkp.enabled {
            // Return a dummy commitment if ZKP is disabled
            return (
                Commitment {
                    value: vec![0u8; 32],
                    commitment_type: crate::crypto::zkp::CommitmentType::Pedersen,
                },
                vec![0u8; 32],
            );
        }
        
        crate::crypto::zkp::commit_pedersen(value, rng)
    }
    
    /// Create a range proof for a value
    pub fn create_range_proof<R: rand::CryptoRng + rand::RngCore>(
        &self,
        value: u64,
        blinding_factor: &[u8],
        range_bits: u8,
        rng: &mut R,
    ) -> ZeroKnowledgeProof {
        if !self.config.crypto.zkp.enabled {
            // Return a dummy proof if ZKP is disabled
            return ZeroKnowledgeProof {
                proof_type: ZkpType::Bulletproof,
                proof: vec![0u8; 32],
                public_inputs: vec![],
            };
        }
        
        let params = ZkpParams {
            proof_type: self.config.crypto.zkp.default_scheme,
            security_level: self.config.crypto.zkp.security_level,
        };
        
        crate::crypto::zkp::create_range_proof(value, blinding_factor, range_bits, params, rng)
    }
    
    // ---------- Environmental API methods ----------
    
    /// Check if environmental features are enabled
    pub fn are_environmental_features_enabled(&self) -> bool {
        self.config.environmental.enabled
    }
    
    /// Get the emissions tracker
    pub fn get_emissions_tracker(&self) -> Result<&EmissionsTracker, ApiError> {
        if !self.config.environmental.enabled {
            return Err(ApiError::FeatureNotEnabled("Environmental features are not enabled".to_string()));
        }
        
        self.emissions_tracker.as_ref()
            .ok_or_else(|| ApiError::Config("Emissions tracker not initialized".to_string()))
    }
    
    /// Get a mutable reference to the emissions tracker
    pub fn get_emissions_tracker_mut(&mut self) -> Result<&mut EmissionsTracker, ApiError> {
        if !self.config.environmental.enabled {
            return Err(ApiError::FeatureNotEnabled("Environmental features are not enabled".to_string()));
        }
        
        self.emissions_tracker.as_mut()
            .ok_or_else(|| ApiError::Config("Emissions tracker not initialized".to_string()))
    }
    
    /// Get the environmental treasury
    pub fn get_treasury(&self) -> Result<&EnvironmentalTreasury, ApiError> {
        if !self.config.environmental.enabled {
            return Err(ApiError::FeatureNotEnabled("Environmental features are not enabled".to_string()));
        }
        
        self.treasury.as_ref()
            .ok_or_else(|| ApiError::Config("Environmental treasury not initialized".to_string()))
    }
    
    /// Get a mutable reference to the environmental treasury
    pub fn get_treasury_mut(&mut self) -> Result<&mut EnvironmentalTreasury, ApiError> {
        if !self.config.environmental.enabled {
            return Err(ApiError::FeatureNotEnabled("Environmental features are not enabled".to_string()));
        }
        
        self.treasury.as_mut()
            .ok_or_else(|| ApiError::Config("Environmental treasury not initialized".to_string()))
    }
    
    /// Get the environmental dashboard
    pub fn get_dashboard(&self) -> Result<&EnvironmentalDashboard, ApiError> {
        if !self.config.environmental.enabled {
            return Err(ApiError::FeatureNotEnabled("Environmental features are not enabled".to_string()));
        }
        
        self.dashboard.as_ref()
            .ok_or_else(|| ApiError::Config("Environmental dashboard not initialized".to_string()))
    }
    
    /// Get a mutable reference to the environmental dashboard
    pub fn get_dashboard_mut(&mut self) -> Result<&mut EnvironmentalDashboard, ApiError> {
        if !self.config.environmental.enabled {
            return Err(ApiError::FeatureNotEnabled("Environmental features are not enabled".to_string()));
        }
        
        self.dashboard.as_mut()
            .ok_or_else(|| ApiError::Config("Environmental dashboard not initialized".to_string()))
    }
    
    /// Calculate network emissions for a time period
    pub fn calculate_network_emissions(&self, start_time: DateTime<Utc>, end_time: DateTime<Utc>) -> Result<Emissions, ApiError> {
        let tracker = self.get_emissions_tracker()?;
        let emissions = tracker.calculate_network_emissions(start_time, end_time)?;
        Ok(emissions)
    }
    
    /// Estimate emissions for a transaction
    pub fn estimate_transaction_emissions(&self, transaction: &Transaction) -> Result<Emissions, ApiError> {
        let tracker = self.get_emissions_tracker()?;
        let emissions = tracker.estimate_transaction_emissions(transaction)?;
        Ok(emissions)
    }
    
    /// Update hashrate for a region
    pub fn update_region_hashrate(&mut self, country_code: &str, hashrate: f64) -> Result<(), ApiError> {
        let tracker = self.get_emissions_tracker_mut()?;
        
        let region = Region {
            country_code: country_code.to_string(),
            sub_region: None,
        };
        
        tracker.update_region_hashrate(region, HashRate(hashrate));
        
        Ok(())
    }
    
    /// Register a mining pool's energy information
    pub fn register_pool_energy_info(&mut self, pool_id: &str, renewable_percentage: f64, regions: Vec<String>, verified: bool) -> Result<(), ApiError> {
        let tracker = self.get_emissions_tracker_mut()?;
        
        // Create pool regions
        let pool_regions = regions.into_iter()
            .map(|code| Region {
                country_code: code,
                sub_region: None,
            })
            .collect();
        
        // Create the pool energy info
        let info = PoolEnergyInfo {
            renewable_percentage,
            verified,
            regions: pool_regions,
            last_updated: Utc::now(),
        };
        
        // Register the pool
        tracker.register_pool_energy_info(PoolId(pool_id.to_string()), info);
        
        Ok(())
    }
    
    /// Register a green miner in the treasury
    pub fn register_green_miner(&mut self, miner_id: &str, renewable_percentage: f64, verification_provider: Option<&str>) -> Result<(), ApiError> {
        let treasury = self.get_treasury_mut()?;
        
        // Create verification info if provider is specified
        let verification = verification_provider.map(|provider| {
            VerificationInfo {
                provider: provider.to_string(),
                date: Utc::now(),
                reference: format!("REF-{}-{}", miner_id, Utc::now().timestamp()),
                status: crate::environmental::treasury::VerificationStatus::Pending,
            }
        });
        
        // Register the miner
        treasury.register_green_miner(
            miner_id.to_string(),
            renewable_percentage,
            verification,
        )?;
        
        Ok(())
    }
    
    /// Get fee discount for a green miner
    pub fn get_green_miner_fee_discount(&self, miner_id: &str) -> Result<f64, ApiError> {
        if !self.config.environmental.enable_green_miner_discounts {
            return Ok(0.0); // No discounts if the feature is disabled
        }
        
        let treasury = self.get_treasury()?;
        let discount = treasury.calculate_miner_fee_discount(miner_id);
        
        Ok(discount)
    }
    
    /// Process a block's allocation to the environmental treasury
    pub fn process_block_environmental_allocation(&mut self, total_fees: u64) -> Result<u64, ApiError> {
        let treasury = self.get_treasury_mut()?;
        let allocation = treasury.process_block_allocation(total_fees);
        
        Ok(allocation)
    }
    
    /// Purchase environmental assets with REC prioritization
    pub fn purchase_environmental_assets(&mut self, amount: u64) -> Result<Vec<EnvironmentalAssetPurchase>, ApiError> {
        let treasury = self.get_treasury_mut()?;
        let rec_allocation_percentage = self.config.environmental.rec_allocation_percentage;
        
        let purchases = treasury.purchase_prioritized_assets(amount, rec_allocation_percentage)?;
        Ok(purchases)
    }
    
    /// Get the current REC prioritization settings
    pub fn get_rec_prioritization_settings(&self) -> Result<(f64, f64), ApiError> {
        if !self.config.environmental.enabled {
            return Err(ApiError::FeatureNotEnabled("Environmental features are not enabled".to_string()));
        }
        
        Ok((
            self.config.environmental.rec_priority_factor,
            self.config.environmental.rec_allocation_percentage
        ))
    }
    
    /// Update REC prioritization settings
    pub fn update_rec_prioritization(&mut self, priority_factor: f64, allocation_percentage: f64) -> Result<(), ApiError> {
        if !self.config.environmental.enabled {
            return Err(ApiError::FeatureNotEnabled("Environmental features are not enabled".to_string()));
        }
        
        // Validate inputs
        if priority_factor <= 0.0 {
            return Err(ApiError::Config("Priority factor must be positive".to_string()));
        }
        
        if allocation_percentage < 0.0 || allocation_percentage > 100.0 {
            return Err(ApiError::Config("Allocation percentage must be between 0 and 100".to_string()));
        }
        
        // Update configuration
        self.config.environmental.rec_priority_factor = priority_factor;
        self.config.environmental.rec_allocation_percentage = allocation_percentage;
        
        Ok(())
    }
    
    /// Generate environmental metrics for a time period
    pub fn generate_environmental_metrics(&mut self, period: EmissionsTimePeriod, transaction_count: u64) -> Result<EnvironmentalMetrics, ApiError> {
        let dashboard = self.get_dashboard_mut()?;
        let metrics = dashboard.generate_metrics(period, transaction_count)
            .map_err(|e| ApiError::Config(e))?;
            
        Ok(metrics)
    }
    
    /// Generate an environmental report
    pub fn generate_environmental_report(&self, period: EmissionsTimePeriod) -> Result<String, ApiError> {
        let dashboard = self.get_dashboard()?;
        let report = dashboard.generate_text_report(period)
            .map_err(|e| ApiError::Config(e))?;
            
        Ok(report)
    }
    
    /// Export environmental metrics as JSON
    pub fn export_environmental_metrics_json(&self, period: EmissionsTimePeriod) -> Result<String, ApiError> {
        let dashboard = self.get_dashboard()?;
        let json = dashboard.export_metrics_json(period)
            .map_err(|e| ApiError::Config(e))?;
            
        Ok(json)
    }
}

/// Create a high-level API with default settings
pub fn create_default_api() -> CryptoAPI {
    CryptoAPI::new(Config::default())
}

/// Create a high-level API for testnet
pub fn create_testnet_api() -> CryptoAPI {
    CryptoAPI::new(Config::testnet())
}

/// Create a high-level API for regtest
pub fn create_regtest_api() -> CryptoAPI {
    CryptoAPI::new(Config::regtest())
}

/// Create a high-level API with environmental features enabled
pub fn create_environmental_api() -> CryptoAPI {
    CryptoAPI::new(Config::with_environmental_features())
} 