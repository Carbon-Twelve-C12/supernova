// SuperNova CLI Client
// This binary provides a command-line interface for interacting with the SuperNova blockchain

mod config;
mod rpc;
mod wallet;
mod commands;

use anyhow::Result;
use clap::{Parser, Subcommand};
use colored::*;
use env_logger::Env;

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
    println!("{}", "    CLI v1.0.0 - Command-line interface for the Supernova blockchain".bright_white());
    println!();
}

#[derive(Parser)]
#[command(
    name = "supernova",
    version = "1.0.0",
    about = "Supernova Blockchain CLI",
    long_about = "Command-line interface for interacting with the Supernova blockchain network"
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
    
    /// Override RPC URL
    #[arg(long, global = true, env = "SUPERNOVA_RPC_URL")]
    rpc_url: Option<String>,
    
    /// Override network
    #[arg(long, global = true, env = "SUPERNOVA_NETWORK")]
    network: Option<String>,
    
    /// Output format (json, table, text)
    #[arg(short, long, global = true)]
    format: Option<String>,
    
    /// Enable debug output
    #[arg(short, long, global = true)]
    debug: bool,
    
    /// Skip banner display
    #[arg(long, global = true, hide = true)]
    no_banner: bool,
}

#[derive(Subcommand)]
enum Commands {
    /// Blockchain information and status
    #[command(subcommand)]
    Blockchain(BlockchainCommands),
    
    /// Wallet management
    #[command(subcommand)]
    Wallet(WalletCommands),
    
    /// Transaction operations
    #[command(subcommand)]
    Transaction(TransactionCommands),
    
    /// Mining operations
    #[command(subcommand)]
    Mining(MiningCommands),
    
    /// Configuration management
    #[command(subcommand)]
    Config(ConfigCommands),
    
    /// Show blockchain status (alias for blockchain status)
    Status,
    
    /// Send NOVA (alias for transaction send)
    Send {
        /// Recipient address
        to: String,
        /// Amount to send
        amount: f64,
    },
    
    /// Check balance (alias for wallet balance)
    Balance {
        /// Address to check (optional)
        address: Option<String>,
    },
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
async fn main() -> Result<()> {
    let cli = Cli::parse();
    
    // Display banner unless in JSON mode or explicitly disabled
    if !cli.no_banner && cli.format.as_deref() != Some("json") {
        print_banner();
    }
    
    // Initialize logger
    let env = if cli.debug {
        Env::default().default_filter_or("debug")
    } else {
        Env::default().default_filter_or("info")
    };
    env_logger::init_from_env(env);
    
    // Load configuration
    let mut config = config::Config::load()?;
    
    // Override with CLI arguments
    if let Some(rpc_url) = cli.rpc_url {
        config.rpc_url = rpc_url;
    }
    if let Some(network) = cli.network {
        config.network = network;
    }
    if let Some(format) = cli.format {
        config.output_format = match format.to_lowercase().as_str() {
            "json" => config::OutputFormat::Json,
            "table" => config::OutputFormat::Table,
            "text" => config::OutputFormat::Text,
            _ => {
                eprintln!("{} Invalid format: {}. Using default.", "✗".red().bold(), format);
                config.output_format
            }
        };
    }
    if cli.debug {
        config.debug = true;
    }
    
    // Execute command
    match cli.command {
        Commands::Blockchain(cmd) => match cmd {
            BlockchainCommands::Status => commands::blockchain::status(&config).await?,
            BlockchainCommands::Peers => commands::blockchain::peers(&config).await?,
            BlockchainCommands::Mempool => commands::blockchain::mempool(&config).await?,
            BlockchainCommands::Environmental => commands::blockchain::environmental(&config).await?,
        },
        
        Commands::Wallet(cmd) => match cmd {
            WalletCommands::Create { name } => commands::wallet::create(&config, name).await?,
            WalletCommands::Import { name } => commands::wallet::import(&config, name).await?,
            WalletCommands::List => commands::wallet::list(&config).await?,
            WalletCommands::Balance { address } => commands::wallet::balance(&config, address).await?,
            WalletCommands::NewAddress { wallet } => commands::wallet::new_address(&config, wallet).await?,
            WalletCommands::Export { wallet } => commands::wallet::export(&config, wallet).await?,
        },
        
        Commands::Transaction(cmd) => match cmd {
            TransactionCommands::Send { to, amount } => commands::transaction::send(&config, to, amount).await?,
            TransactionCommands::Get { txid } => commands::transaction::get(&config, txid).await?,
            TransactionCommands::History { address } => commands::transaction::history(&config, address).await?,
        },
        
        Commands::Mining(cmd) => match cmd {
            MiningCommands::Status => commands::mining::status(&config).await?,
            MiningCommands::Start { threads } => commands::mining::start(&config, threads).await?,
            MiningCommands::Stop => commands::mining::stop(&config).await?,
            MiningCommands::Benchmark => commands::mining::benchmark(&config).await?,
        },
        
        Commands::Config(cmd) => match cmd {
            ConfigCommands::Show => commands::config::show(&config).await?,
            ConfigCommands::Set { key, value } => commands::config::set(key, value).await?,
            ConfigCommands::Reset => commands::config::reset().await?,
            ConfigCommands::Interactive => commands::config::interactive().await?,
        },
        
        // Aliases
        Commands::Status => commands::blockchain::status(&config).await?,
        Commands::Send { to, amount } => commands::transaction::send(&config, to, amount).await?,
        Commands::Balance { address } => commands::wallet::balance(&config, address).await?,
    }
    
    Ok(())
} 