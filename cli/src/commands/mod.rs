pub mod blockchain;
pub mod wallet;
pub mod transaction;
pub mod mining;
pub mod config;
pub mod swap;

use crate::config::OutputFormat;
use anyhow::Result;
use colored::*;
use serde::Serialize;

/// Format output based on user preference
pub fn format_output<T: Serialize>(data: T, format: &OutputFormat, title: Option<&str>) -> Result<()> {
    match format {
        OutputFormat::Json => {
            println!("{}", serde_json::to_string_pretty(&data)?);
        }
        OutputFormat::Text => {
            if let Some(title) = title {
                println!("{}", title.bold().green());
                println!("{}", "=".repeat(title.len()));
            }
            println!("{}", serde_json::to_string_pretty(&data)?);
        }
        OutputFormat::Table => {
            // Table formatting is handled by individual commands
            // This is a fallback to JSON
            println!("{}", serde_json::to_string_pretty(&data)?);
        }
    }
    Ok(())
}

/// Print success message
pub fn print_success(message: &str) {
    println!("{} {}", "✓".green().bold(), message.green());
}

/// Print error message
pub fn print_error(message: &str) {
    eprintln!("{} {}", "✗".red().bold(), message.red());
}

/// Print warning message
pub fn print_warning(message: &str) {
    println!("{} {}", "⚠".yellow().bold(), message.yellow());
}

/// Print info message
pub fn print_info(message: &str) {
    println!("{} {}", "ℹ".blue().bold(), message.blue());
} 