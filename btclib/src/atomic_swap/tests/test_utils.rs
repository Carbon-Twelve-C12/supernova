//! Common test utilities for atomic swap tests

use crate::atomic_swap::{
    AtomicSwapConfig, AtomicSwapSetup, SwapSession, SwapState,
    SupernovaHTLC, HTLCState, ParticipantInfo, TimeLock, FeeStructure,
    HashLock, BitcoinHTLCReference, TimeoutConfig, FeeDistribution, FeePayer,
};
use crate::atomic_swap::crypto::{HashFunction, generate_secure_random_32};
use crate::atomic_swap::monitor::{CrossChainMonitor, MonitorConfig};
use crate::crypto::{MLDSAPrivateKey, MLDSAPublicKey};
use bitcoin::{Network, Transaction as BitcoinTransaction};
use rand::rngs::OsRng;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

/// Test participant with keys
pub struct TestParticipant {
    pub info: ParticipantInfo,
    pub private_key: MLDSAPrivateKey,
}

impl TestParticipant {
    /// Create a new test participant
    pub fn new(name: &str) -> Self {
        let private_key = MLDSAPrivateKey::generate(&mut OsRng);
        let public_key = private_key.public_key();
        
        let info = ParticipantInfo {
            pubkey: public_key,
            address: format!("nova1{}", name),
            refund_address: Some(format!("nova1{}_refund", name)),
        };
        
        Self { info, private_key }
    }
}

/// Create a test HTLC
pub fn create_test_htlc(
    alice: &ParticipantInfo,
    bob: &ParticipantInfo,
    amount: u64,
) -> SupernovaHTLC {
    let hash_lock = HashLock::new(HashFunction::SHA256).unwrap();
    
    let time_lock = TimeLock {
        absolute_timeout: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() + 3600, // 1 hour
        relative_timeout: 144,
        grace_period: 6,
    };
    
    let fee_structure = FeeStructure {
        claim_fee: 1000,
        refund_fee: 1000,
        service_fee: Some(100),
    };
    
    SupernovaHTLC::new(
        alice.clone(),
        bob.clone(),
        hash_lock,
        time_lock,
        amount,
        fee_structure,
    ).unwrap()
}

/// Create a test swap session
pub fn create_test_swap_session(
    alice: &ParticipantInfo,
    bob: &ParticipantInfo,
    btc_amount: u64,
    nova_amount: u64,
) -> SwapSession {
    let swap_id = generate_secure_random_32();
    
    let setup = AtomicSwapSetup {
        swap_id,
        bitcoin_amount: btc_amount,
        nova_amount,
        fee_distribution: FeeDistribution {
            bitcoin_fee_payer: FeePayer::Sender,
            nova_fee_payer: FeePayer::Sender,
        },
        timeout_blocks: TimeoutConfig {
            bitcoin_claim_timeout: 144,
            supernova_claim_timeout: 720,
            refund_safety_margin: 10,
        },
    };
    
    let nova_htlc = create_test_htlc(alice, bob, nova_amount);
    
    let btc_htlc = BitcoinHTLCReference {
        txid: "pending".to_string(),
        vout: 0,
        script_pubkey: vec![],
        amount: btc_amount,
        timeout_height: 0,
        address: "tb1qtest".to_string(),
    };
    
    SwapSession {
        setup,
        secret: Some(nova_htlc.hash_lock.preimage.unwrap()),
        nova_htlc,
        btc_htlc,
        state: SwapState::Active,
        created_at: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs(),
        updated_at: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs(),
    }
}

/// Create a test monitor
pub fn create_test_monitor() -> Arc<CrossChainMonitor> {
    let config = MonitorConfig::default();
    Arc::new(CrossChainMonitor::new(config, None))
}

/// Create a mock Bitcoin transaction for testing
pub fn create_mock_bitcoin_tx(
    txid: &str,
    amount: u64,
    script_pubkey: Vec<u8>,
) -> BitcoinTransaction {
    use bitcoin::{Transaction, TxOut, TxIn, OutPoint, Txid};
    use std::str::FromStr;
    
    Transaction {
        version: 2,
        lock_time: bitcoin::blockdata::locktime::absolute::LockTime::ZERO,
        input: vec![TxIn {
            previous_output: OutPoint {
                txid: Txid::from_str("0000000000000000000000000000000000000000000000000000000000000000").unwrap(),
                vout: 0,
            },
            script_sig: bitcoin::Script::empty().into(),
            sequence: bitcoin::blockdata::transaction::Sequence::MAX,
            witness: bitcoin::blockdata::witness::Witness::default(),
        }],
        output: vec![TxOut {
            value: amount,
            script_pubkey: bitcoin::Script::from(script_pubkey).into(),
        }],
    }
}

/// Helper to advance time in tests
pub fn advance_test_time(seconds: u64) -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs() + seconds
}

/// Create a test atomic swap config
pub fn create_test_config() -> AtomicSwapConfig {
    AtomicSwapConfig {
        bitcoin_network: "testnet".to_string(),
        bitcoin_rpc_url: "http://localhost:18332".to_string(),
        bitcoin_rpc_user: Some("test".to_string()),
        bitcoin_rpc_pass: Some("test".to_string()),
        min_btc_confirmations: 1,
        min_nova_confirmations: 10,
        timeout_delta: 144,
        refund_grace_period: 6,
        min_swap_amount_btc: 10_000,
        max_swap_amount_btc: 10_000_000,
        max_swaps_per_hour: 1000,
        max_swaps_per_address: 100,
    }
}

/// Verify HTLC state transitions
pub fn assert_valid_htlc_transition(from: HTLCState, to: HTLCState) -> bool {
    use HTLCState::*;
    
    match (from, to) {
        (Created, Funded) => true,
        (Funded, Claimed) => true,
        (Funded, Refunded) => true,
        (Created, Refunded) => true,
        _ => false,
    }
}

/// Generate test hash and preimage
pub fn generate_test_hash_pair() -> ([u8; 32], [u8; 32]) {
    use sha2::{Sha256, Digest};
    
    let preimage = generate_secure_random_32();
    let mut hasher = Sha256::new();
    hasher.update(&preimage);
    let hash_result = hasher.finalize();
    
    let mut hash = [0u8; 32];
    hash.copy_from_slice(&hash_result);
    
    (preimage, hash)
} 