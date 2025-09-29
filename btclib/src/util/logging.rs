//! Logging utilities for the Supernova blockchain

use tracing::{info, warn, error, debug, Level};
use tracing_subscriber::EnvFilter;
use std::fs::OpenOptions;
use std::path::PathBuf;
use chrono::Local;

/// Initialize logging with optional file output
pub fn init_logging(log_level: Option<Level>, log_file: Option<PathBuf>) -> Result<(), String> {
    let env_filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| {
            let level = log_level.unwrap_or(Level::INFO);
            EnvFilter::new(format!("btclib={},supernova={}", level, level))
        });
    
    match log_file {
        Some(path) => {
            // Create the file or open for append if it exists
            let file = OpenOptions::new()
                .create(true)
                .append(true)
                .open(&path)
                .map_err(|e| format!("Failed to open log file: {}", e))?;
            
            // Set up logging to both stdout and the file
            let subscriber = tracing_subscriber::fmt()
                .with_env_filter(env_filter)
                .with_writer(std::io::stdout)
                .with_ansi(true)
                .finish();
            
            tracing::subscriber::set_global_default(subscriber)
                .map_err(|e| format!("Failed to set global default subscriber: {}", e))?;
            
            info!("Logging initialized with output to {}", path.display());
            Ok(())
        },
        None => {
            // Set up logging to stdout only
            let subscriber = tracing_subscriber::fmt()
                .with_env_filter(env_filter)
                .with_ansi(true)
                .finish();
            
            tracing::subscriber::set_global_default(subscriber)
                .map_err(|e| format!("Failed to set global default subscriber: {}", e))?;
            
            info!("Logging initialized to stdout");
            Ok(())
        }
    }
}

/// Format a log message with timestamp
pub fn format_log_message(message: &str) -> String {
    let now = Local::now();
    format!("[{}] {}", now.format("%Y-%m-%d %H:%M:%S%.3f"), message)
}

/// Structured logging for blockchain events
pub struct BlockchainLogger;

impl BlockchainLogger {
    pub fn block_added(height: u64, hash: &str, txs: usize, size: usize) {
        info!(
            height = height,
            hash = hash,
            txs = txs,
            size_bytes = size,
            "Block added to chain"
        );
    }
    
    pub fn tx_received(hash: &str, size: usize, fee: u64) {
        debug!(
            hash = hash,
            size_bytes = size,
            fee_sats = fee,
            "Transaction received"
        );
    }
    
    pub fn peer_connected(peer_id: &str, addr: &str) {
        info!(
            peer_id = peer_id,
            addr = addr,
            "Peer connected"
        );
    }
    
    pub fn peer_disconnected(peer_id: &str, reason: &str) {
        info!(
            peer_id = peer_id,
            reason = reason,
            "Peer disconnected"
        );
    }
    
    pub fn warning(component: &str, message: &str) {
        warn!(
            component = component,
            "Warning: {}", message
        );
    }
    
    pub fn error(component: &str, message: &str) {
        error!(
            component = component,
            "Error: {}", message
        );
    }
} 