use crate::config::{Config, OutputFormat};
use crate::rpc::RpcClient;
use crate::commands::{print_success, print_error, print_info, print_warning};
use anyhow::Result;
use colored::*;
use prettytable::{Cell, Row, Table};

pub async fn status(config: &Config) -> Result<()> {
    let client = RpcClient::new(config.rpc_url.clone(), config.timeout)?;
    
    print_info("Fetching blockchain status...");
    
    // Try to ping first
    if let Err(e) = client.ping().await {
        print_error(&format!("Cannot connect to node at {}: {}", config.rpc_url, e));
        return Err(e);
    }
    
    let info = client.get_blockchain_info().await?;
    let node_info = client.get_node_info().await?;
    let peer_info = client.get_peer_info().await?;
    
    match &config.output_format {
        OutputFormat::Json => {
            let output = serde_json::json!({
                "blockchain": info,
                "node": node_info,
                "peers": peer_info.len(),
            });
            println!("{}", serde_json::to_string_pretty(&output)?);
        }
        OutputFormat::Table | OutputFormat::Text => {
            println!("\n{}", "Supernova Blockchain Status".bold().green());
            println!("{}", "=".repeat(50));
            
            let mut table = Table::new();
            table.add_row(Row::new(vec![
                Cell::new("Network").style_spec("bFg"),
                Cell::new(&info.chain),
            ]));
            table.add_row(Row::new(vec![
                Cell::new("Block Height").style_spec("bFg"),
                Cell::new(&format!("{}", info.blocks)),
            ]));
            table.add_row(Row::new(vec![
                Cell::new("Difficulty").style_spec("bFg"),
                Cell::new(&format!("{:.2}", info.difficulty)),
            ]));
            table.add_row(Row::new(vec![
                Cell::new("Best Block").style_spec("bFg"),
                Cell::new(&format!("{}...{}", &info.best_block_hash[..8], &info.best_block_hash[56..])),
            ]));
            table.add_row(Row::new(vec![
                Cell::new("Sync Progress").style_spec("bFg"),
                Cell::new(&format!("{:.1}%", info.verification_progress * 100.0)),
            ]));
            table.add_row(Row::new(vec![
                Cell::new("Node Version").style_spec("bFg"),
                Cell::new(&node_info.version),
            ]));
            table.add_row(Row::new(vec![
                Cell::new("Connections").style_spec("bFg"),
                Cell::new(&format!("{} peers", peer_info.len())),
            ]));
            table.add_row(Row::new(vec![
                Cell::new("Uptime").style_spec("bFg"),
                Cell::new(&format_duration(node_info.uptime)),
            ]));
            
            table.printstd();
        }
    }
    
    Ok(())
}

pub async fn peers(config: &Config) -> Result<()> {
    let client = RpcClient::new(config.rpc_url.clone(), config.timeout)?;
    let peers = client.get_peer_info().await?;
    
    match &config.output_format {
        OutputFormat::Json => {
            println!("{}", serde_json::to_string_pretty(&peers)?);
        }
        OutputFormat::Table | OutputFormat::Text => {
            println!("\n{}", format!("Connected Peers ({})", peers.len()).bold().green());
            println!("{}", "=".repeat(80));
            
            if peers.is_empty() {
                print_info("No peers connected");
            } else {
                let mut table = Table::new();
                table.add_row(Row::new(vec![
                    Cell::new("ID").style_spec("bFg"),
                    Cell::new("Address").style_spec("bFg"),
                    Cell::new("Version").style_spec("bFg"),
                    Cell::new("Connected").style_spec("bFg"),
                    Cell::new("Last Activity").style_spec("bFg"),
                ]));
                
                for peer in peers {
                    table.add_row(Row::new(vec![
                        Cell::new(&peer.id[..8]),
                        Cell::new(&peer.addr),
                        Cell::new(&peer.version),
                        Cell::new(&format_duration(peer.connection_time)),
                        Cell::new(&format!("{}s ago", 
                            std::time::SystemTime::now()
                                .duration_since(std::time::UNIX_EPOCH)
                                .unwrap()
                                .as_secs() - peer.last_recv
                        )),
                    ]));
                }
                
                table.printstd();
            }
        }
    }
    
    Ok(())
}

pub async fn mempool(config: &Config) -> Result<()> {
    let client = RpcClient::new(config.rpc_url.clone(), config.timeout)?;
    let mempool = client.get_mempool_info().await?;
    
    match &config.output_format {
        OutputFormat::Json => {
            println!("{}", serde_json::to_string_pretty(&mempool)?);
        }
        OutputFormat::Table | OutputFormat::Text => {
            println!("\n{}", "Mempool Information".bold().green());
            println!("{}", "=".repeat(40));
            
            let mut table = Table::new();
            table.add_row(Row::new(vec![
                Cell::new("Transactions").style_spec("bFg"),
                Cell::new(&format!("{}", mempool.size)),
            ]));
            table.add_row(Row::new(vec![
                Cell::new("Size").style_spec("bFg"),
                Cell::new(&format_bytes(mempool.bytes)),
            ]));
            table.add_row(Row::new(vec![
                Cell::new("Usage").style_spec("bFg"),
                Cell::new(&format!("{} / {}", format_bytes(mempool.usage), format_bytes(mempool.max_mempool))),
            ]));
            table.add_row(Row::new(vec![
                Cell::new("Min Fee Rate").style_spec("bFg"),
                Cell::new(&format!("{:.8} NOVA/kB", mempool.mempool_min_fee)),
            ]));
            
            table.printstd();
        }
    }
    
    Ok(())
}

pub async fn environmental(config: &Config) -> Result<()> {
    let client = RpcClient::new(config.rpc_url.clone(), config.timeout)?;
    let metrics = client.get_environmental_metrics().await?;
    
    match &config.output_format {
        OutputFormat::Json => {
            println!("{}", serde_json::to_string_pretty(&metrics)?);
        }
        OutputFormat::Table | OutputFormat::Text => {
            println!("\n{}", "Environmental Metrics".bold().green());
            println!("{}", "=".repeat(50));
            
            let mut table = Table::new();
            table.add_row(Row::new(vec![
                Cell::new("Carbon Footprint").style_spec("bFg"),
                Cell::new(&format!("{:.2} kg CO₂/hour", metrics.carbon_footprint)),
            ]));
            table.add_row(Row::new(vec![
                Cell::new("Renewable Energy").style_spec("bFg"),
                Cell::new(&format!("{:.1}%", metrics.renewable_percentage)),
            ]));
            table.add_row(Row::new(vec![
                Cell::new("Green Miners").style_spec("bFg"),
                Cell::new(&format!("{} nodes", metrics.green_miners)),
            ]));
            table.add_row(Row::new(vec![
                Cell::new("Carbon Credits Earned").style_spec("bFg"),
                Cell::new(&format!("{:.4} NOVA", metrics.carbon_credits_earned)),
            ]));
            
            table.printstd();
            
            // Show environmental status
            if metrics.renewable_percentage > 75.0 {
                print_success(&format!("Network is {}% powered by renewable energy.", metrics.renewable_percentage));
            } else if metrics.renewable_percentage > 50.0 {
                print_info(&format!("Network is {}% renewable. Room for improvement.", metrics.renewable_percentage));
            } else {
                print_warning(&format!("Only {}% renewable energy. Consider green mining.", metrics.renewable_percentage));
            }
        }
    }
    
    Ok(())
}

// Helper functions
fn format_duration(seconds: u64) -> String {
    let days = seconds / 86400;
    let hours = (seconds % 86400) / 3600;
    let minutes = (seconds % 3600) / 60;
    
    if days > 0 {
        format!("{}d {}h", days, hours)
    } else if hours > 0 {
        format!("{}h {}m", hours, minutes)
    } else {
        format!("{}m", minutes)
    }
}

fn format_bytes(bytes: u64) -> String {
    const UNITS: &[&str] = &["B", "KB", "MB", "GB", "TB"];
    let mut size = bytes as f64;
    let mut unit_index = 0;
    
    while size >= 1024.0 && unit_index < UNITS.len() - 1 {
        size /= 1024.0;
        unit_index += 1;
    }
    
    format!("{:.2} {}", size, UNITS[unit_index])
} 