use clap::{Parser, Subcommand};
use std::path::PathBuf;
use tracing::{info, error};
use crate::core::{Wallet, WalletError, UTXO};
use crate::ui::tui::WalletTui;
use crate::network::NetworkClient;

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Cli {
    /// Optional path to wallet file
    #[arg(long, default_value = "wallet.json")]
    wallet: PathBuf,

    /// Node address for network communication
    #[arg(long, default_value = "127.0.0.1:8000")]
    node: String,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Create a new wallet
    New,
    
    /// Get wallet address
    Address,
    
    /// Get wallet balance
    Balance,
    
    /// Send NOVA to an address
    Send {
        /// Recipient address
        #[arg(long)]
        to: String,
        
        /// Amount to send (in NOVA)
        #[arg(long)]
        amount: u64,
        
        /// Transaction fee (in NOVA)
        #[arg(long, default_value = "1")]
        fee: u64,
    },
    
    /// List all UTXOs
    ListUtxos,

    /// Launch interactive TUI mode
    Tui,
}

fn main() -> Result<(), WalletError> {
    // Initialize logging
    tracing_subscriber::fmt::init();

    // Parse command line arguments
    let cli = Cli::parse();

    // Initialize network client
    let network = NetworkClient::new(cli.node);

    // Handle commands
    match cli.command {
        Commands::New => {
            info!("Creating new wallet at {:?}", cli.wallet);
            let wallet = Wallet::new(cli.wallet)?;
            println!("Created new wallet with address: {}", wallet.get_address());
        }

        Commands::Address => {
            let wallet = Wallet::load(cli.wallet)?;
            println!("Wallet address: {}", wallet.get_address());
        }

        Commands::Balance => {
            let wallet = Wallet::load(cli.wallet)?;
            println!("Balance: {} NOVA", wallet.get_balance());
        }

        Commands::Send { to, amount, fee } => {
            let wallet = Wallet::load(cli.wallet)?;
            
            // Create runtime for async operations
            let runtime = tokio::runtime::Runtime::new()?;
            
            runtime.block_on(async {
                match wallet.send_transaction(&to, amount, fee, &network).await {
                    Ok(tx_hash) => {
                        println!("Transaction broadcast successfully!");
                        println!("Transaction ID: {}", hex::encode(tx_hash));
                        println!("Total amount (including fee): {}", amount + fee);
                    }
                    Err(e) => {
                        error!("Failed to send transaction: {}", e);
                        return Err(e);
                    }
                }
                Ok(())
            })?;
        }

        Commands::ListUtxos => {
            let wallet = Wallet::load(cli.wallet)?;
            println!("Unspent Transaction Outputs:");
            for utxos in wallet.utxos.values() {
                for utxo in utxos {
                    println!("  Transaction: {}", hex::encode(utxo.tx_hash));
                    println!("  Output Index: {}", utxo.output_index);
                    println!("  Amount: {} NOVA", utxo.amount);
                    println!();
                }
            }
        }

        Commands::Tui => {
            let wallet = Wallet::load(cli.wallet)?;
            let mut tui = WalletTui::new(wallet).map_err(|e| {
                error!("Failed to create TUI: {}", e);
                WalletError::Io(e)
            })?;
            tui.run().map_err(|e| {
                error!("TUI error: {}", e);
                WalletError::Io(e)
            })?;
        }
    }

    Ok(())
}