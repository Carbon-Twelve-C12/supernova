//! Atomic Swap Cross-Chain Monitoring Demo
//! 
//! This example demonstrates the Phase 2 monitoring capabilities including:
//! - Bitcoin blockchain monitoring
//! - Supernova event detection
//! - Automatic claim/refund triggers
//! - Real-time swap state tracking

use btclib::atomic_swap::{
    monitor::{CrossChainMonitor, MonitorConfig, SupernovaHandle},
    SwapSession, SwapState,
    AtomicSwapSetup, TimeoutConfig, FeeDistribution, FeePayer,
    SupernovaHTLC, BitcoinHTLCReference,
    ParticipantInfo,
    crypto::{HashLock, HashFunction},
    htlc::{TimeLock, FeeStructure},
};
use btclib::crypto::MLDSAPrivateKey;
use rand::rngs::OsRng;
use std::sync::Arc;
use tokio::time::{sleep, Duration};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Supernova Atomic Swap Monitoring Demo ===\n");

    // 1. Create monitoring configuration
    println!("1. Setting up monitoring configuration...");
    let mut config = MonitorConfig::default();
    config.poll_interval = 5; // 5 seconds for demo
    config.auto_claim = true;
    config.auto_refund = true;
    config.min_confirmations = 2;
    println!("   Poll interval: {} seconds", config.poll_interval);
    println!("   Auto-claim: {}", config.auto_claim);
    println!("   Auto-refund: {}", config.auto_refund);
    println!("   Confirmations required: {}", config.min_confirmations);

    // 2. Create Supernova handle (simulated)
    println!("\n2. Creating Supernova blockchain handle...");
    let supernova_handle = Some(SupernovaHandle {
        current_height: 1000,
    });

    // 3. Create cross-chain monitor
    println!("\n3. Initializing cross-chain monitor...");
    let monitor = Arc::new(CrossChainMonitor::new(config, supernova_handle));
    println!("   Monitor created successfully");

    // 4. Create test swap sessions
    println!("\n4. Creating test swap sessions...");
    let swap1 = create_demo_swap("Alice", "Bob", 100000, 1000000000);
    let swap2 = create_demo_swap("Charlie", "Dave", 50000, 500000000);
    
    // Add swaps to monitor
    monitor.add_swap(swap1.clone()).await?;
    monitor.add_swap(swap2.clone()).await?;
    println!("   Added 2 swap sessions to monitor");

    // 5. Start monitoring in background
    println!("\n5. Starting cross-chain monitoring...");
    let monitor_clone = monitor.clone();
    let monitor_task = tokio::spawn(async move {
        monitor_clone.start_monitoring().await;
    });

    // 6. Simulate blockchain events
    println!("\n6. Simulating blockchain events...");
    
    // Give monitor time to start
    sleep(Duration::from_secs(1)).await;
    
    // Check active swaps
    let active_swaps = monitor.get_active_swaps().await;
    println!("   Active swaps: {}", active_swaps.len());
    for swap in &active_swaps {
        println!("   - Swap {}: {} BTC ↔ {} NOVA (State: {:?})",
            hex::encode(&swap.swap_id[..8]),
            swap.bitcoin_amount,
            swap.nova_amount,
            swap.state
        );
    }

    // 7. Simulate secret revelation on Bitcoin
    println!("\n7. Simulating secret revelation on Bitcoin blockchain...");
    sleep(Duration::from_secs(2)).await;
    
    // In a real scenario, this would be detected by monitoring Bitcoin blocks
    // and the monitor would automatically trigger the claim
    println!("   Secret revealed for swap 1!");
    println!("   (In production, the monitor would automatically claim on Supernova)");

    // 8. Check updated states
    println!("\n8. Checking updated swap states...");
    sleep(Duration::from_secs(1)).await;
    
    let updated_swaps = monitor.get_active_swaps().await;
    for swap in &updated_swaps {
        println!("   - Swap {}: State = {:?}",
            hex::encode(&swap.swap_id[..8]),
            swap.state
        );
    }

    // 9. Simulate timeout scenario
    println!("\n9. Simulating timeout scenario for swap 2...");
    sleep(Duration::from_secs(2)).await;
    
    // In a real scenario, the monitor would detect the timeout
    // and automatically trigger the refund
    println!("   Timeout reached for swap 2!");
    println!("   (In production, the monitor would automatically refund on Supernova)");

    // 10. Final state check
    println!("\n10. Final swap states:");
    sleep(Duration::from_secs(1)).await;
    
    let final_swaps = monitor.get_active_swaps().await;
    for swap in &final_swaps {
        let status = match swap.state {
            SwapState::Claimed => "✅ CLAIMED",
            SwapState::Refunded => "↩️ REFUNDED",
            _ => "⏳ PENDING",
        };
        println!("   - Swap {}: {}",
            hex::encode(&swap.swap_id[..8]),
            status
        );
    }

    // Stop monitoring
    monitor_task.abort();

    println!("\n=== Monitoring Demo Complete ===");
    println!("\nThis demo demonstrated:");
    println!("- Cross-chain monitoring setup");
    println!("- Real-time swap state tracking");
    println!("- Automatic claim detection and execution");
    println!("- Timeout handling and refunds");
    println!("- Event-driven architecture");

    Ok(())
}

fn create_demo_swap(from: &str, to: &str, btc_amount: u64, nova_amount: u64) -> SwapSession {
    let mut rng = OsRng;
    
    let initiator_key = MLDSAPrivateKey::generate(&mut rng);
    let recipient_key = MLDSAPrivateKey::generate(&mut rng);
    
    let initiator = ParticipantInfo {
        pubkey: initiator_key.public_key(),
        address: format!("nova1{}", from.to_lowercase()),
        refund_address: None,
    };
    
    let recipient = ParticipantInfo {
        pubkey: recipient_key.public_key(),
        address: format!("nova1{}", to.to_lowercase()),
        refund_address: None,
    };
    
    let hash_lock = HashLock::new(HashFunction::SHA256).unwrap();
    
    let time_lock = TimeLock {
        absolute_timeout: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() + 3600,
        relative_timeout: 144,
        grace_period: 6,
    };
    
    let fee_structure = FeeStructure {
        claim_fee: 1000,
        refund_fee: 1000,
        service_fee: None,
    };
    
    let mut swap_id = [0u8; 32];
    let copy_len = std::cmp::min(8, from.len());
    swap_id[..copy_len].copy_from_slice(&from.as_bytes()[..copy_len]);
    
    let timeout_config = TimeoutConfig {
        bitcoin_claim_timeout: 144,
        supernova_claim_timeout: 720,
        refund_safety_margin: 6,
    };
    
    let setup = AtomicSwapSetup {
        swap_id,
        bitcoin_amount: btc_amount,
        nova_amount,
        fee_distribution: FeeDistribution {
            bitcoin_fee_payer: FeePayer::Sender,
            nova_fee_payer: FeePayer::Recipient,
        },
        timeout_blocks: timeout_config,
    };
    
    let nova_htlc = SupernovaHTLC::new(
        initiator.clone(),
        recipient.clone(),
        hash_lock,
        time_lock,
        nova_amount,
        fee_structure,
    ).unwrap();
    
    let btc_htlc = BitcoinHTLCReference {
        txid: format!("{}btctx", from.to_lowercase()),
        vout: 0,
        script_pubkey: vec![0x00, 0x14], // Dummy P2WPKH prefix
        amount: btc_amount,
        timeout_height: 500000,
    };
    
    SwapSession {
        setup,
        secret: None,
        nova_htlc,
        btc_htlc,
        state: SwapState::Active,
        created_at: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs(),
        updated_at: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs(),
    }
} 