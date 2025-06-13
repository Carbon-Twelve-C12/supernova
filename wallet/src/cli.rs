use clap::{Parser, Subcommand, CommandFactory};
use std::path::PathBuf;
use bitcoin::network::Network;
use std::str::FromStr;
use crate::{
    hdwallet::{AccountType, HDWallet},
    ui::tui::WalletTui,
    history::{TransactionHistory, TransactionRecord, TransactionDirection, TransactionStatus},
};
use chrono::{Utc};
use btclib::storage::utxo_set::UtxoSet;

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
pub struct Cli {
    #[arg(short, long, default_value = "~/.supernova-wallet")]
    wallet_dir: String,

    #[arg(short, long, default_value = "testnet")]
    network: String,

    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Create a new wallet
    New,

    /// Load an existing wallet
    Load,

    /// Create a wallet from a mnemonic phrase
    FromMnemonic {
        /// Mnemonic phrase
        mnemonic: String,
    },

    /// List all accounts in the wallet
    ListAccounts,

    /// Create a new account
    CreateAccount {
        /// Account name
        name: String,

        /// Account type (legacy, segwit, native_segwit)
        #[arg(short, long, default_value = "native_segwit")]
        account_type: String,
    },

    /// Get a new address for an account
    GetNewAddress {
        /// Account index or name
        #[arg(short, long)]
        account: String,
    },

    /// Get balance for an account
    GetBalance {
        /// Account index or name
        #[arg(short, long)]
        account: String,
    },

    /// Run the TUI
    Tui,
    
    /// Create a test transaction (for development/demo)
    #[cfg(debug_assertions)]
    CreateTestTransaction {
        /// Account to credit
        #[arg(short, long)]
        account: String,
        
        /// Amount in satoshis
        #[arg(short, long, default_value = "50000")]
        amount: u64,
    },
}

pub fn run_cli() -> Result<(), String> {
    let cli = Cli::parse();
    
    // Parse network string to Network enum
    let network = match cli.network.to_lowercase().as_str() {
        "mainnet" | "bitcoin" => Network::Bitcoin,
        "testnet" => Network::Testnet,
        "regtest" => Network::Regtest,
        "signet" => Network::Signet,
        _ => return Err(format!("Invalid network: {}", cli.network)),
    };
    
    // Expand the wallet directory path
    let wallet_dir = shellexpand::tilde(&cli.wallet_dir).to_string();
    let wallet_dir = PathBuf::from(wallet_dir);
    
    // Create wallet directory if it doesn't exist
    if !wallet_dir.exists() {
        std::fs::create_dir_all(&wallet_dir)
            .map_err(|e| format!("Failed to create wallet directory: {}", e))?;
    }
    
    let wallet_path = wallet_dir.join("wallet.json");
    let history_path = wallet_dir.join("history.json");
    
    match cli.command {
        Some(Commands::New) => {
            println!("Creating new wallet...");
            let wallet = HDWallet::new(network, wallet_path)
                .map_err(|e| format!("Failed to create wallet: {}", e))?;
            
            println!("Wallet created successfully.");
            println!("Your mnemonic phrase (keep this safe!):");
            // In a real implementation, we'd get the mnemonic from the created wallet
            // Here we're just showing the phrase would be displayed
            println!("{}", wallet.get_mnemonic());
            
            // Create default account
            let mut wallet = wallet;
            wallet.create_account("default".to_string(), AccountType::NativeSegWit)
                .map_err(|e| format!("Failed to create default account: {}", e))?;
            
            println!("Default account created.");
            Ok(())
        },
        
        Some(Commands::Load) => {
            if !wallet_path.exists() {
                return Err("No wallet found. Create one first with 'new' command.".to_string());
            }
            
            let wallet = HDWallet::load(wallet_path)
                .map_err(|e| format!("Failed to load wallet: {}", e))?;
            
            println!("Wallet loaded successfully.");
            Ok(())
        },
        
        Some(Commands::FromMnemonic { mnemonic }) => {
            println!("Creating wallet from mnemonic...");
            let wallet = HDWallet::from_mnemonic(&mnemonic, network, wallet_path)
                .map_err(|e| format!("Failed to create wallet from mnemonic: {}", e))?;
            
            println!("Wallet created successfully.");
            
            // Create default account
            let mut wallet = wallet;
            wallet.create_account("default".to_string(), AccountType::NativeSegWit)
                .map_err(|e| format!("Failed to create default account: {}", e))?;
            
            println!("Default account created.");
            Ok(())
        },
        
        Some(Commands::ListAccounts) => {
            if !wallet_path.exists() {
                return Err("No wallet found. Create one first with 'new' command.".to_string());
            }
            
            let wallet = HDWallet::load(wallet_path)
                .map_err(|e| format!("Failed to load wallet: {}", e))?;
            
            let accounts = wallet.list_accounts();
            if accounts.is_empty() {
                println!("No accounts found.");
            } else {
                println!("Accounts:");
                for (idx, account) in accounts {
                    println!("{}. {} (type: {:?}, addresses: {})", 
                            idx, 
                            account.name, 
                            account.account_type,
                            account.addresses.len());
                }
            }
            Ok(())
        },
        
        Some(Commands::CreateAccount { name, account_type }) => {
            if !wallet_path.exists() {
                return Err("No wallet found. Create one first with 'new' command.".to_string());
            }
            
            let mut wallet = HDWallet::load(wallet_path)
                .map_err(|e| format!("Failed to load wallet: {}", e))?;
            
            let acc_type = AccountType::from_str(&account_type)
                .map_err(|e| format!("Invalid account type: {}", e))?;
            
            wallet.create_account(name.clone(), acc_type)
                .map_err(|e| format!("Failed to create account: {}", e))?;
            
            println!("Account '{}' of type {:?} created successfully.", name, acc_type);
            Ok(())
        },
        
        Some(Commands::GetNewAddress { account }) => {
            if !wallet_path.exists() {
                return Err("No wallet found. Create one first with 'new' command.".to_string());
            }
            
            let mut wallet = HDWallet::load(wallet_path)
                .map_err(|e| format!("Failed to load wallet: {}", e))?;
            
            let address = wallet.get_new_address(&account)
                .map_err(|e| format!("Failed to get new address: {}", e))?;
            
            println!("New address: {}", address.address);
            Ok(())
        },
        
        Some(Commands::GetBalance { account }) => {
            if !wallet_path.exists() {
                return Err("No wallet found. Create one first with 'new' command.".to_string());
            }
            
            let wallet = HDWallet::load(wallet_path)
                .map_err(|e| format!("Failed to load wallet: {}", e))?;
            
            // Create temporary UTXO set for balance checking
            let utxo_set = UtxoSet::new_in_memory(1000);
            
            let balance = wallet.get_balance(&account, &utxo_set)
                .map_err(|e| format!("Failed to get balance: {}", e))?;
            
            println!("Balance for '{}': {} satoshis", account, balance);
            Ok(())
        },
        
        Some(Commands::Tui) => {
            if !wallet_path.exists() {
                return Err("No wallet found. Create one first with 'new' command.".to_string());
            }
            
            let wallet = HDWallet::load(wallet_path)
                .map_err(|e| format!("Failed to load wallet: {}", e))?;
            
            let history = TransactionHistory::new(history_path)
                .map_err(|e| format!("Failed to load transaction history: {}", e))?;
            
            let mut tui = WalletTui::new(wallet, history)
                .map_err(|e| format!("Failed to create TUI: {}", e))?;
            
            tui.run().map_err(|e| format!("TUI error: {}", e))?;
            Ok(())
        },
        
        #[cfg(debug_assertions)]
        Some(Commands::CreateTestTransaction { account, amount }) => {
            if !wallet_path.exists() {
                return Err("No wallet found. Create one first with 'new' command.".to_string());
            }
            
            let wallet = HDWallet::load(wallet_path)
                .map_err(|e| format!("Failed to load wallet: {}", e))?;
            
            let mut history = TransactionHistory::new(history_path)
                .map_err(|e| format!("Failed to load transaction history: {}", e))?;
                
            // Create temporary UTXO set for balance checking
            let utxo_set = UtxoSet::new_in_memory(1000);
            
            // Validate the account exists
            if wallet.get_balance(&account, &utxo_set).is_err() {
                return Err(format!("Account '{}' not found", account));
            }
            
            // Create a test transaction record
            let tx_record = TransactionRecord {
                hash: format!("test_tx_{}", Utc::now().timestamp()),
                timestamp: Utc::now(),
                direction: TransactionDirection::Received,
                amount,
                fee: 1000,
                status: TransactionStatus::Confirmed(6),
                label: Some(format!("Test transaction to {}", account)),
                category: Some("Test".to_string()),
                tags: vec!["test".to_string(), "demo".to_string()],
            };
            
            history.add_transaction(tx_record)
                .map_err(|e| format!("Failed to add transaction: {}", e))?;
                
            println!("Test transaction of {} sats created for account '{}'", amount, account);
            Ok(())
        },
        
        None => {
            // No command provided, show help
            Cli::command().print_help().map_err(|e| format!("Failed to print help: {}", e))?;
            Ok(())
        }
    }
} 