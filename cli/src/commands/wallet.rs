use crate::commands::{print_error, print_info, print_success, print_warning};
use crate::config::{Config, OutputFormat};
use crate::rpc::RpcClient;
use crate::wallet::WalletManager;
use anyhow::Result;
use colored::*;
use dialoguer::{Confirm, Input, Password, Select};
use prettytable::{Cell, Row, Table};

pub async fn create(config: &Config, name: Option<String>) -> Result<()> {
    let wallet_manager = WalletManager::new(Config::wallet_dir()?)?;

    // Get wallet name
    let wallet_name = if let Some(n) = name {
        n
    } else {
        Input::<String>::new()
            .with_prompt("Wallet name")
            .interact_text()?
    };

    // Check if wallet already exists
    let existing_wallets = wallet_manager.list_wallets()?;
    if existing_wallets.contains(&wallet_name) {
        print_error(&format!("Wallet '{}' already exists", wallet_name));
        return Ok(());
    }

    // Get network
    let network = Select::new()
        .with_prompt("Select network")
        .items(&["testnet", "mainnet", "devnet"])
        .default(0)
        .interact()?;

    let networks = ["testnet", "mainnet", "devnet"];
    let network_str = networks[network];

    // Create wallet
    let wallet = wallet_manager.create_wallet(&wallet_name, network_str)?;

    match &config.output_format {
        OutputFormat::Json => {
            println!(
                "{}",
                serde_json::json!({
                    "name": wallet.name,
                    "network": network_str,
                    "mnemonic": wallet.mnemonic,
                })
            );
        }
        _ => {
            print_success(&format!("Created wallet '{}'", wallet_name));
            println!("\n{}", "IMPORTANT: Save your recovery phrase!".bold().red());
            println!("{}", "=".repeat(50));
            println!("\n{}\n", wallet.mnemonic.yellow().bold());
            println!("{}", "=".repeat(50));
            print_warning("Write down this phrase and store it securely.");
            print_warning("You will need it to recover your wallet.");
            print_warning("Never share this phrase with anyone!");
        }
    }

    Ok(())
}

pub async fn import(_config: &Config, name: Option<String>) -> Result<()> {
    let wallet_manager = WalletManager::new(Config::wallet_dir()?)?;

    // Get wallet name
    let wallet_name = if let Some(n) = name {
        n
    } else {
        Input::<String>::new()
            .with_prompt("Wallet name")
            .interact_text()?
    };

    // Check if wallet already exists
    let existing_wallets = wallet_manager.list_wallets()?;
    if existing_wallets.contains(&wallet_name) {
        print_error(&format!("Wallet '{}' already exists", wallet_name));
        return Ok(());
    }

    // Get mnemonic
    let mnemonic = Password::new()
        .with_prompt("Enter recovery phrase")
        .interact()?;

    // Get network
    let network = Select::new()
        .with_prompt("Select network")
        .items(&["testnet", "mainnet", "devnet"])
        .default(0)
        .interact()?;

    let networks = ["testnet", "mainnet", "devnet"];
    let network_str = networks[network];

    // Import wallet
    match wallet_manager.import_wallet(&wallet_name, &mnemonic, network_str) {
        Ok(_) => {
            print_success(&format!("Imported wallet '{}'", wallet_name));
        }
        Err(e) => {
            print_error(&format!("Failed to import wallet: {}", e));
        }
    }

    Ok(())
}

pub async fn list(config: &Config) -> Result<()> {
    let wallet_manager = WalletManager::new(Config::wallet_dir()?)?;
    let wallets = wallet_manager.list_wallets()?;

    match &config.output_format {
        OutputFormat::Json => {
            println!("{}", serde_json::to_string_pretty(&wallets)?);
        }
        _ => {
            println!("\n{}", "Available Wallets".bold().green());
            println!("{}", "=".repeat(40));

            if wallets.is_empty() {
                print_info("No wallets found. Create one with 'supernova wallet create'");
            } else {
                for wallet_name in wallets {
                    if let Ok(wallet) = wallet_manager.load_wallet(&wallet_name) {
                        let info = wallet_manager.get_wallet_info(&wallet);
                        println!(
                            "• {} ({}, {} addresses)",
                            wallet_name.cyan(),
                            info.network,
                            info.address_count
                        );
                    }
                }
            }
        }
    }

    Ok(())
}

pub async fn balance(config: &Config, address: Option<String>) -> Result<()> {
    let client = RpcClient::new(config.rpc_url.clone(), config.timeout)?;

    let addr = if let Some(a) = address {
        a
    } else {
        // Try to get address from default wallet
        let wallet_manager = WalletManager::new(Config::wallet_dir()?)?;
        let wallets = wallet_manager.list_wallets()?;

        if wallets.is_empty() {
            print_error("No wallets found. Create one first or specify an address.");
            return Ok(());
        }

        // Select wallet
        let wallet_index = if wallets.len() == 1 {
            0
        } else {
            Select::new()
                .with_prompt("Select wallet")
                .items(&wallets)
                .interact()?
        };

        let wallet = wallet_manager.load_wallet(&wallets[wallet_index])?;

        if wallet.addresses.is_empty() {
            print_error("Wallet has no addresses. Generate one first.");
            return Ok(());
        }

        // Select address
        let addresses: Vec<String> = wallet
            .addresses
            .iter()
            .map(|a| format!("{}: {}", a.index, a.address))
            .collect();

        let addr_index = if addresses.len() == 1 {
            0
        } else {
            Select::new()
                .with_prompt("Select address")
                .items(&addresses)
                .interact()?
        };

        wallet.addresses[addr_index].address.clone()
    };

    // Get balance
    match client.get_balance(&addr).await {
        Ok(balance) => match &config.output_format {
            OutputFormat::Json => {
                println!("{}", serde_json::to_string_pretty(&balance)?);
            }
            _ => {
                println!("\n{}", "Balance Information".bold().green());
                println!("{}", "=".repeat(50));

                let mut table = Table::new();
                table.add_row(Row::new(vec![
                    Cell::new("Address").style_spec("bFg"),
                    Cell::new(&format!("{}...{}", &addr[..8], &addr[addr.len() - 8..])),
                ]));
                table.add_row(Row::new(vec![
                    Cell::new("Total Balance").style_spec("bFg"),
                    Cell::new(&format!("{:.8} NOVA", balance.balance)),
                ]));
                table.add_row(Row::new(vec![
                    Cell::new("Confirmed").style_spec("bFg"),
                    Cell::new(&format!("{:.8} NOVA", balance.confirmed)),
                ]));
                table.add_row(Row::new(vec![
                    Cell::new("Unconfirmed").style_spec("bFg"),
                    Cell::new(&format!("{:.8} NOVA", balance.unconfirmed)),
                ]));

                table.printstd();
            }
        },
        Err(e) => {
            print_error(&format!("Failed to get balance: {}", e));
        }
    }

    Ok(())
}

pub async fn new_address(config: &Config, wallet_name: Option<String>) -> Result<()> {
    let wallet_manager = WalletManager::new(Config::wallet_dir()?)?;

    let name = if let Some(n) = wallet_name {
        n
    } else {
        let wallets = wallet_manager.list_wallets()?;

        if wallets.is_empty() {
            print_error("No wallets found. Create one first.");
            return Ok(());
        }

        if wallets.len() == 1 {
            wallets[0].clone()
        } else {
            let index = Select::new()
                .with_prompt("Select wallet")
                .items(&wallets)
                .interact()?;
            wallets[index].clone()
        }
    };

    // Load wallet and generate address
    let mut wallet = wallet_manager.load_wallet(&name)?;
    let address = wallet_manager.generate_address(&mut wallet)?;

    match &config.output_format {
        OutputFormat::Json => {
            println!("{}", serde_json::to_string_pretty(&address)?);
        }
        _ => {
            print_success(&format!("Generated new address #{}", address.index));
            println!("\nAddress: {}", address.address.cyan().bold());

            if Confirm::new()
                .with_prompt("Show private key?")
                .default(false)
                .interact()?
            {
                println!("Private Key: {}", address.private_key.yellow());
                print_warning("Keep this private key secure!");
            }
        }
    }

    Ok(())
}

pub async fn export(config: &Config, wallet_name: String) -> Result<()> {
    let wallet_manager = WalletManager::new(Config::wallet_dir()?)?;

    // Load wallet
    let wallet = wallet_manager.load_wallet(&wallet_name)?;

    print_warning("This will export all private keys from your wallet.");
    print_warning("Anyone with these keys can spend your funds!");

    if !Confirm::new()
        .with_prompt("Are you sure you want to continue?")
        .default(false)
        .interact()?
    {
        println!("Export cancelled.");
        return Ok(());
    }

    let keys = wallet_manager.export_private_keys(&wallet);

    match &config.output_format {
        OutputFormat::Json => {
            let output: Vec<serde_json::Value> = keys
                .iter()
                .map(|(addr, key)| {
                    serde_json::json!({
                        "address": addr,
                        "private_key": key,
                    })
                })
                .collect();
            println!("{}", serde_json::to_string_pretty(&output)?);
        }
        _ => {
            println!("\n{}", "Exported Private Keys".bold().red());
            println!("{}", "=".repeat(70));

            for (address, private_key) in keys {
                println!("\nAddress: {}", address.cyan());
                println!("Private Key: {}", private_key.yellow());
            }

            println!("\n{}", "=".repeat(70));
            print_warning("Save these keys securely and delete this output!");
        }
    }

    Ok(())
}
