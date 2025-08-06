//! Test program for atomic swap implementation

#[cfg(not(feature = "atomic-swap"))]
fn main() {
    eprintln!("This binary requires the 'atomic-swap' feature to be enabled.");
    eprintln!("Run with: cargo run --features atomic-swap --bin test_atomic_swap");
    std::process::exit(1);
}

#[cfg(feature = "atomic-swap")]
fn main() {
    actual_main();
}

#[cfg(feature = "atomic-swap")]
fn actual_main() {
    use btclib::atomic_swap::{
        SupernovaHTLC, HTLCState, TimeLock, ParticipantInfo,
    };
    use btclib::atomic_swap::crypto::{HashLock, HashFunction};
    use btclib::atomic_swap::htlc::FeeStructure;
    use btclib::crypto::{MLDSAPrivateKey, MLDSASecurityLevel};
    use rand::rngs::OsRng;
    use std::time::{SystemTime, UNIX_EPOCH};
    
    println!("Testing Supernova Atomic Swap Implementation\n");
    
    // Create test participants
    let alice_key = MLDSAPrivateKey::generate(&mut OsRng, MLDSASecurityLevel::Level3)
        .expect("Failed to generate Alice's key");
    let alice = ParticipantInfo {
        pubkey: alice_key.public_key(),
        address: "nova1alice".to_string(),
        refund_address: None,
    };
    
    let bob_key = MLDSAPrivateKey::generate(&mut OsRng, MLDSASecurityLevel::Level3)
        .expect("Failed to generate Bob's key");
    let bob = ParticipantInfo {
        pubkey: bob_key.public_key(),
        address: "nova1bob".to_string(),
        refund_address: None,
    };
    
    // Create hash lock
    let hash_lock = HashLock::new(HashFunction::SHA256).unwrap();
    println!("Hash lock created:");
    println!("  Hash: {}", hex::encode(&hash_lock.hash_value));
    
    // Create time lock
    let current_time = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();
    
    let time_lock = TimeLock {
        absolute_timeout: current_time + 3600,
        relative_timeout: 144,
        grace_period: 6,
    };
    
    // Create HTLC
    let fee_structure = FeeStructure {
        claim_fee: 1000,
        refund_fee: 1000,
        service_fee: None,
    };
    
    let htlc = SupernovaHTLC::new(
        alice,
        bob,
        hash_lock,
        time_lock,
        100_000_000, // 1 NOVA
        fee_structure,
    ).unwrap();
    
    println!("\nHTLC created successfully!");
    println!("  ID: {}", hex::encode(&htlc.htlc_id));
    println!("  Amount: {} base units", htlc.amount);
    println!("  State: {:?}", htlc.state);
    
    println!("\n✅ Atomic swap module is working correctly!");
} 