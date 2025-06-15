use node::{Node, NodeError};
use node::config::NodeConfig;
use tracing::{info, error, warn};
use clap::Parser;
use std::sync::Arc;
use tokio::signal;

/// Command-line arguments
#[derive(Parser, Debug)]
#[command(author, version, about = "Supernova blockchain node", long_about = None)]
struct Args {
    /// Start with animation
    #[arg(long)]
    with_animation: bool,
    
    /// Configuration file path
    #[arg(short, long, default_value = "config.toml")]
    config: String,
    
    /// Enable debug logging
    #[arg(short, long)]
    debug: bool,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Parse command-line arguments
    let args = Args::parse();

    // Initialize logging
    let log_level = if args.debug { "debug" } else { "info" };
    tracing_subscriber::fmt()
        .with_env_filter(log_level)
        .init();

    // Show animation if requested
    if args.with_animation {
        if let Err(e) = btclib::util::ascii_art::testnet_startup_animation() {
            eprintln!("Failed to display startup animation: {}", e);
        }
    }

    // Load configuration
    let config = NodeConfig::load()
        .unwrap_or_else(|e| {
            eprintln!("Failed to load configuration: {}", e);
            std::process::exit(1);
        });
    
    // Check if this is a testnet deployment
    let is_testnet = config.node.network_name.to_lowercase().contains("test") || 
                     config.testnet.enabled;
    
    // Display logo and info
    if is_testnet {
        info!("Starting Supernova Testnet node...");
        if !args.with_animation {
            if let Err(e) = btclib::util::ascii_art::display_logo() {
                eprintln!("Failed to display logo: {}", e);
            }
        }
        info!("Network: {}", config.node.network_name);
        info!("Chain ID: {}", config.node.chain_id);
        if config.testnet.enable_faucet {
            info!("Faucet: Enabled (amount: {} NOVA)", config.testnet.faucet_amount as f64 / 100_000_000.0);
        }
    } else {
        info!("Starting Supernova node...");
        info!("Network: {}", config.node.network_name);
        info!("Chain ID: {}", config.node.chain_id);
    }

    // Create and start node
    let node = Arc::new(Node::new(config.clone()).await?);
    
    // Start the node
    node.start().await?;
    
    // Start API server if configured (check if bind_address and port are set)
    let api_server_handle = if !config.api.bind_address.is_empty() && config.api.port > 0 {
        info!("Starting API server on {}:{}", config.api.bind_address, config.api.port);
        let api_server = node::api::create_api_server(
            Arc::clone(&node),
            &config.api.bind_address,
            config.api.port,
        );
        
        // Start the API server and get the server handle
        match api_server.start().await {
            Ok(server) => {
                info!("API server started on port {}", config.api.port);
                // Spawn the server to run in the background
                let server_handle = tokio::spawn(server);
                Some(server_handle)
            }
            Err(e) => {
                error!("Failed to start API server: {}", e);
                None
            }
        }
    } else {
        warn!("API server disabled (no bind address or port configured)");
        None
    };
    
    info!("Node started successfully");
    info!("Press Ctrl+C to stop the node");

    // Wait for shutdown signal
    match signal::ctrl_c().await {
        Ok(()) => {
            info!("Shutdown signal received");
        }
        Err(err) => {
            error!("Unable to listen for shutdown signal: {}", err);
        }
    }

    // Stop the API server if running
    if let Some(handle) = api_server_handle {
        handle.abort();
    }

    // Stop the node
    info!("Stopping node...");
    node.stop().await?;
    info!("Node stopped successfully");

    Ok(())
}