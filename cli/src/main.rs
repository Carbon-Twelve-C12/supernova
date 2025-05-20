// SuperNova CLI Client
// This binary provides a command-line interface for interacting with the SuperNova blockchain

use std::env;
use std::process::exit;
use std::io::{self, Write};

fn main() {
    println!("SuperNova CLI Client");
    println!("-------------------");
    
    // Parse command line arguments
    let args: Vec<String> = env::args().collect();
    if args.len() > 1 {
        handle_command(&args[1], &args[2..]);
    } else {
        interactive_mode();
    }
}

// Handle a command from command line arguments
fn handle_command(command: &str, args: &[String]) {
    match command {
        "help" => show_help(),
        "version" => show_version(),
        "status" => show_status(),
        "balance" => show_balance(args),
        "send" => send_transaction(args),
        "mine" => start_mining(args),
        _ => {
            println!("Unknown command: {}", command);
            show_help();
        }
    }
}

// Show help information
fn show_help() {
    println!("Usage: supernova [COMMAND] [OPTIONS]");
    println!("");
    println!("Commands:");
    println!("  help                  Show this help message");
    println!("  version               Show version information");
    println!("  status                Show network status");
    println!("  balance [ADDRESS]     Show balance for an address");
    println!("  send [TO] [AMOUNT]    Send NOVA to an address");
    println!("  mine [THREADS]        Start mining");
    println!("");
    println!("Examples:");
    println!("  supernova balance 0x123456789abcdef");
    println!("  supernova send 0x123456789abcdef 10.5");
}

// Show version information
fn show_version() {
    println!("SuperNova CLI v0.1.0");
    println!("Testnet Edition");
}

// Show network status
fn show_status() {
    println!("Network: Testnet");
    println!("Status: Running");
    println!("Nodes: 3 active");
    println!("Block height: 1000");
    println!("Difficulty: 12345");
}

// Show balance for an address
fn show_balance(args: &[String]) {
    if args.is_empty() {
        println!("Error: No address specified");
        return;
    }
    
    let address = &args[0];
    println!("Address: {}", address);
    println!("Balance: 100.0 NOVA");
}

// Send a transaction
fn send_transaction(args: &[String]) {
    if args.len() < 2 {
        println!("Error: Missing arguments");
        println!("Usage: supernova send [TO] [AMOUNT]");
        return;
    }
    
    let to = &args[0];
    let amount = &args[1];
    println!("Sending {} NOVA to {}", amount, to);
    println!("Transaction submitted: 0x9876543210abcdef");
}

// Start mining
fn start_mining(args: &[String]) {
    let threads = if args.is_empty() { 
        1 
    } else { 
        args[0].parse::<u32>().unwrap_or(1) 
    };
    
    println!("Starting mining with {} threads", threads);
    println!("Press Ctrl+C to stop mining");
    
    // Simulate mining
    println!("Mining...");
    std::thread::sleep(std::time::Duration::from_secs(3));
    println!("Block found! Hash: 0xabcdef1234567890");
}

// Interactive command-line mode
fn interactive_mode() {
    println!("Interactive mode (type 'exit' to quit, 'help' for commands)");
    
    loop {
        print!("> ");
        io::stdout().flush().unwrap();
        
        let mut input = String::new();
        io::stdin().read_line(&mut input).unwrap();
        
        let input = input.trim();
        if input.is_empty() {
            continue;
        }
        
        if input == "exit" || input == "quit" {
            break;
        }
        
        let parts: Vec<&str> = input.split_whitespace().collect();
        let command = parts[0];
        let args: Vec<String> = parts[1..].iter().map(|s| s.to_string()).collect();
        
        handle_command(command, &args);
    }
} 