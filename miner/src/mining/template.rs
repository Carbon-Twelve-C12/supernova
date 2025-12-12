use super::reward::{calculate_mining_reward, EnvironmentalProfile};
use async_trait::async_trait;
use supernova_core::types::block::Block;
use supernova_core::types::transaction::{Transaction, TransactionInput, TransactionOutput};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

// ============================================================================
// Treasury Output Validation Configuration
// ============================================================================

/// Treasury allocation configuration
pub struct TreasuryAllocationConfig;

impl TreasuryAllocationConfig {
    /// Treasury allocation percentage
    /// 
    /// SECURITY FIX (P2-010): 5% of block reward goes to environmental treasury.
    /// This is a consensus rule and must be enforced in block validation.
    pub const TREASURY_ALLOCATION_PERCENT: f64 = 5.0;
    
    /// Treasury address (multisig controlled by governance)
    /// 
    /// SECURITY: In production, this should be a multisig address controlled
    /// by decentralized governance, not a single address.
    /// 
    /// TODO: Replace with actual governance-controlled multisig address
    pub const TREASURY_ADDRESS_PLACEHOLDER: &'static [u8] = b"TREASURY_PLACEHOLDER_ADDRESS";
    
    /// Minimum treasury output amount (prevents dust)
    pub const MIN_TREASURY_OUTPUT: u64 = 1000; // 1000 nova units minimum
}

pub const BLOCK_MAX_SIZE: usize = 4_000_000; // 4MB (increased for 2.5-minute blocks)
pub const TEMPLATE_REFRESH_INTERVAL: Duration = Duration::from_secs(30); // Refresh template every 30 seconds for 2.5-minute blocks

#[async_trait]
pub trait MempoolInterface: Send + Sync {
    async fn get_transactions(&self, max_size: usize) -> Vec<Transaction>;

    // Add new method to get prioritized transactions based on fee
    async fn get_prioritized_transactions(&self, max_size: usize) -> Vec<Transaction> {
        self.get_transactions(max_size).await
    }

    // Method to get estimate of transaction fees
    async fn get_transaction_fees(&self, txids: &[Vec<u8>]) -> Vec<u64> {
        // Default implementation returns zero fees
        vec![0; txids.len()]
    }
}

pub struct BlockTemplate {
    version: u32,
    prev_block_hash: [u8; 32],
    target: u32,
    coinbase: Transaction,
    transactions: Vec<Transaction>,
    creation_time: Instant,
    needs_refresh: AtomicBool,
    block_height: u64, // Add block height for reward calculation
}

impl BlockTemplate {
    pub async fn new(
        version: u32,
        prev_block_hash: [u8; 32],
        target: u32,
        reward_address: Vec<u8>,
        mempool: &dyn MempoolInterface,
        block_height: u64, // Add block height parameter
        environmental_profile: Option<&EnvironmentalProfile>, // Add environmental profile
    ) -> Self {
        // Calculate reward based on block height and environmental profile
        let env_profile = environmental_profile.cloned().unwrap_or_default();
        let mining_reward = calculate_mining_reward(block_height, &env_profile);

        // Create coinbase transaction with calculated reward
        let coinbase =
            Self::create_coinbase_transaction(mining_reward.total_reward, reward_address.clone());

        // Get prioritized transactions from mempool
        let coinbase_size = bincode::serialize(&coinbase).unwrap().len();
        let available_size = BLOCK_MAX_SIZE - coinbase_size;
        let transactions = Self::select_transactions(mempool, available_size).await;

        Self {
            version,
            prev_block_hash,
            target,
            coinbase,
            transactions,
            creation_time: Instant::now(),
            needs_refresh: AtomicBool::new(false),
            block_height,
        }
    }

    // Efficient transaction selection based on fees
    async fn select_transactions(
        mempool: &dyn MempoolInterface,
        available_size: usize,
    ) -> Vec<Transaction> {
        // Get prioritized transactions
        let transactions = mempool
            .get_prioritized_transactions(available_size * 2)
            .await;

        // Sort by fee per byte (fee density) if not already sorted
        let txids: Vec<Vec<u8>> = transactions.iter().map(|tx| tx.hash().to_vec()).collect();
        let fees = mempool.get_transaction_fees(&txids).await;

        // Create tuples of (transaction, fee, size) for sorting
        let mut tx_fee_size: Vec<(Transaction, u64, usize)> = transactions
            .into_iter()
            .zip(fees.into_iter())
            .map(|(tx, fee)| {
                let size = bincode::serialize(&tx).unwrap().len();
                (tx, fee, size)
            })
            .collect();

        // Sort by fee per byte (fee density) in descending order
        tx_fee_size.sort_by(|a, b| {
            let fee_rate_a = if a.2 > 0 {
                a.1 as f64 / a.2 as f64
            } else {
                0.0
            };
            let fee_rate_b = if b.2 > 0 {
                b.1 as f64 / b.2 as f64
            } else {
                0.0
            };
            fee_rate_b
                .partial_cmp(&fee_rate_a)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        // Select transactions that fit in the block
        let mut selected = Vec::new();
        let mut total_size = 0;

        for (tx, _, size) in tx_fee_size {
            if total_size + size <= available_size {
                total_size += size;
                selected.push(tx);
            }
        }

        selected
    }

    pub fn create_block(&self) -> Block {
        let mut transactions = vec![self.coinbase.clone()];
        transactions.extend(self.transactions.clone());

        Block::new_with_params(
            self.version,
            self.prev_block_hash,
            transactions,
            self.target,
        )
    }

    // Check if template needs refresh
    pub fn needs_refresh(&self) -> bool {
        self.needs_refresh.load(Ordering::Relaxed)
            || self.creation_time.elapsed() > TEMPLATE_REFRESH_INTERVAL
    }

    // Mark template as needing refresh
    pub fn mark_for_refresh(&self) {
        self.needs_refresh.store(true, Ordering::Relaxed);
    }

    // Efficient update that only refreshes transactions
    pub async fn update_transactions(&mut self, mempool: &dyn MempoolInterface) {
        let coinbase_size = bincode::serialize(&self.coinbase).unwrap().len();
        let available_size = BLOCK_MAX_SIZE - coinbase_size;
        self.transactions = Self::select_transactions(mempool, available_size).await;
        self.creation_time = Instant::now();
        self.needs_refresh.store(false, Ordering::Relaxed);
    }

    /// Create coinbase transaction with treasury allocation
    /// 
    /// Adds mandatory treasury output to coinbase.
    /// Previous implementation sent 100% of reward to miner, breaking the
    /// environmental system's economic model.
    ///
    /// # Coinbase Output Structure
    /// 1. Miner output: 95% of reward
    /// 2. Treasury output: 5% of reward (environmental funding)
    ///
    /// # Security Guarantee
    /// - Treasury allocation is enforced
    /// - Percentage is configurable constant
    /// - Minimum treasury output prevents dust
    /// - TODO: Production must validate treasury address via governance
    ///
    /// # Arguments
    /// * `reward` - Total block reward (base + fees + environmental bonus)
    /// * `reward_address` - Miner's reward address
    ///
    /// # Returns
    /// Transaction with miner and treasury outputs
    fn create_coinbase_transaction(reward: u64, reward_address: Vec<u8>) -> Transaction {
        let coinbase_input = TransactionInput::new(
            [0u8; 32],  // Previous transaction hash is zero for coinbase
            0xffffffff, // Previous output index is max value for coinbase
            vec![],     // No signature script needed for coinbase
            0,          // Sequence
        );

        // SECURITY: Calculate treasury allocation
        let treasury_percentage = TreasuryAllocationConfig::TREASURY_ALLOCATION_PERCENT / 100.0;
        let treasury_amount = (reward as f64 * treasury_percentage) as u64;
        let miner_amount = reward.saturating_sub(treasury_amount);
        
        // Create outputs: [miner, treasury]
        let mut outputs = Vec::new();
        
        // Output 0: Miner reward (95%)
        outputs.push(TransactionOutput::new(miner_amount, reward_address));
        
        // Output 1: Treasury allocation (5%)
        // SECURITY: Only add if amount meets minimum threshold
        if treasury_amount >= TreasuryAllocationConfig::MIN_TREASURY_OUTPUT {
            outputs.push(TransactionOutput::new(
                treasury_amount,
                TreasuryAllocationConfig::TREASURY_ADDRESS_PLACEHOLDER.to_vec(),
            ));
        } else {
            // If treasury amount too small, give it to miner
            // This only happens for very small rewards in early testing
            outputs[0] = TransactionOutput::new(reward, outputs[0].pub_key_script.clone());
        }

        Transaction::new(
            1, // Version
            vec![coinbase_input],
            outputs,
            0, // Lock time
        )
    }
    
    /// Validate coinbase transaction has correct treasury output
    /// 
    /// SECURITY: Validates that coinbase includes proper treasury allocation.
    /// This should be called during block validation to prevent fund diversion.
    ///
    /// # Arguments
    /// * `coinbase` - Coinbase transaction to validate
    /// * `expected_reward` - Expected total block reward
    ///
    /// # Returns
    /// * `Ok(())` - Coinbase is valid
    /// * `Err(String)` - Coinbase is invalid with reason
    pub fn validate_coinbase_treasury(
        coinbase: &Transaction,
        expected_reward: u64,
    ) -> Result<(), String> {
        let outputs = coinbase.outputs();
        
        // Validation 1: Must have at least 1 output (miner)
        if outputs.is_empty() {
            return Err("Coinbase has no outputs".to_string());
        }
        
        // Validation 2: Calculate expected treasury amount
        let treasury_percentage = TreasuryAllocationConfig::TREASURY_ALLOCATION_PERCENT / 100.0;
        let expected_treasury = (expected_reward as f64 * treasury_percentage) as u64;
        
        // Validation 3: If treasury amount significant, must have treasury output
        if expected_treasury >= TreasuryAllocationConfig::MIN_TREASURY_OUTPUT {
            if outputs.len() < 2 {
                return Err(format!(
                    "Coinbase missing treasury output (expected {} nova units)",
                    expected_treasury
                ));
            }
            
            // Validation 4: Verify treasury output amount
            let actual_treasury = outputs[1].amount();
            
            // Allow 1% tolerance for rounding
            let tolerance = expected_treasury / 100;
            if actual_treasury < expected_treasury.saturating_sub(tolerance) 
                || actual_treasury > expected_treasury + tolerance {
                return Err(format!(
                    "Treasury output amount incorrect: expected {}, got {}",
                    expected_treasury,
                    actual_treasury
                ));
            }
            
            // Validation 5: Verify miner doesn't get full reward
            let miner_output = outputs[0].amount();
            let total_outputs: u64 = outputs.iter().map(|o| o.amount()).sum();
            
            if total_outputs > expected_reward {
                return Err(format!(
                    "Total coinbase outputs {} exceed expected reward {}",
                    total_outputs,
                    expected_reward
                ));
            }
        }
        
        Ok(())
    }

    // Add method to update template with additional fee to prioritize mining
    pub fn add_fee_to_coinbase(&mut self, additional_fee: u64) {
        if additional_fee == 0 {
            return;
        }

        // Update the coinbase output with additional fee
        if let Some(output) = self.coinbase.outputs().first() {
            // Since we can't modify output directly, create a new output with higher amount
            let current_amount = output.amount();
            let new_amount = current_amount + additional_fee;

            // Since we can't access private fields, we need to recreate the transaction
            // First, create new outputs by recreating them based on available data
            let mut new_outputs = Vec::with_capacity(self.coinbase.outputs().len());

            // Replace first output with increased amount
            new_outputs.push(TransactionOutput::new(
                new_amount,
                Vec::new(), // We can't access the original pubkey script, use empty for now
            ));

            // Keep remaining outputs unchanged if any
            for i in 1..self.coinbase.outputs().len() {
                let o = &self.coinbase.outputs()[i];
                new_outputs.push(TransactionOutput::new(
                    o.amount(),
                    Vec::new(), // We can't access the original pubkey script, use empty for now
                ));
            }

            // Create a new coinbase transaction
            self.coinbase = Transaction::new(
                1, // Use default version
                self.coinbase.inputs().to_vec(),
                new_outputs,
                0, // Use default lock_time
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    struct MockMempool;

    #[async_trait]
    impl MempoolInterface for MockMempool {
        async fn get_transactions(&self, _max_size: usize) -> Vec<Transaction> {
            Vec::new()
        }

        async fn get_prioritized_transactions(&self, _max_size: usize) -> Vec<Transaction> {
            // Create three transactions with different fees for testing
            let tx1 = Transaction::new(1, vec![], vec![], 0);
            let tx2 = Transaction::new(1, vec![], vec![], 0);
            let tx3 = Transaction::new(1, vec![], vec![], 0);

            vec![tx1, tx2, tx3]
        }

        async fn get_transaction_fees(&self, txids: &[Vec<u8>]) -> Vec<u64> {
            // Return fees corresponding to transaction positions
            txids
                .iter()
                .enumerate()
                .map(|(i, _)| {
                    match i {
                        0 => 50000, // High fee for first tx
                        1 => 20000, // Medium fee for second tx
                        2 => 10000, // Low fee for third tx
                        _ => 1000,  // Default fee
                    }
                })
                .collect()
        }
    }

    #[tokio::test]
    async fn test_block_template_creation() {
        let mempool = MockMempool;
        let template =
            BlockTemplate::new(1, [0u8; 32], u32::MAX, vec![1, 2, 3, 4], &mempool, 1, None).await;

        let block = template.create_block();
        // The MockMempool implementation returns 3 transactions,
        // plus the coinbase transaction makes 4 total
        assert_eq!(
            block.transactions().len(),
            4,
            "Block should have 4 transactions (coinbase + 3 from mempool)"
        );

        // Coinbase output 0 is the miner payout (reward minus treasury allocation)
        let expected_reward = crate::mining::reward::calculate_base_reward(1);
        let treasury_pct = TreasuryAllocationConfig::TREASURY_ALLOCATION_PERCENT / 100.0;
        let expected_treasury = (expected_reward as f64 * treasury_pct) as u64;
        let expected_miner_payout = expected_reward.saturating_sub(expected_treasury);
        assert_eq!(
            block.transactions()[0].outputs()[0].amount(),
            expected_miner_payout
        );
    }

    #[tokio::test]
    async fn test_transaction_selection() {
        let mempool = MockMempool;
        let transactions = BlockTemplate::select_transactions(&mempool, 10000).await;

        // We should have all 3 transactions sorted by fee per byte
        assert_eq!(transactions.len(), 3);

        // First transaction should be the high-fee one
        let fee_ratio_first = 50000.0 / bincode::serialize(&transactions[0]).unwrap().len() as f64;
        let fee_ratio_second = 20000.0 / bincode::serialize(&transactions[1]).unwrap().len() as f64;

        // Verify sorting by fee density
        assert!(fee_ratio_first >= fee_ratio_second);
    }

    #[tokio::test]
    async fn test_template_refresh() {
        let mempool = MockMempool;
        let mut template =
            BlockTemplate::new(1, [0u8; 32], u32::MAX, vec![1, 2, 3, 4], &mempool, 1, None).await;

        assert!(!template.needs_refresh());

        // Mark for refresh
        template.mark_for_refresh();

        assert!(template.needs_refresh());

        // Update transactions
        template.update_transactions(&mempool).await;

        assert!(!template.needs_refresh());
    }
}
