use clap::{Parser, Subcommand};
use std::path::{Path, PathBuf};
use tracing::{info, error};
use crate::core::{Wallet, WalletError, UTXO};
use crate::hdwallet::{HDWallet, HDWalletError};
use crate::history::{TransactionHistory, TransactionDirection};
use crate::ui::tui::WalletTui;
use crate::network::NetworkClient;
use rpassword::read_password;
use std::io::{self, Write};
use std::fs;

mod core;
mod network;
mod ui;
mod hdwallet;
mod history;

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
    New {
        /// Create HD wallet with mnemonic
        #[arg(long)]
        hd: bool,
    },
    
    /// Restore wallet from mnemonic
    Restore {
        /// Path to use for restored wallet
        #[arg(long)]
        path: Option<PathBuf>,
    },
    
    /// Get wallet address
    Address {
        /// Account index (for HD wallet)
        #[arg(long, default_value = "0")]
        account: u32,
        
        /// Generate new address
        #[arg(long)]
        new: bool,
    },
    
    /// Get wallet balance
    Balance {
        /// Account index (for HD wallet)
        #[arg(long, default_value = "0")]
        account: u32,
    },
    
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
        
        /// Account to send from (for HD wallet)
        #[arg(long, default_value = "0")]
        account: u32,
        
        /// Transaction label
        #[arg(long)]
        label: Option<String>,
    },
    
    /// List all UTXOs
    ListUtxos {
        /// Account index (for HD wallet)
        #[arg(long, default_value = "0")]
        account: u32,
    },
    
    /// Manage accounts (HD wallet only)
    Account {
        /// Create a new account
        #[arg(long)]
        create: bool,
        
        /// Account name
        #[arg(long)]
        name: Option<String>,
        
        /// List all accounts
        #[arg(long)]
        list: bool,
    },
    
    /// View transaction history
    History {
        /// Filter by type (incoming/outgoing/all)
        #[arg(long, default_value = "all")]
        filter: String,
        
        /// Account index
        #[arg(long)]
        account: Option<u32>,
    },
    
    /// Manage transaction labels
    Label {
        /// Transaction hash
        #[arg(long)]
        tx: String,
        
        /// Label text
        #[arg(long)]
        text: Option<String>,
        
        /// Remove label
        #[arg(long)]
        remove: bool,
    },

    /// Launch interactive TUI mode
    Tui,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::fmt::init();

    // Parse command line arguments
    let cli = Cli::parse();

    // Check if we're using an HD wallet
    let is_hd = cli.wallet.to_string_lossy().contains("hd");
    
    // Initialize network client
    let network = NetworkClient::new(cli.node);

    // Handle commands
    match cli.command {
        Commands::New { hd } => {
            if hd {
                // Create HD wallet
                info!("Creating new HD wallet at {:?}", cli.wallet);
                
                // Get password for encryption
                println!("Enter a password to encrypt the wallet:");
                let password = read_password()?;
                
                // Create wallet
                let wallet = HDWallet::new(&password)?;
                
                // Save wallet to file
                save_hd_wallet(&wallet, &cli.wallet)?;
                
                // Create empty transaction history
                let history = TransactionHistory::new();
                save_transaction_history(&history, &transaction_history_path(&cli.wallet)?)?;
                
                // Display mnemonic for backup
                println!("Your wallet has been created!");
                println!("\nIMPORTANT: Write down your mnemonic phrase and keep it safe. It's the only way to recover your wallet if you lose access to this file.\n");
                
                let mnemonic = wallet.get_mnemonic()?;
                println!("Mnemonic: {}", mnemonic);
                
                // Show first address
                let address = wallet.get_account(0)
                    .and_then(|account| account.addresses.values().next())
                    .map(|addr| &addr.address)
                    .unwrap_or(&"No address generated".to_string());
                    
                println!("\nFirst address: {}", address);
            } else {
                // Create legacy wallet
                info!("Creating new legacy wallet at {:?}", cli.wallet);
                let wallet = Wallet::new(cli.wallet.clone())?;
                println!("Created new wallet with address: {}", wallet.get_address());
            }
        },
        
        Commands::Restore { path } => {
            let wallet_path = path.unwrap_or(cli.wallet);
            
            println!("Enter your mnemonic phrase:");
            let mut mnemonic = String::new();
            io::stdin().read_line(&mut mnemonic)?;
            mnemonic = mnemonic.trim().to_string();
            
            println!("Enter a password to encrypt the wallet:");
            let password = read_password()?;
            
            // Create HD wallet from mnemonic
            let wallet = HDWallet::from_mnemonic(&mnemonic, &password)?;
            
            // Save wallet to file
            save_hd_wallet(&wallet, &wallet_path)?;
            
            // Create empty transaction history
            let history = TransactionHistory::new();
            save_transaction_history(&history, &transaction_history_path(&wallet_path)?)?;
            
            println!("Wallet restored successfully to {:?}", wallet_path);
            
            // Show first address
            let address = wallet.get_account(0)
                .and_then(|account| account.addresses.values().next())
                .map(|addr| &addr.address)
                .unwrap_or(&"No address generated".to_string());
                
            println!("First address: {}", address);
        },

        Commands::Address { account, new } => {
            if is_hd {
                // Load HD wallet
                let mut wallet = load_hd_wallet(&cli.wallet)?;
                
                if new {
                    // Generate new address
                    let address = wallet.new_receiving_address(account)?;
                    save_hd_wallet(&wallet, &cli.wallet)?;
                    println!("New address generated: {}", address.address);
                } else {
                    // Get existing addresses
                    if let Some(acct) = wallet.get_account(account) {
                        println!("Addresses for account #{} ({}):", account, acct.name);
                        
                        for addr in acct.get_receiving_addresses() {
                            println!("  {}", addr.address);
                        }
                    } else {
                        println!("Account #{} not found", account);
                    }
                }
            } else {
                // Legacy wallet
                let wallet = Wallet::load(cli.wallet)?;
                println!("Wallet address: {}", wallet.get_address());
            }
        },

        Commands::Balance { account } => {
            if is_hd {
                // Load HD wallet
                let wallet = load_hd_wallet(&cli.wallet)?;
                
                if account == 0 {
                    // Show total balance
                    println!("Total balance: {} NOVA", wallet.get_total_balance());
                    
                    // Show balance for each account
                    for acct in wallet.list_accounts() {
                        let balance = wallet.get_account_balance(acct.index);
                        println!("  Account #{} ({}): {} NOVA", acct.index, acct.name, balance);
                    }
                } else {
                    // Show balance for specific account
                    if let Some(acct) = wallet.get_account(account) {
                        let balance = wallet.get_account_balance(account);
                        println!("Balance for account #{} ({}): {} NOVA", account, acct.name, balance);
                    } else {
                        println!("Account #{} not found", account);
                    }
                }
            } else {
                // Legacy wallet
                let wallet = Wallet::load(cli.wallet)?;
                println!("Balance: {} NOVA", wallet.get_balance());
            }
        },

        Commands::Send { to, amount, fee, account, label } => {
            if is_hd {
                // Load HD wallet
                let mut wallet = load_hd_wallet(&cli.wallet)?;
                let mut history = load_transaction_history(&cli.wallet)?;
                
                // Create runtime for async operations
                let runtime = tokio::runtime::Runtime::new()?;
                
                runtime.block_on(async {
                    match wallet.send_from_account(account, &to, amount, fee, &network).await {
                        Ok((tx, tx_hash)) => {
                            println!("Transaction broadcast successfully!");
                            println!("Transaction ID: {}", hex::encode(tx_hash));
                            println!("Total amount (including fee): {}", amount + fee);
                            
                            // Add to transaction history
                            let record = history.add_transaction(
                                &tx, 
                                TransactionDirection::Outgoing,
                                amount,
                                fee,
                                &wallet
                            );
                            
                            // Add label if provided
                            if let Some(lbl) = label {
                                history.update_label(&record.tx_hash, Some(lbl));
                                println!("Added label: {}", lbl);
                            }
                            
                            // Save wallet and history
                            save_hd_wallet(&wallet, &cli.wallet)?;
                            save_transaction_history(&history, &transaction_history_path(&cli.wallet)?)?;
                            
                            Ok(())
                        },
                        Err(e) => {
                            error!("Failed to send transaction: {}", e);
                            Err(e.into())
                        }
                    }
                })?;
            } else {
                // Legacy wallet
                let wallet = Wallet::load(cli.wallet)?;
                
                // Create runtime for async operations
                let runtime = tokio::runtime::Runtime::new()?;
                
                runtime.block_on(async {
                    match wallet.send_transaction(&to, amount, fee, &network).await {
                        Ok(tx_hash) => {
                            println!("Transaction broadcast successfully!");
                            println!("Transaction ID: {}", hex::encode(tx_hash));
                            println!("Total amount (including fee): {}", amount + fee);
                            Ok(())
                        },
                        Err(e) => {
                            error!("Failed to send transaction: {}", e);
                            Err(e)
                        }
                    }
                })?;
            }
        },

        Commands::ListUtxos { account } => {
            if is_hd {
                // Load HD wallet
                let wallet = load_hd_wallet(&cli.wallet)?;
                
                let utxos = wallet.get_account_utxos(account);
                
                if utxos.is_empty() {
                    println!("No UTXOs found for account #{}", account);
                } else {
                    println!("UTXOs for account #{}:", account);
                    for (addr, utxo) in &utxos {
                        println!("  Address: {}", addr);
                        println!("  Transaction: {}", hex::encode(utxo.tx_hash));
                        println!("  Output Index: {}", utxo.output_index);
                        println!("  Amount: {} NOVA", utxo.amount);
                        println!();
                    }
                }
            } else {
                // Legacy wallet
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
        },
        
        Commands::Account { create, name, list } => {
            if !is_hd {
                println!("This command is only available for HD wallets.");
                return Ok(());
            }
            
            // Load HD wallet
            let mut wallet = load_hd_wallet(&cli.wallet)?;
            
            if create {
                // Create a new account
                let account_name = name.unwrap_or_else(|| "Account".to_string());
                let account = wallet.create_account(account_name)?;
                
                println!("Created new account #{} with name: {}", account.index, account.name);
                println!("Receiving address: {}", account.get_receiving_addresses().get(0)
                    .map(|addr| &addr.address)
                    .unwrap_or(&"No address generated".to_string()));
                
                // Save wallet
                save_hd_wallet(&wallet, &cli.wallet)?;
            } else if list {
                // List all accounts
                let accounts = wallet.list_accounts();
                
                println!("Accounts:");
                for account in accounts {
                    let balance = wallet.get_account_balance(account.index);
                    let address_count = account.addresses.len();
                    
                    println!("  #{}: \"{}\"", account.index, account.name);
                    println!("    Balance: {} NOVA", balance);
                    println!("    Addresses: {}", address_count);
                    
                    if !account.addresses.is_empty() {
                        let sample_address = account.addresses.values().next().unwrap();
                        println!("    Sample address: {}", sample_address.address);
                    }
                    
                    println!();
                }
            } else {
                println!("Please specify an action: --create or --list");
            }
        },
        
        Commands::History { filter, account } => {
            if !is_hd {
                println!("This command is only available for HD wallets.");
                return Ok(());
            }
            
            // Load transaction history
            let history = load_transaction_history(&cli.wallet)?;
            let wallet = load_hd_wallet(&cli.wallet)?;
            
            // Get transactions based on filter
            let transactions = match filter.as_str() {
                "incoming" => history.get_transactions_by_direction(TransactionDirection::Incoming),
                "outgoing" => history.get_transactions_by_direction(TransactionDirection::Outgoing),
                "self" => history.get_transactions_by_direction(TransactionDirection::SelfTransfer),
                _ => history.get_all_transactions(),
            };
            
            if transactions.is_empty() {
                println!("No transactions found.");
                return Ok(());
            }
            
            // Filter by account if specified
            let filtered_transactions = if let Some(acct) = account {
                // Get all addresses for the account
                let addresses = if let Some(account) = wallet.get_account(acct) {
                    account.addresses.keys().cloned().collect::<Vec<_>>()
                } else {
                    Vec::new()
                };
                
                // Filter transactions involving these addresses
                transactions.into_iter()
                    .filter(|tx| {
                        tx.addresses.iter().any(|addr| addresses.contains(addr))
                    })
                    .collect::<Vec<_>>()
            } else {
                transactions
            };
            
            // Display transactions
            println!("Transaction History:");
            for tx in filtered_transactions {
                let direction = match tx.direction {
                    TransactionDirection::Incoming => "Received",
                    TransactionDirection::Outgoing => "Sent",
                    TransactionDirection::SelfTransfer => "Self Transfer",
                };
                
                let status = match tx.status {
                    TransactionStatus::Pending => "Pending",
                    TransactionStatus::Confirmed(conf) => {
                        format!("Confirmed ({} confirmations)", conf).as_str()
                    },
                    TransactionStatus::Failed => "Failed",
                };
                
                println!("  Transaction: {}", tx.tx_hash);
                println!("  Type: {}", direction);
                println!("  Amount: {} NOVA", tx.amount);
                println!("  Fee: {} NOVA", tx.fee);
                println!("  Status: {}", status);
                
                if let Some(label) = &tx.label {
                    println!("  Label: {}", label);
                }
                
                println!();
            }
        },
        
        Commands::Label { tx, text, remove } => {
            if !is_hd {
                println!("This command is only available for HD wallets.");
                return Ok(());
            }
            
            // Load transaction history
            let mut history = load_transaction_history(&cli.wallet)?;
            
            if remove {
                // Remove label
                history.update_label(&tx, None);
                println!("Removed label from transaction {}", tx);
            } else if let Some(label) = text {
                // Add or update label
                history.update_label(&tx, Some(label.clone()));
                println!("Added label \"{}\" to transaction {}", label, tx);
            } else {
                // Display current label
                if let Some(record) = history.get_transaction(&tx) {
                    if let Some(label) = &record.label {
                        println!("Transaction {}: Label = \"{}\"", tx, label);
                    } else {
                        println!("Transaction {} has no label", tx);
                    }
                } else {
                    println!("Transaction {} not found in history", tx);
                }
            }
            
            // Save transaction history
            save_transaction_history(&history, &transaction_history_path(&cli.wallet)?)?;
        },

        Commands::Tui => {
            if is_hd {
                // Load HD wallet and history
                let wallet = load_hd_wallet(&cli.wallet)?;
                let history = load_transaction_history(&cli.wallet)?;
                
                // Launch TUI with HD wallet
                let mut tui = WalletTui::new(wallet, history)?;
                tui.run()?;
                
                // Save any changes when TUI exits
                save_hd_wallet(&tui.hd_wallet, &cli.wallet)?;
                save_transaction_history(&tui.transaction_history, &transaction_history_path(&cli.wallet)?)?;
            } else {
                // Legacy wallet TUI
                let wallet = Wallet::load(cli.wallet)?;
                let mut tui = WalletTui::new(wallet)?;
                tui.run()?;
            }
        }
    }

    Ok(())
}

// Helper functions for file operations

fn load_hd_wallet(path: &Path) -> Result<HDWallet, Box<dyn std::error::Error>> {
    // Check if file exists
    if !path.exists() {
        return Err(format!("Wallet file not found: {:?}", path).into());
    }
    
    // Read file
    let data = fs::read_to_string(path)?;
    
    // Deserialize
    let wallet: HDWallet = serde_json::from_str(&data)?;
    
    // Prompt for password to unlock
    println!("Enter wallet password:");
    let password = read_password()?;
    
    // Unlock wallet
    let mut wallet = wallet;
    wallet.unlock(&password)?;
    
    Ok(wallet)
}

fn save_hd_wallet(wallet: &HDWallet, path: &Path) -> Result<(), Box<dyn std::error::Error>> {
    // Make sure directory exists
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    
    // Serialize
    let data = serde_json::to_string_pretty(wallet)?;
    
    // Write to file
    fs::write(path, data)?;
    
    Ok(())
}

fn transaction_history_path(wallet_path: &Path) -> Result<PathBuf, Box<dyn std::error::Error>> {
    let file_name = wallet_path.file_name()
        .ok_or("Invalid wallet path")?
        .to_string_lossy();
    
    let history_name = format!("{}_history.json", file_name);
    let history_path = wallet_path.with_file_name(history_name);
    
    Ok(history_path)
}

fn load_transaction_history(wallet_path: &Path) -> Result<TransactionHistory, Box<dyn std::error::Error>> {
    let history_path = transaction_history_path(wallet_path)?;
    
    // Check if file exists
    if !history_path.exists() {
        // Create empty history
        return Ok(TransactionHistory::new());
    }
    
    // Read file
    let data = fs::read_to_string(history_path)?;
    
    // Deserialize
    let history: TransactionHistory = serde_json::from_str(&data)?;
    
    Ok(history)
}

fn save_transaction_history(history: &TransactionHistory, path: &Path) -> Result<(), Box<dyn std::error::Error>> {
    // Make sure directory exists
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    
    // Serialize
    let data = serde_json::to_string_pretty(history)?;
    
    // Write to file
    fs::write(path, data)?;
    
    Ok(())
}