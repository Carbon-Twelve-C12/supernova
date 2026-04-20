//! Treasury Output Validation — consensus-critical end-to-end tests
//!
//! P1-mainnet-blocker A1. Verifies that:
//! * the miner attaches a well-formed P2WSH treasury output to every
//!   coinbase whose 5% share clears the dust threshold,
//! * the validator's [`BlockTemplate::validate_coinbase_treasury`] accepts
//!   canonical coinbases and rejects every attack we can think of:
//!     - full reward diverted to the miner,
//!     - treasury amount skewed outside the 1% tolerance,
//!     - treasury output carrying the wrong P2WSH script (wrong network,
//!       legacy ASCII placeholder, garbage bytes),
//!     - total outputs exceeding the reward (inflation).
//!
//! The legacy-string placeholder `b"TREASURY_PLACEHOLDER_ADDRESS"` is
//! rejected at every network because it is not a valid P2WSH script.

use async_trait::async_trait;
use miner::mining::template::{BlockTemplate, MempoolInterface, TreasuryAllocationConfig};
use supernova_core::config::NetworkType;
use supernova_core::governance::{
    treasury_script_pubkey, MAINNET_TREASURY_PENDING_GENESIS, TREASURY_ALLOCATION_PERCENT,
    TREASURY_SCRIPT_LEN,
};
use supernova_core::types::transaction::{
    Transaction, TransactionInput, TransactionOutput,
};

struct EmptyMempool;

#[async_trait]
impl MempoolInterface for EmptyMempool {
    async fn get_transactions(&self, _max_size: usize) -> Vec<Transaction> {
        Vec::new()
    }
}

fn coinbase_input() -> TransactionInput {
    TransactionInput::new([0u8; 32], 0xffffffff, vec![], 0)
}

fn build_coinbase(miner_amount: u64, treasury_amount: u64, treasury_script: Vec<u8>) -> Transaction {
    let outputs = vec![
        TransactionOutput::new(miner_amount, vec![0x01, 0x02, 0x03]),
        TransactionOutput::new(treasury_amount, treasury_script),
    ];
    Transaction::new(1, vec![coinbase_input()], outputs, 0)
}

#[test]
fn treasury_allocation_constant_matches_governance() {
    assert_eq!(
        TreasuryAllocationConfig::TREASURY_ALLOCATION_PERCENT,
        TREASURY_ALLOCATION_PERCENT,
        "miner and governance must use the same percent constant",
    );
    assert_eq!(TreasuryAllocationConfig::MIN_TREASURY_OUTPUT, 1000);
}

#[tokio::test]
async fn miner_builds_canonical_treasury_output() {
    // Rewards here are deterministic for block 1.
    let template = BlockTemplate::new(
        1,
        [0u8; 32],
        u32::MAX,
        vec![0xAA, 0xBB, 0xCC],
        &EmptyMempool,
        1,
        None,
        NetworkType::Regtest,
    )
    .await;

    let block = template.create_block();
    let coinbase = &block.transactions()[0];
    let outputs = coinbase.outputs();
    assert_eq!(outputs.len(), 2, "coinbase must have miner + treasury outputs");

    let script = &outputs[1].pub_key_script;
    assert_eq!(script.len(), TREASURY_SCRIPT_LEN, "P2WSH is 34 bytes");
    assert_eq!(script[0], 0x00, "witness version 0");
    assert_eq!(script[1], 0x20, "push 32 bytes");
    assert_eq!(
        script,
        &treasury_script_pubkey(NetworkType::Regtest),
        "must equal regtest governance script",
    );

    let total: u64 = outputs.iter().map(|o| o.amount()).sum();
    let expected_treasury = total.saturating_mul(TREASURY_ALLOCATION_PERCENT) / 100;
    assert_eq!(outputs[1].amount(), expected_treasury);
}

#[test]
fn validator_accepts_canonical_coinbase() {
    let reward = 5_000_000_000u64;
    let treasury = reward * TREASURY_ALLOCATION_PERCENT / 100;
    let miner = reward - treasury;
    let coinbase = build_coinbase(miner, treasury, treasury_script_pubkey(NetworkType::Testnet));
    assert!(
        BlockTemplate::validate_coinbase_treasury(&coinbase, reward, NetworkType::Testnet).is_ok(),
    );
}

#[test]
fn validator_rejects_full_reward_theft() {
    // Attacker keeps 100% for themselves — no treasury output at all.
    let reward = 5_000_000_000u64;
    let coinbase = Transaction::new(
        1,
        vec![coinbase_input()],
        vec![TransactionOutput::new(reward, vec![0x01])],
        0,
    );
    let err = BlockTemplate::validate_coinbase_treasury(&coinbase, reward, NetworkType::Testnet)
        .expect_err("missing treasury must be rejected");
    assert!(err.contains("missing treasury"), "unexpected error: {}", err);
}

#[test]
fn validator_rejects_legacy_placeholder_script() {
    // Pre-governance placeholder was 28 ASCII bytes, not a P2WSH script.
    let reward = 5_000_000_000u64;
    let treasury = reward * TREASURY_ALLOCATION_PERCENT / 100;
    let miner = reward - treasury;
    let legacy_script = b"TREASURY_PLACEHOLDER_ADDRESS".to_vec();
    let coinbase = build_coinbase(miner, treasury, legacy_script);
    let err = BlockTemplate::validate_coinbase_treasury(&coinbase, reward, NetworkType::Testnet)
        .expect_err("legacy placeholder must be rejected");
    assert!(
        err.contains("Treasury script rejected"),
        "unexpected error: {}",
        err
    );
}

#[test]
fn validator_rejects_wrong_network_script() {
    // Miner on Testnet, validator on Regtest — scripts must not match.
    let reward = 5_000_000_000u64;
    let treasury = reward * TREASURY_ALLOCATION_PERCENT / 100;
    let miner = reward - treasury;
    let coinbase = build_coinbase(miner, treasury, treasury_script_pubkey(NetworkType::Testnet));
    let err = BlockTemplate::validate_coinbase_treasury(&coinbase, reward, NetworkType::Regtest)
        .expect_err("cross-network scripts must be rejected");
    assert!(
        err.contains("Treasury script rejected"),
        "unexpected error: {}",
        err
    );
}

#[test]
fn validator_rejects_amount_outside_tolerance() {
    let reward = 5_000_000_000u64;
    let canonical_treasury = reward * TREASURY_ALLOCATION_PERCENT / 100; // 250_000_000
    let tolerance = canonical_treasury / 100; // 2_500_000

    // Well below the lower bound.
    let skewed_treasury = canonical_treasury - tolerance - 1;
    let miner = reward - skewed_treasury;
    let coinbase = build_coinbase(
        miner,
        skewed_treasury,
        treasury_script_pubkey(NetworkType::Testnet),
    );
    let err = BlockTemplate::validate_coinbase_treasury(&coinbase, reward, NetworkType::Testnet)
        .expect_err("below-tolerance amount must be rejected");
    assert!(
        err.contains("Treasury output amount incorrect"),
        "unexpected error: {}",
        err
    );
}

#[test]
fn validator_accepts_amount_inside_tolerance() {
    let reward = 5_000_000_000u64;
    let canonical_treasury = reward * TREASURY_ALLOCATION_PERCENT / 100;
    let tolerance = canonical_treasury / 100;
    // Within tolerance — half the band above canonical.
    let near_treasury = canonical_treasury + tolerance / 2;
    let miner = reward - near_treasury;
    let coinbase = build_coinbase(
        miner,
        near_treasury,
        treasury_script_pubkey(NetworkType::Testnet),
    );
    assert!(
        BlockTemplate::validate_coinbase_treasury(&coinbase, reward, NetworkType::Testnet).is_ok(),
    );
}

#[test]
fn validator_rejects_inflated_total() {
    // Total outputs exceed the advertised reward — classic value creation.
    let reward = 5_000_000_000u64;
    let treasury = reward * TREASURY_ALLOCATION_PERCENT / 100;
    let miner_inflated = reward; // takes the full reward, treasury output is "extra"
    let coinbase = build_coinbase(
        miner_inflated,
        treasury,
        treasury_script_pubkey(NetworkType::Testnet),
    );
    let err = BlockTemplate::validate_coinbase_treasury(&coinbase, reward, NetworkType::Testnet)
        .expect_err("inflation must be rejected");
    assert!(
        err.contains("exceed expected reward"),
        "unexpected error: {}",
        err
    );
}

#[test]
fn validator_tolerates_dust_rewards() {
    // For rewards below the dust threshold, the 5% share rounds below
    // MIN_TREASURY_OUTPUT; validator and miner agree the treasury output
    // is optional.
    let dust_reward = 500u64;
    let expected_treasury = dust_reward * TREASURY_ALLOCATION_PERCENT / 100;
    assert!(expected_treasury < TreasuryAllocationConfig::MIN_TREASURY_OUTPUT);

    let coinbase = Transaction::new(
        1,
        vec![coinbase_input()],
        vec![TransactionOutput::new(dust_reward, vec![0xAA])],
        0,
    );
    assert!(
        BlockTemplate::validate_coinbase_treasury(&coinbase, dust_reward, NetworkType::Testnet)
            .is_ok(),
    );
}

#[test]
fn mainnet_pending_sentinel_script_is_distinct() {
    // Sanity check: any future replacement must update this constant.
    let mainnet = treasury_script_pubkey(NetworkType::Mainnet);
    assert_eq!(&mainnet[2..], &MAINNET_TREASURY_PENDING_GENESIS);
    assert_ne!(mainnet, treasury_script_pubkey(NetworkType::Testnet));
    assert_ne!(mainnet, treasury_script_pubkey(NetworkType::Regtest));
}
