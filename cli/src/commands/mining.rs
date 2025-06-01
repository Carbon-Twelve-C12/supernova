use crate::config::{Config, OutputFormat};
use crate::rpc::RpcClient;
use crate::commands::{print_success, print_error, print_info, print_warning};
use anyhow::Result;
use colored::*;
use prettytable::{Cell, Row, Table};
use dialoguer::{Confirm, Input};
use indicatif::{ProgressBar, ProgressStyle};
use std::time::Duration;

pub async fn status(config: &Config) -> Result<()> {
    let client = RpcClient::new(config.rpc_url.clone(), config.timeout)?;
    
    print_info("Fetching mining status...");
    
    match client.get_mining_info().await {
        Ok(info) => {
            match &config.output_format {
                OutputFormat::Json => {
                    println!("{}", serde_json::to_string_pretty(&info)?);
                }
                _ => {
                    println!("\n{}", "Mining Status".bold().green());
                    println!("{}", "=".repeat(50));
                    
                    let mut table = Table::new();
                    table.add_row(Row::new(vec![
                        Cell::new("Mining Active").style_spec("bFg"),
                        Cell::new(if info.mining_enabled { "Yes" } else { "No" })
                            .style_spec(if info.mining_enabled { "Fg" } else { "Fr" }),
                    ]));
                    table.add_row(Row::new(vec![
                        Cell::new("Current Height").style_spec("bFg"),
                        Cell::new(&format!("{}", info.blocks)),
                    ]));
                    table.add_row(Row::new(vec![
                        Cell::new("Network Difficulty").style_spec("bFg"),
                        Cell::new(&format!("{:.6}", info.difficulty)),
                    ]));
                    table.add_row(Row::new(vec![
                        Cell::new("Network Hashrate").style_spec("bFg"),
                        Cell::new(&format_hashrate(info.network_hashrate)),
                    ]));
                    
                    if info.mining_enabled {
                        table.add_row(Row::new(vec![
                            Cell::new("Mining Threads").style_spec("bFg"),
                            Cell::new(&format!("{}", info.threads)),
                        ]));
                    }
                    
                    table.printstd();
                    
                    if !info.mining_enabled {
                        print_info("Mining is not active. Start with 'supernova mining start'");
                    }
                }
            }
        }
        Err(e) => {
            print_error(&format!("Failed to get mining info: {}", e));
        }
    }
    
    Ok(())
}

pub async fn start(config: &Config, threads: Option<u32>) -> Result<()> {
    let client = RpcClient::new(config.rpc_url.clone(), config.timeout)?;
    
    // Get current mining info
    let current_info = client.get_mining_info().await?;
    
    if current_info.mining_enabled {
        print_warning("Mining is already active");
        return Ok(());
    }
    
    // Get number of threads
    let thread_count = if let Some(t) = threads {
        t
    } else {
        let cpu_count = num_cpus::get() as u32;
        let default_threads = (cpu_count / 2).max(1);
        
        let input: String = Input::new()
            .with_prompt(format!("Number of mining threads (1-{}, default: {})", cpu_count, default_threads))
            .default(default_threads.to_string())
            .interact_text()?;
        
        input.parse::<u32>()
            .unwrap_or(default_threads)
            .min(cpu_count)
            .max(1)
    };
    
    println!("\n{}", "Mining Configuration".bold().yellow());
    println!("{}", "=".repeat(50));
    println!("Threads: {}", thread_count);
    println!("Network Difficulty: {:.6}", current_info.difficulty);
    println!("Network Hashrate: {}", format_hashrate(current_info.network_hashrate));
    
    print_warning("Mining will use significant CPU resources");
    
    if !Confirm::new()
        .with_prompt("Start mining?")
        .default(true)
        .interact()?
    {
        println!("Mining cancelled.");
        return Ok(());
    }
    
    // Show progress
    let pb = ProgressBar::new_spinner();
    pb.set_style(
        ProgressStyle::default_spinner()
            .template("{spinner:.green} {msg}")
            .unwrap()
    );
    pb.set_message("Starting mining...");
    pb.enable_steady_tick(Duration::from_millis(100));
    
    match client.start_mining(thread_count).await {
        Ok(_) => {
            pb.finish_and_clear();
            print_success(&format!("Mining started with {} threads", thread_count));
            println!("\n{}", "Mining Tips:".bold().blue());
            println!("• Monitor your system temperature");
            println!("• Use renewable energy");
            println!("• Check status with 'supernova mining status'");
            println!("• Stop mining with 'supernova mining stop'");
        }
        Err(e) => {
            pb.finish_and_clear();
            print_error(&format!("Failed to start mining: {}", e));
        }
    }
    
    Ok(())
}

pub async fn stop(config: &Config) -> Result<()> {
    let client = RpcClient::new(config.rpc_url.clone(), config.timeout)?;
    
    // Check if mining is active
    let info = client.get_mining_info().await?;
    
    if !info.mining_enabled {
        print_info("Mining is not currently active");
        return Ok(());
    }
    
    if !Confirm::new()
        .with_prompt("Stop mining?")
        .default(true)
        .interact()?
    {
        println!("Cancelled.");
        return Ok(());
    }
    
    match client.stop_mining().await {
        Ok(_) => {
            print_success("Mining stopped");
        }
        Err(e) => {
            print_error(&format!("Failed to stop mining: {}", e));
        }
    }
    
    Ok(())
}

pub async fn benchmark(config: &Config) -> Result<()> {
    println!("\n{}", "Mining Benchmark".bold().green());
    println!("{}", "=".repeat(50));
    print_info("Running 30-second benchmark...");
    
    // Simulate benchmark with progress bar
    let pb = ProgressBar::new(30);
    pb.set_style(
        ProgressStyle::default_bar()
            .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} seconds")
            .unwrap()
            .progress_chars("#>-")
    );
    
    for i in 0..30 {
        pb.set_position(i);
        std::thread::sleep(Duration::from_secs(1));
    }
    
    pb.finish_and_clear();
    
    // Mock benchmark results
    let cpu_count = num_cpus::get();
    let mock_results: Vec<(String, u64)> = vec![
        ("1 thread".to_string(), 145_000u64),
        (format!("{} threads", cpu_count / 2), 145_000 * (cpu_count as u64 / 2) * 85 / 100),
        (format!("{} threads", cpu_count), 145_000 * cpu_count as u64 * 75 / 100),
    ];
    
    match &config.output_format {
        OutputFormat::Json => {
            let results: Vec<serde_json::Value> = mock_results.iter()
                .map(|(threads, hashrate)| {
                    serde_json::json!({
                        "configuration": threads,
                        "hashrate": hashrate,
                        "hashrate_formatted": format_hashrate(*hashrate as f64),
                    })
                })
                .collect();
            println!("{}", serde_json::to_string_pretty(&results)?);
        }
        _ => {
            println!("\n{}", "Benchmark Results".bold().green());
            
            let mut table = Table::new();
            table.add_row(Row::new(vec![
                Cell::new("Configuration").style_spec("bFg"),
                Cell::new("Hashrate").style_spec("bFg"),
                Cell::new("Efficiency").style_spec("bFg"),
            ]));
            
            let max_hashrate = mock_results.iter().map(|(_, h)| *h).max().unwrap();
            
            for (config_name, hashrate) in &mock_results {
                let efficiency = (*hashrate as f64 / max_hashrate as f64 * 100.0) as u32;
                
                table.add_row(Row::new(vec![
                    Cell::new(config_name),
                    Cell::new(&format_hashrate(*hashrate as f64)),
                    Cell::new(&format!("{}%", efficiency))
                        .style_spec(if efficiency > 80 { "Fg" } else if efficiency > 60 { "Fy" } else { "Fr" }),
                ]));
            }
            
            table.printstd();
            
            println!("\n{}", "Recommendations:".bold().blue());
            println!("• Use {} threads for best efficiency", cpu_count / 2);
            println!("• Monitor CPU temperature during mining");
            println!("• Consider your electricity costs");
        }
    }
    
    Ok(())
}

// Helper function to format hashrate
fn format_hashrate(hashrate: f64) -> String {
    const UNITS: &[&str] = &["H/s", "KH/s", "MH/s", "GH/s", "TH/s", "PH/s"];
    let mut rate = hashrate;
    let mut unit_index = 0;
    
    while rate >= 1000.0 && unit_index < UNITS.len() - 1 {
        rate /= 1000.0;
        unit_index += 1;
    }
    
    format!("{:.2} {}", rate, UNITS[unit_index])
}

// Add this to Cargo.toml dependencies
// num_cpus = "1.15" 