//! Atomic swap CLI commands
//!
//! This module provides CLI commands for managing atomic swaps
//! between Bitcoin and Supernova blockchains.

use clap::{Args, Subcommand};
use colored::*;
use serde_json::json;
use std::time::{SystemTime, UNIX_EPOCH};

/// Atomic swap commands
#[derive(Debug, Args)]
pub struct SwapCmd {
    #[command(subcommand)]
    pub command: SwapCommand,
}

/// Swap subcommands
#[derive(Debug, Subcommand)]
pub enum SwapCommand {
    /// Initialize a new atomic swap
    Init(InitSwapArgs),
    
    /// Check the status of a swap
    Status(StatusArgs),
    
    /// Claim a swap using the secret
    Claim(ClaimArgs),
    
    /// Refund a timed-out swap
    Refund(RefundArgs),
    
    /// List all active swaps
    List(ListArgs),
    
    /// Monitor swap events in real-time
    Monitor(MonitorArgs),
}

/// Arguments for swap initialization
#[derive(Debug, Args)]
pub struct InitSwapArgs {
    /// Amount of Bitcoin to swap (in satoshis)
    #[arg(long)]
    pub btc_amount: u64,
    
    /// Amount of NOVA to receive
    #[arg(long)]
    pub nova_amount: u64,
    
    /// Bitcoin counterparty address
    #[arg(long)]
    pub btc_counterparty: String,
    
    /// Supernova counterparty address
    #[arg(long)]
    pub nova_counterparty: String,
    
    /// Timeout in minutes (default: 60)
    #[arg(long, default_value = "60")]
    pub timeout_minutes: u32,
}

/// Arguments for status check
#[derive(Debug, Args)]
pub struct StatusArgs {
    /// Swap ID (hex encoded)
    pub swap_id: String,
    
    /// Show detailed information
    #[arg(long, short)]
    pub verbose: bool,
}

/// Arguments for claiming a swap
#[derive(Debug, Args)]
pub struct ClaimArgs {
    /// Swap ID (hex encoded)
    pub swap_id: String,
    
    /// Secret preimage (hex encoded)
    pub secret: String,
}

/// Arguments for refunding a swap
#[derive(Debug, Args)]
pub struct RefundArgs {
    /// Swap ID (hex encoded)
    pub swap_id: String,
}

/// Arguments for listing swaps
#[derive(Debug, Args)]
pub struct ListArgs {
    /// Filter by state (active, completed, refunded)
    #[arg(long)]
    pub state: Option<String>,
    
    /// Show only swaps from last N hours
    #[arg(long)]
    pub hours: Option<u32>,
}

/// Arguments for monitoring
#[derive(Debug, Args)]
pub struct MonitorArgs {
    /// Specific swap ID to monitor (monitors all if not specified)
    #[arg(long)]
    pub swap_id: Option<String>,
    
    /// Output format (text, json)
    #[arg(long, default_value = "text")]
    pub format: String,
}

/// Execute swap command
pub async fn execute(cmd: SwapCmd, config: &crate::config::Config) -> anyhow::Result<()> {
    match cmd.command {
        SwapCommand::Init(args) => init_swap(args, config).await,
        SwapCommand::Status(args) => check_status(args, config).await,
        SwapCommand::Claim(args) => claim_swap(args, config).await,
        SwapCommand::Refund(args) => refund_swap(args, config).await,
        SwapCommand::List(args) => list_swaps(args, config).await,
        SwapCommand::Monitor(args) => monitor_swaps(args, config).await,
    }
}

/// Initialize a new atomic swap
async fn init_swap(args: InitSwapArgs, config: &crate::config::Config) -> anyhow::Result<()> {
    println!("{}", "Initializing atomic swap...".yellow());
    
    // Prepare RPC request
    let params = serde_json::json!({
        "bitcoin_amount": args.btc_amount,
        "nova_amount": args.nova_amount,
        "bitcoin_counterparty": args.btc_counterparty,
        "nova_counterparty": args.nova_counterparty,
        "timeout_minutes": args.timeout_minutes,
    });
    
    // Make RPC call
    let client = crate::rpc::RpcClient::new(config.rpc_url.clone(), config.timeout)?;
    let response: serde_json::Value = client
        .call("initiate_swap", params)
        .await?;
    
    // Parse response
    if let Some(swap_id) = response["swap_id"].as_str() {
        println!("{}", "✓ Swap initialized successfully!".green());
        println!("\n{}", "Swap Details:".bold());
        println!("  Swap ID: {}", swap_id.cyan());
        println!("  BTC Amount: {} sats", args.btc_amount);
        println!("  NOVA Amount: {} NOVA", args.nova_amount);
        println!("  State: {}", "Active".yellow());
        
        if let Some(btc_address) = response["bitcoin_htlc_address"].as_str() {
            println!("\n{}", "Next Steps:".bold());
            println!("  1. Send {} BTC to: {}", 
                (args.btc_amount as f64 / 100_000_000.0), 
                btc_address.cyan()
            );
            println!("  2. Wait for confirmations");
            println!("  3. Monitor the swap: {}", 
                format!("supernova swap monitor --swap-id {}", swap_id).cyan()
            );
        }
    } else {
        println!("{}", "✗ Failed to initialize swap".red());
        if let Some(error) = response["error"]["message"].as_str() {
            println!("  Error: {}", error);
        }
    }
    
    Ok(())
}

/// Check swap status
async fn check_status(args: StatusArgs, config: &crate::config::Config) -> anyhow::Result<()> {
    let swap_id_bytes = hex::decode(&args.swap_id)?;
    if swap_id_bytes.len() != 32 {
        anyhow::bail!("Invalid swap ID length");
    }
    
    // Make RPC call
    let client = crate::rpc::RpcClient::new(config.rpc_url.clone(), config.timeout)?;
    let response: serde_json::Value = client
        .call("get_swap_status", json!([args.swap_id]))
        .await?;
    
    // Display status
    println!("{}", format!("Swap Status: {}", args.swap_id).bold());
    println!("{}", "─".repeat(50));
    
    if let Some(state) = response["state"].as_str() {
        let state_display = match state {
            "Active" => state.yellow(),
            "Claimed" => state.green(),
            "Refunded" => state.red(),
            _ => state.normal(),
        };
        println!("State: {}", state_display);
    }
    
    if let Some(btc_amount) = response["bitcoin_amount"].as_u64() {
        println!("BTC Amount: {} sats", btc_amount);
    }
    
    if let Some(nova_amount) = response["nova_amount"].as_u64() {
        println!("NOVA Amount: {} NOVA", nova_amount);
    }
    
    if let Some(btc_conf) = response["bitcoin_confirmations"].as_u64() {
        println!("Bitcoin Confirmations: {}", btc_conf);
    }
    
    if let Some(nova_conf) = response["nova_confirmations"].as_u64() {
        println!("Supernova Confirmations: {}", nova_conf);
    }
    
    if args.verbose {
        println!("\n{}", "Detailed Information:".bold());
        println!("{}", serde_json::to_string_pretty(&response)?);
    }
    
    Ok(())
}

/// Claim a swap
async fn claim_swap(args: ClaimArgs, config: &crate::config::Config) -> anyhow::Result<()> {
    let swap_id_bytes = hex::decode(&args.swap_id)?;
    let secret_bytes = hex::decode(&args.secret)?;
    
    if swap_id_bytes.len() != 32 {
        anyhow::bail!("Invalid swap ID length");
    }
    if secret_bytes.len() != 32 {
        anyhow::bail!("Invalid secret length");
    }
    
    println!("{}", "Claiming swap...".yellow());
    
    // Make RPC call
    let client = crate::rpc::RpcClient::new(config.rpc_url.clone(), config.timeout)?;
    let response: serde_json::Value = client
        .call("claim_swap", json!([args.swap_id, args.secret]))
        .await?;
    
    if let Some(txid) = response["txid"].as_str() {
        println!("{}", "✓ Swap claimed successfully!".green());
        println!("  Transaction ID: {}", txid.cyan());
    } else {
        println!("{}", "✗ Failed to claim swap".red());
        if let Some(error) = response["error"]["message"].as_str() {
            println!("  Error: {}", error);
        }
    }
    
    Ok(())
}

/// Refund a swap
async fn refund_swap(args: RefundArgs, config: &crate::config::Config) -> anyhow::Result<()> {
    let swap_id_bytes = hex::decode(&args.swap_id)?;
    if swap_id_bytes.len() != 32 {
        anyhow::bail!("Invalid swap ID length");
    }
    
    println!("{}", "Refunding swap...".yellow());
    
    // Make RPC call
    let client = crate::rpc::RpcClient::new(config.rpc_url.clone(), config.timeout)?;
    let response: serde_json::Value = client
        .call("refund_swap", json!([args.swap_id]))
        .await?;
    
    if let Some(txid) = response["txid"].as_str() {
        println!("{}", "✓ Swap refunded successfully!".green());
        println!("  Transaction ID: {}", txid.cyan());
    } else {
        println!("{}", "✗ Failed to refund swap".red());
        if let Some(error) = response["error"]["message"].as_str() {
            println!("  Error: {}", error);
        }
    }
    
    Ok(())
}

/// List swaps
async fn list_swaps(args: ListArgs, config: &crate::config::Config) -> anyhow::Result<()> {
    // Prepare filter
    let filter = serde_json::json!({
        "state": args.state,
        "hours": args.hours,
    });
    
    // Make RPC call
    let client = crate::rpc::RpcClient::new(config.rpc_url.clone(), config.timeout)?;
    let response: serde_json::Value = client
        .call("list_swaps", filter)
        .await?;
    
    if let Some(swaps) = response.as_array() {
        if swaps.is_empty() {
            println!("No swaps found matching criteria.");
        } else {
            println!("{}", format!("Found {} swap(s):", swaps.len()).bold());
            println!("{}", "─".repeat(80));
            
            for swap in swaps {
                if let Some(swap_id) = swap["swap_id"].as_str() {
                    let state = swap["state"].as_str().unwrap_or("Unknown");
                    let state_display = match state {
                        "Active" => state.yellow(),
                        "Claimed" => state.green(),
                        "Refunded" => state.red(),
                        _ => state.normal(),
                    };
                    
                    println!("ID: {} | State: {} | BTC: {} | NOVA: {}",
                        &swap_id[..16].cyan(),
                        state_display,
                        swap["bitcoin_amount"].as_u64().unwrap_or(0),
                        swap["nova_amount"].as_u64().unwrap_or(0),
                    );
                }
            }
        }
    }
    
    Ok(())
}

/// Monitor swaps in real-time
async fn monitor_swaps(args: MonitorArgs, config: &crate::config::Config) -> anyhow::Result<()> {
    use tokio_tungstenite::{connect_async, tungstenite::Message};
    use futures_util::{StreamExt, SinkExt};
    
    println!("{}", "Connecting to swap monitor...".yellow());
    
    // Connect to WebSocket
    let ws_url = "ws://localhost:8545/ws/swaps"; // Configure this
    let (ws_stream, _) = connect_async(ws_url).await?;
    let (mut write, mut read) = ws_stream.split();
    
    // Subscribe to events
    let subscribe_msg = if let Some(swap_id) = args.swap_id {
        serde_json::json!({
            "type": "subscribe",
            "swap_id": swap_id,
        })
    } else {
        serde_json::json!({
            "type": "subscribe",
        })
    };
    
    write.send(Message::Text(subscribe_msg.to_string())).await?;
    
    println!("{}", "✓ Connected! Monitoring swap events...".green());
    println!("{}", "Press Ctrl+C to stop monitoring\n".dimmed());
    
    // Handle incoming messages
    while let Some(msg) = read.next().await {
        match msg? {
            Message::Text(text) => {
                if let Ok(event) = serde_json::from_str::<serde_json::Value>(&text) {
                    if args.format == "json" {
                        println!("{}", text);
                    } else {
                        display_event(&event);
                    }
                }
            }
            Message::Close(_) => break,
            _ => {}
        }
    }
    
    Ok(())
}

/// Display a swap event in human-readable format
fn display_event(event: &serde_json::Value) {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();
    
    let event_type = event["type"].as_str().unwrap_or("unknown");
    let time_str = format!("[{}]", chrono::Local::now().format("%H:%M:%S"));
    
    match event_type {
        "swap_initiated" => {
            println!("{} {} New swap initiated: {}",
                time_str.dimmed(),
                "●".green(),
                event["swap_id"].as_str().unwrap_or("").cyan()
            );
        }
        "htlc_funded" => {
            println!("{} {} HTLC funded on {}: {}",
                time_str.dimmed(),
                "●".blue(),
                event["chain"].as_str().unwrap_or(""),
                event["tx_id"].as_str().unwrap_or("")
            );
        }
        "secret_revealed" => {
            println!("{} {} Secret revealed on Bitcoin!",
                time_str.dimmed(),
                "●".yellow()
            );
        }
        "swap_claimed" => {
            println!("{} {} Swap claimed successfully!",
                time_str.dimmed(),
                "✓".green()
            );
        }
        "swap_refunded" => {
            println!("{} {} Swap refunded",
                time_str.dimmed(),
                "↩".red()
            );
        }
        _ => {
            println!("{} {} {}: {}",
                time_str.dimmed(),
                "●".white(),
                event_type,
                serde_json::to_string(event).unwrap_or_default()
            );
        }
    }
}

 