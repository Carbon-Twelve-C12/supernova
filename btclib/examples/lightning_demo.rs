// SuperNova Lightning Network Demo
//
// This example demonstrates the basic functionality of the Lightning Network
// implementation in SuperNova, including channel creation, payments, and closure.

use btclib::lightning::{
    LightningNetwork,
    LightningConfig,
    LightningNetworkError,
    channel::{ChannelConfig, ChannelState},
};
use btclib::lightning::wallet::LightningWallet;
use btclib::lightning::invoice::{Invoice, PaymentHash};
use btclib::crypto::quantum::QuantumScheme;
use std::time::Duration;
use tracing::{info, warn, error, Level};
use tracing_subscriber;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_max_level(Level::INFO)
        .init();
    
    info!("Starting SuperNova Lightning Network Demo");
    
    // Create a test wallet with 1 BTC balance
    let mut wallet = LightningWallet::new_test_wallet(100_000_000);
    
    // Create Lightning configuration
    let config = LightningConfig {
        use_quantum_signatures: true,
        quantum_scheme: Some(QuantumScheme::Falcon),
        quantum_security_level: 1,
        ..LightningConfig::default()
    };
    
    // Create Lightning Network instance
    let lightning = LightningNetwork::new(config, wallet);
    
    info!("Lightning Network instance created with quantum-resistant signatures");
    
    // Configure custom channel parameters
    let channel_config = ChannelConfig {
        announce_channel: true,
        max_htlc_value_in_flight_msat: 50_000_000,
        min_htlc_value_msat: 1_000,
        max_accepted_htlcs: 20,
        cltv_expiry_delta: 40,
        channel_reserve_satoshis: 10_000,
        dust_limit_satoshis: 546,
        max_commitment_transactions: 10,
        use_quantum_signatures: true,
        force_close_timeout_seconds: 86400,
    };
    
    // Open a new channel
    let peer_id = "029a059f014307e795a31e1ddfdd19c7df6c7b1e2d09d6788c31ca4c38bac0f9ab";
    let channel_capacity = 10_000_000; // 0.1 Nova 
    let push_amount = 1_000_000;       // 0.01 Nova
    
    info!("Opening channel with peer {} with capacity of {} Nova", peer_id, channel_capacity);
    
    let channel_id = lightning.open_channel(
        peer_id,
        channel_capacity,
        push_amount,
        Some(channel_config.clone()),
    ).await?;
    
    info!("Channel opened with ID: {}", channel_id);
    
    // Simulate funding transaction confirmation
    tokio::time::sleep(Duration::from_secs(1)).await;
    
    // Get channel info
    let channel_info = lightning.get_channel_info(&channel_id)
        .expect("Channel should exist");
    
    info!("Channel state: {:?}", channel_info.state);
    info!("Local balance: {} msat", channel_info.local_balance_msat);
    info!("Remote balance: {} msat", channel_info.remote_balance_msat);
    
    // Create an invoice
    let invoice_amount = 500_000_000; // 500k satoshis in millisats
    let description = "Test payment";
    let expiry = 3600; // 1 hour
    
    info!("Creating invoice for {} msat", invoice_amount);
    
    let invoice = lightning.create_invoice(
        invoice_amount,
        description,
        expiry,
    )?;
    
    info!("Invoice created with payment hash: {}", hex::encode(invoice.payment_hash()));
    
    // Simulate payment
    tokio::time::sleep(Duration::from_secs(1)).await;
    
    // In a real scenario, another node would pay this invoice
    // For demonstration, we'll just simulate the payment
    info!("Simulating payment of invoice");
    
    let payment_result = lightning.pay_invoice(&invoice).await;
    
    match payment_result {
        Ok(preimage) => {
            info!("Payment successful! Preimage: {}", hex::encode(preimage.as_bytes()));
        },
        Err(e) => {
            warn!("Payment failed: {}", e);
        }
    }
    
    // List active channels
    let channels = lightning.list_channels();
    info!("Active channels: {}", channels.len());
    
    // Close channel
    info!("Closing channel: {}", channel_id);
    
    let force_close = false;
    let close_tx = lightning.close_channel(&channel_id, force_close).await?;
    
    info!("Channel closed with transaction: {}", hex::encode(&close_tx.hash()));
    
    // Verify channel is closed
    match lightning.get_channel_info(&channel_id) {
        Some(info) => {
            info!("Channel final state: {:?}", info.state);
        },
        None => {
            info!("Channel has been removed from active channels");
        }
    }
    
    info!("Lightning Network Demo completed successfully");
    
    Ok(())
} 