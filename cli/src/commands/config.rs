use crate::config::{Config, OutputFormat};
use crate::commands::{print_success, print_error, print_info};
use anyhow::Result;
use colored::*;
use prettytable::{Cell, Row, Table};
use dialoguer::{Input, Select};

pub async fn show(config: &Config) -> Result<()> {
    match &config.output_format {
        OutputFormat::Json => {
            println!("{}", serde_json::to_string_pretty(&config)?);
        }
        _ => {
            println!("\n{}", "Current Configuration".bold().green());
            println!("{}", "=".repeat(50));
            
            let mut table = Table::new();
            table.add_row(Row::new(vec![
                Cell::new("Setting").style_spec("bFg"),
                Cell::new("Value").style_spec("bFg"),
            ]));
            
            table.add_row(Row::new(vec![
                Cell::new("RPC URL"),
                Cell::new(&config.rpc_url).style_spec("Fy"),
            ]));
            table.add_row(Row::new(vec![
                Cell::new("Network"),
                Cell::new(&config.network),
            ]));
            table.add_row(Row::new(vec![
                Cell::new("Timeout"),
                Cell::new(&format!("{} seconds", config.timeout)),
            ]));
            table.add_row(Row::new(vec![
                Cell::new("Debug Mode"),
                Cell::new(if config.debug { "Enabled" } else { "Disabled" })
                    .style_spec(if config.debug { "Fy" } else { "Fr" }),
            ]));
            table.add_row(Row::new(vec![
                Cell::new("Output Format"),
                Cell::new(&format!("{:?}", config.output_format)),
            ]));
            
            if let Some(wallet_path) = &config.wallet_path {
                table.add_row(Row::new(vec![
                    Cell::new("Wallet Path"),
                    Cell::new(&wallet_path.display().to_string()),
                ]));
            }
            
            table.printstd();
            
            println!("\nConfig file: {}", Config::config_path()?.display().to_string().cyan());
        }
    }
    
    Ok(())
}

pub async fn set(key: String, value: String) -> Result<()> {
    let mut config = Config::load()?;
    
    match key.to_lowercase().as_str() {
        "rpc" | "rpc_url" | "url" => {
            config.rpc_url = value;
            print_success(&format!("Set RPC URL to: {}", config.rpc_url));
        }
        "network" => {
            if !["mainnet", "testnet", "devnet"].contains(&value.as_str()) {
                print_error("Invalid network. Must be: mainnet, testnet, or devnet");
                return Ok(());
            }
            config.network = value;
            print_success(&format!("Set network to: {}", config.network));
        }
        "timeout" => {
            match value.parse::<u64>() {
                Ok(timeout) => {
                    config.timeout = timeout;
                    print_success(&format!("Set timeout to: {} seconds", config.timeout));
                }
                Err(_) => {
                    print_error("Invalid timeout value. Must be a number.");
                    return Ok(());
                }
            }
        }
        "debug" => {
            match value.to_lowercase().as_str() {
                "true" | "on" | "1" | "yes" => {
                    config.debug = true;
                    print_success("Debug mode enabled");
                }
                "false" | "off" | "0" | "no" => {
                    config.debug = false;
                    print_success("Debug mode disabled");
                }
                _ => {
                    print_error("Invalid debug value. Use: true/false, on/off, yes/no, 1/0");
                    return Ok(());
                }
            }
        }
        "format" | "output" | "output_format" => {
            match value.to_lowercase().as_str() {
                "json" => {
                    config.output_format = OutputFormat::Json;
                    print_success("Output format set to: JSON");
                }
                "table" => {
                    config.output_format = OutputFormat::Table;
                    print_success("Output format set to: Table");
                }
                "text" => {
                    config.output_format = OutputFormat::Text;
                    print_success("Output format set to: Text");
                }
                _ => {
                    print_error("Invalid output format. Must be: json, table, or text");
                    return Ok(());
                }
            }
        }
        _ => {
            print_error(&format!("Unknown configuration key: {}", key));
            print_info("Valid keys: rpc_url, network, timeout, debug, output_format");
            return Ok(());
        }
    }
    
    config.save()?;
    Ok(())
}

pub async fn reset() -> Result<()> {
    print_warning("This will reset all configuration to default values.");
    
    if !dialoguer::Confirm::new()
        .with_prompt("Are you sure?")
        .default(false)
        .interact()?
    {
        println!("Reset cancelled.");
        return Ok(());
    }
    
    let config = Config::default();
    config.save()?;
    
    print_success("Configuration reset to defaults");
    show(&config).await?;
    
    Ok(())
}

pub async fn interactive() -> Result<()> {
    println!("\n{}", "Interactive Configuration".bold().green());
    println!("{}", "=".repeat(50));
    
    let mut config = Config::load()?;
    
    // RPC URL
    config.rpc_url = Input::new()
        .with_prompt("RPC URL")
        .default(config.rpc_url.clone())
        .interact_text()?;
    
    // Network
    let networks = vec!["testnet", "mainnet", "devnet"];
    let current_index = networks.iter().position(|&n| n == config.network).unwrap_or(0);
    let network_index = Select::new()
        .with_prompt("Network")
        .items(&networks)
        .default(current_index)
        .interact()?;
    config.network = networks[network_index].to_string();
    
    // Timeout
    let timeout_str = Input::<String>::new()
        .with_prompt("Timeout (seconds)")
        .default(config.timeout.to_string())
        .interact_text()?;
    
    if let Ok(timeout) = timeout_str.parse::<u64>() {
        config.timeout = timeout;
    }
    
    // Debug mode
    config.debug = dialoguer::Confirm::new()
        .with_prompt("Enable debug mode?")
        .default(config.debug)
        .interact()?;
    
    // Output format
    let formats = vec!["table", "json", "text"];
    let current_format = match config.output_format {
        OutputFormat::Table => 0,
        OutputFormat::Json => 1,
        OutputFormat::Text => 2,
    };
    let format_index = Select::new()
        .with_prompt("Output format")
        .items(&formats)
        .default(current_format)
        .interact()?;
    
    config.output_format = match format_index {
        0 => OutputFormat::Table,
        1 => OutputFormat::Json,
        2 => OutputFormat::Text,
        _ => OutputFormat::Table,
    };
    
    config.save()?;
    print_success("Configuration saved");
    
    show(&config).await?;
    
    Ok(())
}

use crate::commands::print_warning; 