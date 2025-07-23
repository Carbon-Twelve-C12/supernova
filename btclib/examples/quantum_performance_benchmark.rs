//! Example: Quantum Signature Performance Benchmarking
//! 
//! This example demonstrates how to benchmark and monitor the performance
//! of Supernova's post-quantum signature schemes, inspired by the
//! LaBRADOR signature aggregation research.

use btclib::monitoring::quantum_signature_benchmarks::{
    QuantumSignatureMonitor, BenchmarkResults
};
use btclib::crypto::quantum::{QuantumScheme, QuantumParameters};
use prometheus::Registry;
use std::collections::HashMap;

fn main() {
    println!("Starting Supernova Quantum Signature Performance Benchmark...\n");
    
    // Create Prometheus registry for metrics
    let registry = Registry::new();
    let mut monitor = QuantumSignatureMonitor::new(&registry);
    
    // Run comprehensive benchmarks
    println!("Running full benchmark suite (this may take several minutes)...\n");
    let results = monitor.run_full_benchmark_suite();
    
    // Generate performance report
    monitor.generate_performance_report(&results);
    
    // Analyze specific scenarios
    analyze_production_scenarios(&mut monitor);
    
    // Compare with research findings
    compare_with_labrador_research(&results);
    
    // Export metrics for monitoring
    export_prometheus_metrics(&registry);
}

fn analyze_production_scenarios(monitor: &mut QuantumSignatureMonitor) {
    println!("\n\n=== Production Scenario Analysis ===\n");
    
    // Scenario 1: Block validation (100 transactions)
    println!("Scenario 1: Block Validation (100 transactions)");
    let params = QuantumParameters {
        scheme: QuantumScheme::Dilithium,
        security_level: 3,
    };
    let block_result = monitor.benchmark_batch_verification(params, 100);
    println!("  - Total verification time: {:.2}ms", block_result.verification_time_ms);
    println!("  - Per-transaction time: {:.4}ms", block_result.verification_time_ms / 100.0);
    println!("  - Throughput: {:.0} tx/sec", 1000.0 / block_result.verification_time_ms * 100.0);
    
    // Scenario 2: Lightning Network channel opening (2 signatures)
    println!("\nScenario 2: Lightning Network Channel Opening");
    let ln_result = monitor.benchmark_batch_verification(params, 2);
    println!("  - Channel open time: {:.2}ms", ln_result.verification_time_ms);
    
    // Scenario 3: Large batch processing (1000 signatures)
    println!("\nScenario 3: Large Batch Processing (1000 signatures)");
    let batch_result = monitor.benchmark_batch_verification(params, 1000);
    println!("  - Batch verification time: {:.2}ms", batch_result.verification_time_ms);
    println!("  - Aggregated proof size: {:.2}KB", batch_result.proof_size_kb);
}

fn compare_with_labrador_research(results: &HashMap<String, Vec<BenchmarkResults>>) {
    println!("\n\n=== Detailed Comparison with LaBRADOR Research ===\n");
    
    // Find results for 10k signatures
    if let Some(dilithium_results) = results.get("Dilithium") {
        if let Some(result_10k) = dilithium_results.iter().find(|r| r.signatures_count == 10000) {
            println!("Supernova Dilithium (10k signatures):");
            println!("  - Proving time: {:.2}s", result_10k.proving_time_ms / 1000.0);
            println!("  - Verification time: {:.2}s", result_10k.verification_time_ms / 1000.0);
            println!("  - Proof size: {:.2}KB", result_10k.proof_size_kb);
            
            println!("\nLaBRADOR Falcon (10k signatures):");
            println!("  - Proving time: 5.95s");
            println!("  - Verification time: 2.65s");
            println!("  - Proof size: 74.07KB");
            
            // Analysis
            let proving_ratio = result_10k.proving_time_ms / 1000.0 / 5.95;
            let verify_ratio = result_10k.verification_time_ms / 1000.0 / 2.65;
            
            println!("\nPerformance Analysis:");
            if proving_ratio < 1.0 {
                println!("  ✅ Supernova proving is {:.1}x faster", 1.0 / proving_ratio);
            } else {
                println!("  ⚠️  Supernova proving is {:.1}x slower", proving_ratio);
            }
            
            if verify_ratio < 1.0 {
                println!("  ✅ Supernova verification is {:.1}x faster", 1.0 / verify_ratio);
            } else {
                println!("  ⚠️  Supernova verification is {:.1}x slower", verify_ratio);
            }
            
            println!("\nKey Differences:");
            println!("  - Supernova uses NIST-standardized algorithms");
            println!("  - Environmental tracking adds ~5% overhead");
            println!("  - Hybrid mode support for gradual migration");
        }
    }
}

fn export_prometheus_metrics(registry: &Registry) {
    println!("\n\n=== Exporting Prometheus Metrics ===\n");
    
    // In production, these would be exposed via HTTP endpoint
    let metric_families = registry.gather();
    
    println!("Available metrics for monitoring:");
    for family in &metric_families {
        println!("  - {}: {}", family.get_name(), family.get_help());
    }
    
    println!("\nMetrics can be visualized in Grafana dashboards");
    println!("Example queries:");
    println!("  - rate(quantum_sign_duration_seconds_sum[5m]) / rate(quantum_sign_duration_seconds_count[5m])");
    println!("  - histogram_quantile(0.95, quantum_verify_duration_seconds_bucket)");
    println!("  - quantum_signature_size_bytes_avg");
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_benchmark_execution() {
        let registry = Registry::new();
        let mut monitor = QuantumSignatureMonitor::new(&registry);
        
        // Quick benchmark with small batch
        let params = QuantumParameters {
            scheme: QuantumScheme::Dilithium,
            security_level: 2,
        };
        
        let result = monitor.benchmark_batch_verification(params, 10);
        assert_eq!(result.signatures_count, 10);
        assert!(result.verification_time_ms > 0.0);
        assert!(result.proof_size_kb > 0.0);
    }
} 