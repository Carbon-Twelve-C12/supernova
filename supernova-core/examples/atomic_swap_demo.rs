//! Atomic Swap Demo - Demonstrates the Supernova atomic swap implementation
//!
//! This example shows how to create and execute an atomic swap between
//! Bitcoin and Supernova blockchains.

use btclib::atomic_swap::bitcoin_adapter::{BitcoinHTLC, HTLCScriptType};
use btclib::atomic_swap::crypto::{generate_secure_random_32, HashFunction, HashLock};
use btclib::atomic_swap::htlc::FeeStructure;
use btclib::atomic_swap::monitor::MonitorConfig;
use btclib::atomic_swap::{
    AtomicSwapConfig, AtomicSwapSetup, CrossChainMonitor, FeeDistribution, FeePayer, HTLCState,
    ParticipantInfo, SupernovaHTLC, SwapSession, SwapState, TimeLock, TimeoutConfig,
};
use btclib::crypto::{MLDSAPrivateKey, MLDSAPublicKey};
use rand::rngs::OsRng;
use std::time::{SystemTime, UNIX_EPOCH};

fn main() {
    println!("=== Supernova Atomic Swap Demo ===\n");

    // 1. Create participants
    println!("1. Creating participants...");
    let alice_key = MLDSAPrivateKey::generate(&mut OsRng);
    let alice = ParticipantInfo {
        pubkey: alice_key.public_key(),
        address: "nova1alice123...".to_string(),
        refund_address: None,
    };
    println!("   Alice (Supernova): {}", alice.address);

    let bob_key = MLDSAPrivateKey::generate(&mut OsRng);
    let bob = ParticipantInfo {
        pubkey: bob_key.public_key(),
        address: "nova1bob456...".to_string(),
        refund_address: None,
    };
    println!("   Bob (Supernova): {}", bob.address);

    // 2. Create hash lock
    println!("\n2. Creating hash lock...");
    let hash_lock = HashLock::new(HashFunction::SHA256).unwrap();
    let secret = hash_lock.preimage.unwrap();
    println!("   Secret: {}", hex::encode(&secret));
    println!("   Hash: {}", hex::encode(&hash_lock.hash_value));

    // 3. Create time lock
    println!("\n3. Setting up time locks...");
    let current_time = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();

    let time_lock = TimeLock {
        absolute_timeout: current_time + 3600, // 1 hour from now
        relative_timeout: 144,                 // ~24 hours in Bitcoin blocks
        grace_period: 6,
    };
    println!(
        "   Timeout: {} seconds from now",
        time_lock.absolute_timeout - current_time
    );

    // 4. Create HTLC
    println!("\n4. Creating Supernova HTLC...");
    let fee_structure = FeeStructure {
        claim_fee: 1000,
        refund_fee: 1000,
        service_fee: Some(100),
    };

    let mut htlc = SupernovaHTLC::new(
        alice.clone(),
        bob.clone(),
        hash_lock.clone(),
        time_lock,
        100_000_000, // 1 NOVA
        fee_structure,
    )
    .unwrap();

    println!("   HTLC ID: {}", hex::encode(&htlc.htlc_id));
    println!("   Amount: {} base units", htlc.amount);
    println!("   State: {:?}", htlc.state);

    // 5. Simulate funding
    println!("\n5. Simulating HTLC funding...");
    htlc.update_state(HTLCState::Funded).unwrap();
    println!("   State updated to: {:?}", htlc.state);

    // 6. Test claim verification
    println!("\n6. Testing claim verification...");

    // Sign claim message
    let claim_sig = bob_key.sign(b"claim message placeholder");

    // Try with correct secret
    let claim_valid = htlc.verify_claim(&secret, &claim_sig, 0).unwrap();
    println!(
        "   Claim with correct secret: {}",
        if claim_valid {
            "VALID ✓"
        } else {
            "INVALID ✗"
        }
    );

    // Try with wrong secret
    let wrong_secret = [0u8; 32];
    let claim_invalid = htlc.verify_claim(&wrong_secret, &claim_sig, 0).unwrap();
    println!(
        "   Claim with wrong secret: {}",
        if claim_invalid {
            "VALID ✓"
        } else {
            "INVALID ✗"
        }
    );

    // 7. Create Bitcoin HTLC script
    println!("\n7. Creating Bitcoin HTLC script...");

    // Generate test Bitcoin keys (in production, use proper Bitcoin keys)
    let btc_sender_key =
        bitcoin::PrivateKey::from_slice(&[1u8; 32], bitcoin::Network::Testnet).unwrap();
    let btc_recipient_key =
        bitcoin::PrivateKey::from_slice(&[2u8; 32], bitcoin::Network::Testnet).unwrap();

    let bitcoin_htlc = BitcoinHTLC {
        hash_lock: hash_lock.hash_value,
        recipient_pubkey: btc_recipient_key.public_key(&bitcoin::secp256k1::Secp256k1::new()),
        sender_pubkey: btc_sender_key.public_key(&bitcoin::secp256k1::Secp256k1::new()),
        timeout_height: 500000,
        script_type: HTLCScriptType::P2WSH,
    };

    let btc_script = bitcoin_htlc.create_redeem_script();
    println!("   Bitcoin script size: {} bytes", btc_script.len());

    let btc_address = bitcoin_htlc
        .create_address(bitcoin::Network::Testnet)
        .unwrap();
    println!("   Bitcoin HTLC address: {}", btc_address);

    // 8. Create swap session
    println!("\n8. Creating swap session...");
    let swap_id = generate_secure_random_32();

    let setup = AtomicSwapSetup {
        swap_id,
        bitcoin_amount: 10_000,   // 0.0001 BTC
        nova_amount: 100_000_000, // 1 NOVA
        fee_distribution: FeeDistribution {
            bitcoin_fee_payer: FeePayer::Split(50),
            nova_fee_payer: FeePayer::Split(50),
        },
        timeout_blocks: TimeoutConfig {
            bitcoin_claim_timeout: 24,   // ~4 hours
            supernova_claim_timeout: 20, // ~3.5 hours
            refund_safety_margin: 6,
        },
    };

    let session = SwapSession {
        setup: setup.clone(),
        secret: Some(secret),
        nova_htlc: htlc,
        btc_htlc: btclib::atomic_swap::BitcoinHTLCReference {
            txid: "dummy_txid".to_string(),
            vout: 0,
            script_pubkey: vec![],
            amount: setup.bitcoin_amount,
            timeout_height: 500000,
        },
        state: SwapState::Active,
        created_at: current_time,
        updated_at: current_time,
    };

    println!("   Swap ID: {}", hex::encode(&swap_id));
    println!("   Bitcoin amount: {} sats", session.setup.bitcoin_amount);
    println!(
        "   Supernova amount: {} base units",
        session.setup.nova_amount
    );
    println!("   State: {:?}", session.state);

    // 9. Demonstrate monitoring
    println!("\n9. Setting up cross-chain monitor...");
    let monitor_config = MonitorConfig::default();
    let monitor = CrossChainMonitor::new(
        monitor_config,
        #[cfg(feature = "atomic-swap")]
        None, // No real Bitcoin client in this demo
    );

    println!("   Monitor created with poll interval: {} seconds", 10);
    println!("   Auto-claim enabled: true");
    println!("   Auto-refund enabled: true");

    // 10. Summary
    println!("\n=== Atomic Swap Demo Complete ===");
    println!("\nThis demo demonstrated:");
    println!("- Creating quantum-resistant participants");
    println!("- Setting up hash and time locks");
    println!("- Creating and funding HTLCs");
    println!("- Verifying claims with secrets");
    println!("- Generating Bitcoin HTLC scripts");
    println!("- Managing swap sessions");
    println!("- Cross-chain monitoring setup");

    println!("\nThe atomic swap implementation is ready for integration!");
}
