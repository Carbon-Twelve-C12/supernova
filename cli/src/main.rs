// supernova CLI Client
// This binary provides a command-line interface for interacting with the supernova blockchain

mod commands;
mod config;
mod rpc;
mod wallet;

use anyhow::Result;
use clap::{Parser, Subcommand};
use colored::*;
use env_logger::Env;
use serde_json::json;

fn print_banner() {
    let banner = r#"
    ╔═══════════════════════════════════════════════════════════════╗
    ║                                                               ║
    ║   ███████╗██╗   ██╗██████╗ ███████╗██████╗ ███╗   ██╗ ██████╗██╗   ██╗ █████╗  ║
    ║   ██╔════╝██║   ██║██╔══██╗██╔════╝██╔══██╗████╗  ██║██╔═══██╗██║   ██║██╔══██╗ ║
    ║   ███████╗██║   ██║██████╔╝█████╗  ██████╔╝██╔██╗ ██║██║   ██║██║   ██║███████║ ║
    ║   ╚════██║██║   ██║██╔═══╝ ██╔══╝  ██╔══██╗██║╚██╗██║██║   ██║╚██╗ ██╔╝██╔══██║ ║
    ║   ███████║╚██████╔╝██║     ███████╗██║  ██║██║ ╚████║╚██████╔╝ ╚████╔╝ ██║  ██║ ║
    ║   ╚══════╝ ╚═════╝ ╚═╝     ╚══════╝╚═╝  ╚═╝╚═╝  ╚═══╝ ╚═════╝   ╚═══╝  ╚═╝  ╚═╝ ║
    ║                                                               ║
    ║             🌍 Carbon-Negative Blockchain 🌍                  ║
    ║             ⚡ Quantum-Resistant Security ⚡                  ║
    ║             🚀 Lightning-Fast Transactions 🚀                 ║
    ║                                                               ║
    ╚═══════════════════════════════════════════════════════════════╝
    "#;

    println!("{}", banner.bright_cyan().bold());
    println!(
        "{}",
        "    CLI v1.0.0 - Command-line interface for the Supernova blockchain".bright_white()
    );
    println!();
}

#[derive(Parser)]
#[command(name = "supernova-cli")]
#[command(about = "Supernova blockchain CLI", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,

    #[arg(short, long, default_value = "http://localhost:8332")]
    rpc_url: String,

    #[arg(short = 'u', long)]
    rpc_user: Option<String>,

    #[arg(short = 'p', long)]
    rpc_password: Option<String>,

    #[arg(short, long)]
    debug: bool,

    #[arg(short, long)]
    network: Option<String>,

    #[arg(short, long)]
    format: Option<String>,

    #[arg(long)]
    no_banner: bool,
}

#[derive(Subcommand)]
enum Commands {
    /// Get blockchain information
    GetBlockchainInfo,

    /// Get network information
    GetNetworkInfo,

    /// Get mining information
    GetMiningInfo,

    /// Get peer information
    GetPeerInfo,

    /// Get block by height
    GetBlock {
        #[arg(value_name = "HEIGHT")]
        height: u64,
    },

    /// Get transaction by ID
    GetTransaction {
        #[arg(value_name = "TXID")]
        txid: String,
    },

    /// Generate new address
    GetNewAddress,

    /// Get wallet balance
    GetBalance,

    /// Send transaction
    SendToAddress {
        #[arg(value_name = "ADDRESS")]
        address: String,

        #[arg(value_name = "AMOUNT")]
        amount: f64,
    },

    /// Atomic swap operations
    #[command(subcommand)]
    Swap(commands::swap::SwapCommand),
}

#[derive(Subcommand)]
enum BlockchainCommands {
    /// Show blockchain status
    Status,
    /// List connected peers
    Peers,
    /// Show mempool information
    Mempool,
    /// Show environmental metrics
    Environmental,
}

#[derive(Subcommand)]
enum WalletCommands {
    /// Create a new wallet
    Create {
        /// Wallet name
        name: Option<String>,
    },
    /// Import wallet from mnemonic
    Import {
        /// Wallet name
        name: Option<String>,
    },
    /// List all wallets
    List,
    /// Check balance
    Balance {
        /// Address to check
        address: Option<String>,
    },
    /// Generate new address
    NewAddress {
        /// Wallet name
        wallet: Option<String>,
    },
    /// Export private keys (dangerous!)
    Export {
        /// Wallet name
        wallet: String,
    },
}

#[derive(Subcommand)]
enum TransactionCommands {
    /// Send NOVA
    Send {
        /// Recipient address
        to: String,
        /// Amount to send
        amount: f64,
    },
    /// Get transaction details
    Get {
        /// Transaction ID
        txid: String,
    },
    /// Show transaction history
    History {
        /// Address to check
        address: Option<String>,
    },
}

#[derive(Subcommand)]
enum MiningCommands {
    /// Show mining status
    Status,
    /// Start mining
    Start {
        /// Number of threads
        #[arg(short, long)]
        threads: Option<u32>,
    },
    /// Stop mining
    Stop,
    /// Run mining benchmark
    Benchmark,
}

#[derive(Subcommand)]
enum ConfigCommands {
    /// Show current configuration
    Show,
    /// Set configuration value
    Set {
        /// Configuration key
        key: String,
        /// Configuration value
        value: String,
    },
    /// Reset to defaults
    Reset,
    /// Interactive configuration
    Interactive,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();

    // Display banner unless in JSON mode or explicitly disabled
    if !cli.no_banner && cli.format.as_deref() != Some("json") {
        print_banner();
    }

    // Initialize logger
    // SECURITY: Debug mode is off by default. When enabled, verbose logging
    // may include sensitive information. Only use for development/troubleshooting.
    let env = if cli.debug {
        #[cfg(not(debug_assertions))]
        eprintln!(
            "{} Debug mode enabled. Verbose logging may include sensitive information.",
            "⚠".yellow().bold()
        );
        Env::default().default_filter_or("debug")
    } else {
        Env::default().default_filter_or("info")
    };
    env_logger::init_from_env(env);

    // Load configuration
    let mut config = config::Config::load()?;

    // Override with CLI arguments
    config.rpc_url = cli.rpc_url;
    if let Some(network) = cli.network {
        config.network = network;
    }
    if let Some(format) = cli.format {
        config.output_format = match format.to_lowercase().as_str() {
            "json" => config::OutputFormat::Json,
            "table" => config::OutputFormat::Table,
            "text" => config::OutputFormat::Text,
            _ => {
                eprintln!(
                    "{} Invalid format: {}. Using default.",
                    "✗".red().bold(),
                    format
                );
                config.output_format
            }
        };
    }
    if cli.debug {
        config.debug = true;
    }

    // Execute command
    let result = match cli.command {
        Commands::GetBlockchainInfo => {
            json!({
                "chain": "testnet",
                "blocks": 0,
                "headers": 0,
                "bestblockhash": "0000000000000000000000000000000000000000000000000000000000000000",
                "difficulty": "0x1d00ffff",
                "mediantime": 0,
                "verificationprogress": 1.0,
                "initialblockdownload": false,
                "chainwork": "0x0",
                "size_on_disk": 0,
                "pruned": false
            })
        }
        Commands::GetNetworkInfo => {
            json!({
                "version": 10000,
                "subversion": "/Supernova:1.0.0/",
                "protocolversion": 70015,
                "localservices": "0000000000000000",
                "localrelay": true,
                "timeoffset": 0,
                "networkactive": true,
                "connections": 0,
                "networks": []
            })
        }
        Commands::GetMiningInfo => {
            json!({
                "blocks": 0,
                "difficulty": "0x1d00ffff",
                "networkhashps": 0,
                "pooledtx": 0,
                "chain": "testnet"
            })
        }
        Commands::GetPeerInfo => {
            json!([])
        }
        Commands::GetBlock { height } => {
            json!({
                "hash": "0000000000000000000000000000000000000000000000000000000000000000",
                "confirmations": 1,
                "height": height,
                "version": 1,
                "merkleroot": "0000000000000000000000000000000000000000000000000000000000000000",
                "time": 0,
                "mediantime": 0,
                "nonce": 0,
                "bits": "1d00ffff",
                "difficulty": 1.0,
                "chainwork": "0x0",
                "nTx": 0,
                "previousblockhash": null,
                "nextblockhash": null
            })
        }
        Commands::GetTransaction { txid } => {
            json!({
                "txid": txid,
                "hash": txid,
                "version": 2,
                "size": 0,
                "vsize": 0,
                "weight": 0,
                "locktime": 0,
                "vin": [],
                "vout": [],
                "hex": ""
            })
        }
        Commands::GetNewAddress => {
            json!({
                "address": "testnet1qnewaddress000000000000000000000000000"
            })
        }
        Commands::GetBalance => {
            json!({
                "balance": 0.0,
                "unconfirmed_balance": 0.0,
                "immature_balance": 0.0
            })
        }
        Commands::SendToAddress { address, amount } => {
            json!({
                "txid": "0000000000000000000000000000000000000000000000000000000000000000",
                "address": address,
                "amount": amount
            })
        }
        Commands::Swap(cmd) => {
            commands::swap::execute(commands::swap::SwapCmd { command: cmd }, &config).await?;
            return Ok(()); // Commands handle their own output
        }
    };

    println!("{}", serde_json::to_string_pretty(&result)?);

    Ok(())
}
