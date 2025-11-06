// Supernova Node Binary
// Main entry point for the Supernova blockchain node

// Enforce panic-free code in production
#![warn(clippy::unwrap_used)]
#![warn(clippy::expect_used)]
#![warn(clippy::panic)]
#![warn(clippy::unimplemented)]
#![warn(clippy::todo)]
#![warn(clippy::unreachable)]
#![warn(clippy::indexing_slicing)]

use clap::Parser;
use node::config::NodeConfig;
use node::Node;
use node::shutdown::{ShutdownCoordinator, ShutdownConfig, ShutdownSignal, register_signal_handlers};
use std::io::Write;
use std::sync::Arc;
use tokio::signal;
use tracing::{error, info, warn};

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
    use tracing_subscriber::EnvFilter;
    let env_filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| {
            // Fall back to default if RUST_LOG not set
            let log_level = if args.debug { "debug" } else { "info" };
            EnvFilter::new(log_level)
        });
    
    tracing_subscriber::fmt().with_env_filter(env_filter).init();

    // Show animation if requested
    if args.with_animation {
        if let Err(e) = supernova_core::util::ascii_art::testnet_startup_animation() {
            eprintln!("Failed to display startup animation: {}", e);
        }
    }

    // Load configuration
    let config = NodeConfig::load().unwrap_or_else(|e| {
        eprintln!("Failed to load configuration: {}", e);
        std::process::exit(1);
    });

    // Check if this is a testnet deployment
    let is_testnet =
        config.node.network_name.to_lowercase().contains("test") || config.testnet.enabled;

    // Display logo and info
    if is_testnet {
        info!("Starting Supernova Testnet node...");
        if !args.with_animation {
            if let Err(e) = supernova_core::util::ascii_art::display_logo() {
                eprintln!("Failed to display logo: {}", e);
            }
        }
        info!("Network: {}", config.node.network_name);
        info!("Chain ID: {}", config.node.chain_id);
        if config.testnet.enable_faucet {
            info!(
                "Faucet: Enabled (amount: {} NOVA)",
                config.testnet.faucet_amount as f64 / 100_000_000.0
            );
        }
    } else {
        info!("Starting Supernova node...");
        info!("Network: {}", config.node.network_name);
        info!("Chain ID: {}", config.node.chain_id);
    }
    
    // Create and start node
    let node = match Node::new(config.clone()).await {
        Ok(n) => Arc::new(n),
        Err(e) => {
            error!("Failed to create node: {}", e);
            return Err(e.into());
        }
    };
    
    // Start the node
    node.start().await?;

    // Start API server if configured (check if bind_address and port are set)
    let api_server_handle = if !config.api.bind_address.is_empty() && config.api.port > 0 {
        info!(
            "Starting API server on {}:{}",
            "0.0.0.0", config.api.port
        );
        let api_server = node::api::create_api_server(
            Arc::clone(&node),
            "0.0.0.0", // Listen on all interfaces for external access
            config.api.port,
        );

        // Start the API server and get the server handle
        match api_server.start().await {
            Ok(server) => {
                info!("API server started on port {}", config.api.port);
                // Spawn the server to run in the background
                let handle = tokio::spawn(server);
                Some(handle)
            }
            Err(e) => {
                error!("Failed to start API server: {}", e);
                None
            }
        }
    } else {
        info!("API server disabled or not configured");
        None
    };

    info!("Node started successfully");
    info!("Press Ctrl+C to stop the node");

    // Create shutdown coordinator
    let shutdown_config = ShutdownConfig {
        max_shutdown_time: std::time::Duration::from_secs(30),
        component_timeout: std::time::Duration::from_secs(5),
        persist_state: true,
        status_file_path: std::path::PathBuf::from("./data/shutdown_status.json"),
        force_after_timeout: true,
    };
    let shutdown_coordinator = Arc::new(ShutdownCoordinator::new(
        Arc::clone(&node),
        shutdown_config,
    ));

    // Register signal handlers
    let mut shutdown_rx = register_signal_handlers();

    // Wait for shutdown signal
    tokio::select! {
        _ = signal::ctrl_c() => {
            info!("Shutdown signal received (Ctrl+C)");
            shutdown_coordinator.request_shutdown(ShutdownSignal::User).await;
        }
        signal = shutdown_rx.recv() => {
            if let Some(sig) = signal {
                info!("Shutdown signal received: {:?}", sig);
                shutdown_coordinator.request_shutdown(sig).await;
            }
        }
    }

    // Stop the API server if running
    if let Some(handle) = api_server_handle {
        handle.abort();
    }

    // Perform graceful shutdown
    info!("Initiating graceful shutdown...");
    match shutdown_coordinator.shutdown(ShutdownSignal::User).await {
        Ok(()) => {
            info!("Node stopped successfully");
        }
        Err(e) => {
            error!("Shutdown failed: {}", e);
            std::process::exit(1);
        }
    }

    Ok(())
}
