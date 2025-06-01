use crate::config::{Config, OutputFormat};
use crate::rpc::RpcClient;
use crate::wallet::WalletManager;
use crate::commands::{print_success, print_error, print_info};
use anyhow::Result;
use colored::*;
use prettytable::{Cell, Row, Table};
use dialoguer::Confirm;
use indicatif::{ProgressBar, ProgressStyle};
use std::time::Duration;
use chrono::TimeZone;

pub async fn send(config: &Config, to: String, amount: f64) -> Result<()> {
    let client = RpcClient::new(config.rpc_url.clone(), config.timeout)?;
    
    // Validate recipient address
    if !client.validate_address(&to).await? {
        print_error("Invalid recipient address");
        return Ok(());
    }
    
    // Validate amount
    if amount <= 0.0 {
        print_error("Amount must be greater than 0");
        return Ok(());
    }
    
    println!("\n{}", "Transaction Details".bold().yellow());
    println!("{}", "=".repeat(50));
    println!("To: {}", to.cyan());
    println!("Amount: {} NOVA", format!("{:.8}", amount).yellow());
    
    // Calculate estimated fee (mock for now)
    let estimated_fee = 0.0001;
    println!("Estimated Fee: {} NOVA", format!("{:.8}", estimated_fee).yellow());
    println!("Total: {} NOVA", format!("{:.8}", amount + estimated_fee).yellow().bold());
    
    if !Confirm::new()
        .with_prompt("Confirm transaction?")
        .default(false)
        .interact()?
    {
        println!("Transaction cancelled.");
        return Ok(());
    }
    
    // Show progress
    let pb = ProgressBar::new_spinner();
    pb.set_style(
        ProgressStyle::default_spinner()
            .template("{spinner:.green} {msg}")
            .unwrap()
    );
    pb.set_message("Broadcasting transaction...");
    pb.enable_steady_tick(Duration::from_millis(100));
    
    // Send transaction
    match client.send_transaction(&to, amount).await {
        Ok(txid) => {
            pb.finish_and_clear();
            print_success("Transaction sent successfully!");
            
            match &config.output_format {
                OutputFormat::Json => {
                    println!("{}", serde_json::json!({
                        "txid": txid,
                        "to": to,
                        "amount": amount,
                        "fee": estimated_fee,
                    }));
                }
                _ => {
                    println!("\nTransaction ID: {}", txid.cyan().bold());
                    println!("Track at: https://explorer.testnet.supernovanetwork.xyz/tx/{}", txid);
                }
            }
        }
        Err(e) => {
            pb.finish_and_clear();
            print_error(&format!("Failed to send transaction: {}", e));
        }
    }
    
    Ok(())
}

pub async fn get(config: &Config, txid: String) -> Result<()> {
    let client = RpcClient::new(config.rpc_url.clone(), config.timeout)?;
    
    print_info(&format!("Fetching transaction {}...", &txid[..8]));
    
    match client.get_transaction(&txid).await {
        Ok(tx) => {
            match &config.output_format {
                OutputFormat::Json => {
                    println!("{}", serde_json::to_string_pretty(&tx)?);
                }
                _ => {
                    println!("\n{}", "Transaction Details".bold().green());
                    println!("{}", "=".repeat(70));
                    
                    let mut table = Table::new();
                    table.add_row(Row::new(vec![
                        Cell::new("Transaction ID").style_spec("bFg"),
                        Cell::new(&tx.txid),
                    ]));
                    table.add_row(Row::new(vec![
                        Cell::new("Size").style_spec("bFg"),
                        Cell::new(&format!("{} bytes", tx.size)),
                    ]));
                    table.add_row(Row::new(vec![
                        Cell::new("Version").style_spec("bFg"),
                        Cell::new(&format!("{}", tx.version)),
                    ]));
                    table.add_row(Row::new(vec![
                        Cell::new("Locktime").style_spec("bFg"),
                        Cell::new(&format!("{}", tx.locktime)),
                    ]));
                    
                    if tx.confirmations > 0 {
                        table.add_row(Row::new(vec![
                            Cell::new("Status").style_spec("bFg"),
                            Cell::new(&format!("Confirmed ({} confirmations)", tx.confirmations))
                                .style_spec("Fg"),
                        ]));
                        
                        if let Some(block_hash) = &tx.block_hash {
                            table.add_row(Row::new(vec![
                                Cell::new("Block").style_spec("bFg"),
                                Cell::new(&format!("{}...{}", &block_hash[..8], &block_hash[56..])),
                            ]));
                        }
                        
                        if let Some(time) = tx.time {
                            let dt = chrono::Local.timestamp_opt(time as i64, 0).unwrap();
                            table.add_row(Row::new(vec![
                                Cell::new("Time").style_spec("bFg"),
                                Cell::new(&dt.format("%Y-%m-%d %H:%M:%S").to_string()),
                            ]));
                        }
                    } else {
                        table.add_row(Row::new(vec![
                            Cell::new("Status").style_spec("bFg"),
                            Cell::new("Unconfirmed").style_spec("Fy"),
                        ]));
                    }
                    
                    table.printstd();
                }
            }
        }
        Err(e) => {
            print_error(&format!("Failed to get transaction: {}", e));
        }
    }
    
    Ok(())
}

pub async fn history(config: &Config, address: Option<String>) -> Result<()> {
    let _client = RpcClient::new(config.rpc_url.clone(), config.timeout)?;
    
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
        
        // For simplicity, use the first address of the first wallet
        let wallet = wallet_manager.load_wallet(&wallets[0])?;
        if wallet.addresses.is_empty() {
            print_error("Wallet has no addresses.");
            return Ok(());
        }
        
        wallet.addresses[0].address.clone()
    };
    
    print_info(&format!("Fetching transaction history for {}...", &addr[..8]));
    
    // Mock transaction history for now
    let mock_history = vec![
        serde_json::json!({
            "txid": "1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef",
            "type": "receive",
            "amount": 10.5,
            "confirmations": 6,
            "time": chrono::Local::now().timestamp() - 3600,
        }),
        serde_json::json!({
            "txid": "fedcba0987654321fedcba0987654321fedcba0987654321fedcba0987654321",
            "type": "send",
            "amount": 5.25,
            "confirmations": 3,
            "time": chrono::Local::now().timestamp() - 7200,
        }),
    ];
    
    match &config.output_format {
        OutputFormat::Json => {
            println!("{}", serde_json::to_string_pretty(&mock_history)?);
        }
        _ => {
            println!("\n{}", format!("Transaction History for {}...{}", &addr[..8], &addr[addr.len()-8..]).bold().green());
            println!("{}", "=".repeat(80));
            
            if mock_history.is_empty() {
                print_info("No transactions found");
            } else {
                let mut table = Table::new();
                table.add_row(Row::new(vec![
                    Cell::new("Time").style_spec("bFg"),
                    Cell::new("Type").style_spec("bFg"),
                    Cell::new("Amount").style_spec("bFg"),
                    Cell::new("Confirmations").style_spec("bFg"),
                    Cell::new("Transaction ID").style_spec("bFg"),
                ]));
                
                for tx in mock_history {
                    let tx_obj = tx.as_object().unwrap();
                    let time = chrono::Local.timestamp_opt(tx_obj["time"].as_i64().unwrap(), 0).unwrap();
                    let tx_type = tx_obj["type"].as_str().unwrap();
                    let amount = tx_obj["amount"].as_f64().unwrap();
                    let confirmations = tx_obj["confirmations"].as_u64().unwrap();
                    let txid = tx_obj["txid"].as_str().unwrap();
                    
                    let type_cell = if tx_type == "receive" {
                        Cell::new("↓ Receive").style_spec("Fg")
                    } else {
                        Cell::new("↑ Send").style_spec("Fr")
                    };
                    
                    let amount_cell = if tx_type == "receive" {
                        Cell::new(&format!("+{:.8}", amount)).style_spec("Fg")
                    } else {
                        Cell::new(&format!("-{:.8}", amount)).style_spec("Fr")
                    };
                    
                    table.add_row(Row::new(vec![
                        Cell::new(&time.format("%Y-%m-%d %H:%M").to_string()),
                        type_cell,
                        amount_cell,
                        Cell::new(&format!("{}", confirmations)),
                        Cell::new(&format!("{}...{}", &txid[..8], &txid[56..])),
                    ]));
                }
                
                table.printstd();
            }
        }
    }
    
    Ok(())
} 