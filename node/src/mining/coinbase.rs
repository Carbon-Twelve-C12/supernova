// Coinbase Transaction Construction for Supernova Mining
// Handles block rewards, environmental treasury allocation, and quantum signatures

use supernova_core::types::transaction::{Transaction, TransactionInput, TransactionOutput};
use thiserror::Error;
use wallet::quantum_wallet::Address;

#[derive(Error, Debug)]
pub enum CoinbaseError {
    #[error("Invalid block height: {0}")]
    InvalidBlockHeight(u64),
    
    #[error("Invalid reward amount: {0}")]
    InvalidReward(u64),
    
    #[error("Address error: {0}")]
    AddressError(String),
    
    #[error("Treasury allocation error: {0}")]
    TreasuryError(String),
}

/// Block reward schedule for Supernova
/// Similar to Bitcoin's halving schedule
const INITIAL_BLOCK_REWARD: u64 = 50_00000000; // 50 NOVA (in attonovas)
const HALVING_INTERVAL: u64 = 210_000; // Blocks between halvings

/// Environmental treasury allocation percentage
const TREASURY_PERCENTAGE: f64 = 0.025; // 2.5% of block reward to treasury

/// Build coinbase transaction for a new block
///
/// # Arguments
/// * `block_height` - Height of the block being mined
/// * `reward_address` - Address to receive miner reward
/// * `total_fees` - Sum of transaction fees in the block
/// * `treasury_address` - Address for environmental treasury
///
/// # Returns
/// * `Transaction` - Fully constructed coinbase transaction
pub fn build_coinbase_transaction(
    block_height: u64,
    reward_address: &Address,
    total_fees: u64,
    treasury_address: &Address,
) -> Result<Transaction, CoinbaseError> {
    // Calculate block reward based on halving schedule
    let base_reward = calculate_block_reward(block_height);
    
    // Total available = base reward + fees
    let total_reward = base_reward.checked_add(total_fees)
        .ok_or(CoinbaseError::InvalidReward(base_reward))?;
    
    // Calculate treasury allocation
    let treasury_amount = (total_reward as f64 * TREASURY_PERCENTAGE) as u64;
    let miner_amount = total_reward.saturating_sub(treasury_amount);
    
    // Create coinbase input with block height
    let coinbase_script = create_coinbase_script(block_height);
    let coinbase_input = TransactionInput::new_coinbase(coinbase_script);
    
    // Create outputs
    let mut outputs = Vec::new();
    
    // Output 1: Miner reward
    outputs.push(TransactionOutput::new(
        miner_amount,
        reward_address.pubkey_hash().to_vec(),
    ));
    
    // Output 2: Environmental treasury
    if treasury_amount > 0 {
        outputs.push(TransactionOutput::new(
            treasury_amount,
            treasury_address.pubkey_hash().to_vec(),
        ));
    }
    
    // Build transaction
    let transaction = Transaction::new(
        1, // version
        vec![coinbase_input],
        outputs,
        0, // locktime
    );
    
    Ok(transaction)
}

/// Calculate block reward based on height (with halving)
fn calculate_block_reward(block_height: u64) -> u64 {
    let halvings = block_height / HALVING_INTERVAL;
    
    // Maximum 64 halvings (after which reward is 0)
    if halvings >= 64 {
        return 0;
    }
    
    // Right shift is equivalent to dividing by 2^halvings
    INITIAL_BLOCK_REWARD >> halvings
}

/// Create coinbase script with block height
fn create_coinbase_script(block_height: u64) -> Vec<u8> {
    let mut script = Vec::new();
    
    // BIP34: Block height must be in coinbase
    // Encode height as compact size
    if block_height <= 0xFC {
        script.push(block_height as u8);
    } else if block_height <= 0xFFFF {
        script.push(0xFD);
        script.extend_from_slice(&(block_height as u16).to_le_bytes());
    } else if block_height <= 0xFFFFFFFF {
        script.push(0xFE);
        script.extend_from_slice(&(block_height as u32).to_le_bytes());
    } else {
        script.push(0xFF);
        script.extend_from_slice(&block_height.to_le_bytes());
    }
    
    // Add identifying text
    let text = format!("Supernova v1.0 - Quantum-Resistant, Carbon-Negative");
    script.extend_from_slice(text.as_bytes());
    
    script
}

/// Validate coinbase transaction
pub fn validate_coinbase(
    transaction: &Transaction,
    block_height: u64,
    total_fees: u64,
) -> Result<(), CoinbaseError> {
    // Must have exactly 1 input
    if transaction.inputs().len() != 1 {
        return Err(CoinbaseError::InvalidReward(transaction.inputs().len() as u64));
    }
    
    // First input must be coinbase
    let input = &transaction.inputs()[0];
    if input.prev_output_index() != 0xFFFFFFFF {
        return Err(CoinbaseError::InvalidBlockHeight(block_height));
    }
    
    // Calculate expected reward
    let expected_reward = calculate_block_reward(block_height).checked_add(total_fees)
        .ok_or(CoinbaseError::InvalidReward(block_height))?;
    
    // Sum outputs
    let actual_reward: u64 = transaction.outputs().iter().map(|o| o.value()).sum();
    
    // Actual should not exceed expected
    if actual_reward > expected_reward {
        return Err(CoinbaseError::InvalidReward(actual_reward));
    }
    
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_initial_block_reward() {
        let reward = calculate_block_reward(0);
        assert_eq!(reward, 50_00000000); // 50 NOVA
    }
    
    #[test]
    fn test_first_halving() {
        let reward = calculate_block_reward(210_000);
        assert_eq!(reward, 25_00000000); // 25 NOVA
    }
    
    #[test]
    fn test_second_halving() {
        let reward = calculate_block_reward(420_000);
        assert_eq!(reward, 12_50000000); // 12.5 NOVA
    }
    
    #[test]
    fn test_reward_after_64_halvings() {
        let reward = calculate_block_reward(64 * 210_000);
        assert_eq!(reward, 0); // Subsidy exhausted
    }
    
    #[test]
    fn test_coinbase_script_generation() {
        let script = create_coinbase_script(12345);
        
        // Should contain height
        assert!(script.len() > 2);
        
        // Should contain identifying text
        let script_str = String::from_utf8_lossy(&script);
        assert!(script_str.contains("Supernova"));
    }
    
    #[test]
    fn test_treasury_allocation() {
        use wallet::quantum_wallet::keystore::KeyPair;
        
        let miner_keypair = KeyPair::generate(None).unwrap();
        let treasury_keypair = KeyPair::generate(None).unwrap();
        
        let coinbase = build_coinbase_transaction(
            100,
            &miner_keypair.address,
            1_00000000, // 1 NOVA in fees
            &treasury_keypair.address,
        ).unwrap();
        
        // Should have 2 outputs (miner + treasury)
        assert_eq!(coinbase.outputs().len(), 2);
        
        // Calculate expected amounts
        let total = 50_00000000 + 1_00000000; // 51 NOVA total
        let treasury_expected = (total as f64 * 0.025) as u64; // 2.5%
        let miner_expected = total - treasury_expected;
        
        // Verify outputs
        assert_eq!(coinbase.outputs()[0].value(), miner_expected);
        assert_eq!(coinbase.outputs()[1].value(), treasury_expected);
    }
    
    #[test]
    fn test_coinbase_validation() {
        use wallet::quantum_wallet::keystore::KeyPair;
        
        let miner_keypair = KeyPair::generate(None).unwrap();
        let treasury_keypair = KeyPair::generate(None).unwrap();
        
        let coinbase = build_coinbase_transaction(
            100,
            &miner_keypair.address,
            0,
            &treasury_keypair.address,
        ).unwrap();
        
        // Should validate correctly
        assert!(validate_coinbase(&coinbase, 100, 0).is_ok());
    }
}

