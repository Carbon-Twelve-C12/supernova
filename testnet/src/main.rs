// supernova Testnet Runner
// This binary provides a minimal environment for running the supernova testnet

use std::env;
use std::process::Command;
use std::path::Path;

// Main function that launches the testnet environment
fn main() {
    println!("supernova Testnet Runner");
    println!("-------------------------");

    // Parse command line arguments
    let args: Vec<String> = env::args().collect();
    if args.len() > 1 && args[1] == "--help" {
        print_help();
        return;
    }

    // Check for Docker
    if !check_docker() {
        println!("Docker is required but not installed or not running.");
        println!("Please install Docker and try again.");
        return;
    }

    // Launch testnet
    launch_testnet();
}

// Print help information
fn print_help() {
    println!("Usage: supernova-testnet [OPTIONS]");
    println!("");
    println!("Options:");
    println!("  --help     Show this help message");
    println!("  --nodes=N  Start N validator nodes (default: 3)");
    println!("");
    println!("Example:");
    println!("  supernova-testnet --nodes=5");
}

// Check if Docker is installed and running
fn check_docker() -> bool {
    let output = Command::new("docker")
        .arg("--version")
        .output();

    match output {
        Ok(_) => true,
        Err(_) => false
    }
}

// Launch testnet environment
fn launch_testnet() {
    println!("Launching supernova testnet...");

    // Check if Docker Compose file exists
    if !Path::new("docker-compose.yml").exists() {
        println!("Error: docker-compose.yml not found");
        println!("Make sure you're running from the project root directory");
        return;
    }

    // Start Docker Compose
    let output = Command::new("docker-compose")
        .arg("up")
        .arg("-d")
        .output();

    match output {
        Ok(out) => {
            if out.status.success() {
                println!("Testnet started successfully!");
                println!("Run 'docker-compose logs -f' to view node logs");
            } else {
                println!("Failed to start testnet");
                println!("Error: {}", String::from_utf8_lossy(&out.stderr));
            }
        },
        Err(e) => {
            println!("Failed to execute docker-compose: {}", e);
        }
    }
}