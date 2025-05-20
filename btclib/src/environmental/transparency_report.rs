use std::fmt;
use std::collections::HashMap;
use chrono::{DateTime, Utc};
use crate::environmental::transparency::{TransparencyReport, TransparencyLevel};
use crate::environmental::miner_reporting::MinerVerificationStatus;

/// Generate a text report from a transparency report
pub fn generate_text_report(report: &TransparencyReport) -> String {
    let mut text = String::new();
    
    // Basic information
    text.push_str(&format!(
        "Transparency Report ({}):\n", 
        report.timestamp.format("%Y-%m-%d %H:%M:%S")
    ));
    text.push_str(&format!("Transparency Level: {}\n", report.transparency_level));
    text.push_str(&format!("Total Renewable Energy: {:.2} MWh\n", report.total_renewable_energy_mwh));
    text.push_str(&format!("Total Carbon Offset: {:.2} tonnes CO2e\n", report.total_offset_tonnes));
    text.push_str(&format!("Renewable Verification: {:.1}%\n", report.renewable_verification_percentage));
    text.push_str(&format!("Offset Verification: {:.1}%\n", report.offset_verification_percentage));
    
    if let Some(ratio) = report.carbon_negative_ratio {
        text.push_str(&format!("Carbon Negative Ratio: {:.2}x\n", ratio));
    }
    
    if let Some(impact) = report.net_carbon_impact {
        text.push_str(&format!("Net Carbon Impact: {:.2} tonnes CO2e\n", impact));
    }
    
    // REC certificate details
    if let Some(rec_stats) = &report.rec_stats {
        text.push_str("\nREC Certificate Details:\n");
        text.push_str(&format!("Total Certificates: {}\n", rec_stats.certificate_count));
        text.push_str(&format!("Total MWh: {:.2}\n", rec_stats.total_mwh));
        
        text.push_str("Energy Type Breakdown:\n");
        for (energy_type, amount) in &rec_stats.energy_type_breakdown {
            text.push_str(&format!("  {}: {:.2} MWh\n", energy_type, amount));
        }
        
        text.push_str("Verification Status:\n");
        for (status, count) in &rec_stats.verification_status_breakdown {
            text.push_str(&format!("  {}: {}\n", status, count));
        }
    }
    
    // Carbon offset details
    if let Some(offset_stats) = &report.offset_stats {
        text.push_str("\nCarbon Offset Details:\n");
        text.push_str(&format!("Total Offsets: {}\n", offset_stats.offset_count));
        text.push_str(&format!("Total Tonnes: {:.2}\n", offset_stats.total_tonnes));
        
        text.push_str("Project Type Breakdown:\n");
        for (project_type, amount) in &offset_stats.project_type_breakdown {
            text.push_str(&format!("  {}: {:.2} tonnes\n", project_type, amount));
        }
        
        text.push_str("Verification Status:\n");
        for (status, count) in &offset_stats.verification_status_breakdown {
            text.push_str(&format!("  {}: {}\n", status, count));
        }
    }
    
    text
} 