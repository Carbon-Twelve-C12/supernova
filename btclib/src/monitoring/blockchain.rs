use prometheus::{
    Registry, IntGauge, IntGaugeVec, IntCounter, IntCounterVec, 
    Gauge, GaugeVec, Histogram, HistogramVec, Opts
};
use std::sync::Arc;
use std::time::Duration;
use crate::monitoring::MetricsError;
use tracing::{info, warn, error, debug};

/// Blockchain metrics collector
pub struct BlockchainMetrics {
    /// Current blockchain height
    height: IntGauge,
    /// Transactions per block
    transactions_per_block: HistogramVec,
    /// Block time in seconds
    block_time: Histogram,
    /// Block size in bytes
    block_size: Histogram,
    /// Difficulty
    difficulty: Gauge,
    /// Total transactions
    total_transactions: IntCounter,
    /// Transaction counts by type
    transaction_counts: IntCounterVec,
    /// Transaction fees
    transaction_fees: GaugeVec,
    /// Transaction size in bytes
    transaction_size: Histogram,
    /// Block propagation time
    block_propagation_time: Histogram,
    /// Orphaned blocks count
    orphaned_blocks: IntCounter,
    /// Reorg depth tracking
    reorg_depths: IntCounterVec,
    /// Number of unconfirmed transactions
    unconfirmed_transactions: IntGauge,
    /// UTXO set size
    utxo_set_size: IntGauge,
    /// Chain state database size
    chain_db_size: IntGauge,
    /// Environmental metrics
    environmental_metrics: Option<EnvironmentalMetrics>,
}

/// Environmental metrics for blockchain
pub struct EnvironmentalMetrics {
    /// Estimated energy consumption in kWh
    energy_consumption: Gauge,
    /// Estimated carbon emissions in kgCO2e
    carbon_emissions: Gauge,
    /// Percentage of renewable energy
    renewable_percentage: Gauge,
    /// Environmental treasury balance
    environmental_treasury: Gauge,
}

impl BlockchainMetrics {
    /// Create a new blockchain metrics collector
    pub fn new(registry: &Registry, namespace: &str) -> Result<Self, MetricsError> {
        // Current blockchain height
        let height = IntGauge::new(
            Opts::new("height", "Current blockchain height")
                .namespace(namespace.to_string())
                .subsystem("blockchain"),
        )?;
        registry.register(Box::new(height.clone()))?;
        
        // Transactions per block
        let transactions_per_block = HistogramVec::new(
            Opts::new("transactions_per_block", "Transactions per block")
                .namespace(namespace.to_string())
                .subsystem("blockchain"),
            &["type"],
            vec![1.0, 5.0, 10.0, 50.0, 100.0, 500.0, 1000.0, 2000.0, 5000.0],
        )?;
        registry.register(Box::new(transactions_per_block.clone()))?;
        
        // Block time in seconds
        let block_time = Histogram::new(
            Opts::new("block_time_seconds", "Block time in seconds")
                .namespace(namespace.to_string())
                .subsystem("blockchain"),
            vec![5.0, 10.0, 30.0, 60.0, 300.0, 600.0, 1800.0, 3600.0],
        )?;
        registry.register(Box::new(block_time.clone()))?;
        
        // Block size in bytes
        let block_size = Histogram::new(
            Opts::new("block_size_bytes", "Block size in bytes")
                .namespace(namespace.to_string())
                .subsystem("blockchain"),
            vec![
                10_000.0, 50_000.0, 100_000.0, 500_000.0, 1_000_000.0, 
                2_000_000.0, 5_000_000.0, 10_000_000.0
            ],
        )?;
        registry.register(Box::new(block_size.clone()))?;
        
        // Difficulty
        let difficulty = Gauge::new(
            Opts::new("difficulty", "Mining difficulty")
                .namespace(namespace.to_string())
                .subsystem("blockchain"),
        )?;
        registry.register(Box::new(difficulty.clone()))?;
        
        // Total transactions
        let total_transactions = IntCounter::new(
            Opts::new("total_transactions", "Total number of transactions")
                .namespace(namespace.to_string())
                .subsystem("blockchain"),
        )?;
        registry.register(Box::new(total_transactions.clone()))?;
        
        // Transaction counts by type
        let transaction_counts = IntCounterVec::new(
            Opts::new("transaction_count", "Transaction count by type")
                .namespace(namespace.to_string())
                .subsystem("blockchain"),
            &["type"],
        )?;
        registry.register(Box::new(transaction_counts.clone()))?;
        
        // Transaction fees
        let transaction_fees = GaugeVec::new(
            Opts::new("transaction_fees", "Transaction fees information")
                .namespace(namespace.to_string())
                .subsystem("blockchain"),
            &["statistic"],
        )?;
        registry.register(Box::new(transaction_fees.clone()))?;
        
        // Transaction size
        let transaction_size = Histogram::new(
            Opts::new("transaction_size_bytes", "Transaction size in bytes")
                .namespace(namespace.to_string())
                .subsystem("blockchain"),
            vec![100.0, 250.0, 500.0, 1000.0, 2500.0, 5000.0, 10000.0, 25000.0],
        )?;
        registry.register(Box::new(transaction_size.clone()))?;
        
        // Block propagation time
        let block_propagation_time = Histogram::new(
            Opts::new("block_propagation_ms", "Block propagation time in milliseconds")
                .namespace(namespace.to_string())
                .subsystem("blockchain"),
            vec![
                50.0, 100.0, 200.0, 500.0, 1000.0, 2000.0, 5000.0, 10000.0, 30000.0
            ],
        )?;
        registry.register(Box::new(block_propagation_time.clone()))?;
        
        // Orphaned blocks
        let orphaned_blocks = IntCounter::new(
            Opts::new("orphaned_blocks", "Number of orphaned blocks")
                .namespace(namespace.to_string())
                .subsystem("blockchain"),
        )?;
        registry.register(Box::new(orphaned_blocks.clone()))?;
        
        // Reorg depths
        let reorg_depths = IntCounterVec::new(
            Opts::new("reorg_depths", "Chain reorganization depths")
                .namespace(namespace.to_string())
                .subsystem("blockchain"),
            &["depth"],
        )?;
        registry.register(Box::new(reorg_depths.clone()))?;
        
        // Unconfirmed transactions
        let unconfirmed_transactions = IntGauge::new(
            Opts::new("unconfirmed_transactions", "Number of unconfirmed transactions")
                .namespace(namespace.to_string())
                .subsystem("blockchain"),
        )?;
        registry.register(Box::new(unconfirmed_transactions.clone()))?;
        
        // UTXO set size
        let utxo_set_size = IntGauge::new(
            Opts::new("utxo_set_size", "Size of the UTXO set")
                .namespace(namespace.to_string())
                .subsystem("blockchain"),
        )?;
        registry.register(Box::new(utxo_set_size.clone()))?;
        
        // Chain DB size
        let chain_db_size = IntGauge::new(
            Opts::new("chain_db_size_bytes", "Size of the blockchain database in bytes")
                .namespace(namespace.to_string())
                .subsystem("blockchain"),
        )?;
        registry.register(Box::new(chain_db_size.clone()))?;
        
        // Environmental metrics
        let environmental_metrics = Some(Self::setup_environmental_metrics(registry, namespace)?);
        
        Ok(Self {
            height,
            transactions_per_block,
            block_time,
            block_size,
            difficulty,
            total_transactions,
            transaction_counts,
            transaction_fees,
            transaction_size,
            block_propagation_time,
            orphaned_blocks,
            reorg_depths,
            unconfirmed_transactions,
            utxo_set_size,
            chain_db_size,
            environmental_metrics,
        })
    }
    
    /// Set up environmental metrics
    fn setup_environmental_metrics(registry: &Registry, namespace: &str) -> Result<EnvironmentalMetrics, MetricsError> {
        // Energy consumption
        let energy_consumption = Gauge::new(
            Opts::new("energy_consumption_kwh", "Estimated energy consumption in kWh")
                .namespace(namespace.to_string())
                .subsystem("environmental"),
        )?;
        registry.register(Box::new(energy_consumption.clone()))?;
        
        // Carbon emissions
        let carbon_emissions = Gauge::new(
            Opts::new("carbon_emissions_kgco2e", "Estimated carbon emissions in kgCO2e")
                .namespace(namespace.to_string())
                .subsystem("environmental"),
        )?;
        registry.register(Box::new(carbon_emissions.clone()))?;
        
        // Renewable percentage
        let renewable_percentage = Gauge::new(
            Opts::new("renewable_percentage", "Percentage of renewable energy")
                .namespace(namespace.to_string())
                .subsystem("environmental"),
        )?;
        registry.register(Box::new(renewable_percentage.clone()))?;
        
        // Environmental treasury
        let environmental_treasury = Gauge::new(
            Opts::new("environmental_treasury", "Environmental treasury balance")
                .namespace(namespace.to_string())
                .subsystem("environmental"),
        )?;
        registry.register(Box::new(environmental_treasury.clone()))?;
        
        Ok(EnvironmentalMetrics {
            energy_consumption,
            carbon_emissions,
            renewable_percentage,
            environmental_treasury,
        })
    }
    
    /// Update the blockchain height
    pub fn set_height(&self, height: i64) {
        self.height.set(height);
    }
    
    /// Update the difficulty
    pub fn set_difficulty(&self, difficulty: f64) {
        self.difficulty.set(difficulty);
    }
    
    /// Register a new block
    pub fn register_block(
        &self,
        height: i64,
        tx_count: usize,
        size: usize,
        time_since_last_block: Duration,
        propagation_time: Duration,
    ) {
        // Update height
        self.height.set(height);
        
        // Update transactions per block
        self.transactions_per_block
            .with_label_values(&["all"])
            .observe(tx_count as f64);
        
        // Update block time
        self.block_time.observe(time_since_last_block.as_secs_f64());
        
        // Update block size
        self.block_size.observe(size as f64);
        
        // Update block propagation time
        self.block_propagation_time.observe(propagation_time.as_millis() as f64);
        
        debug!("Registered block metrics for height {}", height);
    }
    
    /// Register a new transaction
    pub fn register_transaction(
        &self,
        tx_type: &str,
        size: usize,
        fee: f64,
    ) {
        // Increment total transaction count
        self.total_transactions.inc();
        
        // Increment transaction count for this type
        self.transaction_counts
            .with_label_values(&[tx_type])
            .inc();
        
        // Observe transaction size
        self.transaction_size.observe(size as f64);
        
        // Update transaction fee metrics
        self.update_fee_metrics(fee);
        
        debug!("Registered transaction metrics for type {}", tx_type);
    }
    
    /// Update transaction fee metrics
    fn update_fee_metrics(&self, fee: f64) {
        // This is a simplified approach; in practice you might
        // use a more sophisticated algorithm to maintain moving averages
        
        // Get current metrics
        let current_min = self.transaction_fees.with_label_values(&["min"]).get();
        let current_max = self.transaction_fees.with_label_values(&["max"]).get();
        
        // Update min fee if this is the first fee or lower than current min
        if current_min == 0.0 || fee < current_min {
            self.transaction_fees.with_label_values(&["min"]).set(fee);
        }
        
        // Update max fee if higher than current max
        if fee > current_max {
            self.transaction_fees.with_label_values(&["max"]).set(fee);
        }
        
        // For average, we'd need to maintain state elsewhere
        // This is just a placeholder
        self.transaction_fees.with_label_values(&["last"]).set(fee);
    }
    
    /// Register a chain reorganization
    pub fn register_reorg(&self, depth: usize) {
        let depth_label = if depth <= 5 {
            depth.to_string()
        } else if depth <= 10 {
            "6_to_10".to_string()
        } else if depth <= 20 {
            "11_to_20".to_string()
        } else {
            "over_20".to_string()
        };
        
        // Increment reorg counter for this depth
        self.reorg_depths
            .with_label_values(&[&depth_label])
            .inc();
        
        info!("Registered chain reorganization with depth {}", depth);
    }
    
    /// Register an orphaned block
    pub fn register_orphaned_block(&self) {
        self.orphaned_blocks.inc();
    }
    
    /// Update the number of unconfirmed transactions
    pub fn set_unconfirmed_transactions(&self, count: i64) {
        self.unconfirmed_transactions.set(count);
    }
    
    /// Update the UTXO set size
    pub fn set_utxo_set_size(&self, size: i64) {
        self.utxo_set_size.set(size);
    }
    
    /// Update the chain database size
    pub fn set_chain_db_size(&self, size_bytes: i64) {
        self.chain_db_size.set(size_bytes);
    }
    
    /// Update environmental metrics
    pub fn update_environmental_metrics(
        &self,
        energy_kwh: f64,
        emissions_kgco2e: f64,
        renewable_percent: f64,
        treasury_balance: f64,
    ) {
        if let Some(env_metrics) = &self.environmental_metrics {
            env_metrics.energy_consumption.set(energy_kwh);
            env_metrics.carbon_emissions.set(emissions_kgco2e);
            env_metrics.renewable_percentage.set(renewable_percent);
            env_metrics.environmental_treasury.set(treasury_balance);
            
            debug!("Updated environmental metrics");
        }
    }
    
    /// Get blockchain metrics as a formatted string
    pub fn get_summary(&self) -> String {
        let mut summary = String::new();
        summary.push_str("=== Blockchain Metrics Summary ===\n");
        
        // Basic chain information
        summary.push_str(&format!("Height: {}\n", self.height.get()));
        summary.push_str(&format!("Difficulty: {:.2e}\n", self.difficulty.get()));
        summary.push_str(&format!("Total Transactions: {}\n", self.total_transactions.get()));
        summary.push_str(&format!("Unconfirmed Transactions: {}\n", self.unconfirmed_transactions.get()));
        summary.push_str(&format!("UTXO Set Size: {}\n", self.utxo_set_size.get()));
        summary.push_str(&format!("Chain Database Size: {:.2} GB\n", self.chain_db_size.get() as f64 / 1_000_000_000.0));
        summary.push_str(&format!("Orphaned Blocks: {}\n", self.orphaned_blocks.get()));
        
        // Environmental information if available
        if let Some(env_metrics) = &self.environmental_metrics {
            summary.push_str("\nEnvironmental Metrics:\n");
            summary.push_str(&format!("  Energy Consumption: {:.2} kWh\n", env_metrics.energy_consumption.get()));
            summary.push_str(&format!("  Carbon Emissions: {:.2} kgCO2e\n", env_metrics.carbon_emissions.get()));
            summary.push_str(&format!("  Renewable Energy: {:.1}%\n", env_metrics.renewable_percentage.get()));
            summary.push_str(&format!("  Environmental Treasury: {:.8}\n", env_metrics.environmental_treasury.get()));
        }
        
        summary
    }
} 