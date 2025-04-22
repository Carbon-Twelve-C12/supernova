use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::Mutex;
use tokio::task::JoinHandle;
use tokio::time;
use tracing::{info, warn, error, debug};

use crate::metrics::registry::MetricsRegistry;
use sysinfo::{System, SystemExt, ProcessExt, DiskExt, NetworkExt, CpuExt};
use crate::metrics::system::SystemMetrics;

/// Configuration options for the metrics collector
#[derive(Debug, Clone)]
pub struct MetricsCollectorConfig {
    /// Interval for collecting basic system metrics (CPU, Memory)
    pub basic_collection_interval: Duration,
    /// Interval for collecting extended metrics (Disk, Network)
    pub extended_collection_interval: Duration,
    /// Types of metrics to collect
    pub metrics_types: MetricsTypes,
}

/// Types of metrics that can be collected
#[derive(Debug, Clone, Copy)]
pub struct MetricsTypes {
    /// Collect CPU metrics
    pub cpu: bool,
    /// Collect memory metrics
    pub memory: bool,
    /// Collect disk metrics
    pub disk: bool,
    /// Collect network metrics
    pub network: bool,
    /// Collect process-specific metrics
    pub process: bool,
}

impl Default for MetricsCollectorConfig {
    fn default() -> Self {
        Self {
            basic_collection_interval: Duration::from_secs(1),
            extended_collection_interval: Duration::from_secs(10),
            metrics_types: MetricsTypes {
                cpu: true,
                memory: true,
                disk: true,
                network: true,
                process: true,
            },
        }
    }
}

/// Service that collects system metrics
pub struct MetricsCollector {
    /// Configuration for the collector
    config: MetricsCollectorConfig,
    /// Tracks if the collector is running
    running: Arc<Mutex<bool>>,
    /// Handle to the running collection task
    task_handle: Arc<Mutex<Option<JoinHandle<()>>>>,
    /// System metrics storage
    system_metrics: Arc<SystemMetrics>,
    /// Metrics registry for recording metrics
    metrics_registry: Arc<MetricsRegistry>,
}

impl MetricsCollector {
    /// Create a new metrics collector
    pub fn new(config: MetricsCollectorConfig, metrics_registry: Arc<MetricsRegistry>) -> Self {
        Self {
            config,
            running: Arc::new(Mutex::new(false)),
            task_handle: Arc::new(Mutex::new(None)),
            system_metrics: Arc::new(SystemMetrics::new()),
            metrics_registry,
        }
    }

    /// Start the metrics collector
    pub async fn start(&self) -> Result<(), String> {
        let mut running = self.running.lock().await;
        if *running {
            return Err("Metrics collector is already running".to_string());
        }

        *running = true;
        let running_clone = self.running.clone();
        let config_clone = self.config.clone();
        let system_metrics_clone = self.system_metrics.clone();
        let metrics_registry_clone = self.metrics_registry.clone();

        let handle = tokio::spawn(async move {
            let mut sys = System::new_all();
            let mut basic_interval = time::interval(config_clone.basic_collection_interval);
            let mut extended_interval = time::interval(config_clone.extended_collection_interval);
            let mut extended_due = false;

            loop {
                tokio::select! {
                    _ = basic_interval.tick() => {
                        if !*running_clone.lock().await {
                            break;
                        }
                        
                        let start_time = Instant::now();
                        Self::collect_metrics(&mut sys, system_metrics_clone.as_ref(), metrics_registry_clone.as_ref(), 
                                            &config_clone.metrics_types, extended_due);
                        extended_due = false;
                        
                        let duration = start_time.elapsed().as_secs_f64();
                        system_metrics_clone.record_metrics_collection_time(duration);
                    }
                    _ = extended_interval.tick() => {
                        extended_due = true;
                    }
                }
            }
        });

        *self.task_handle.lock().await = Some(handle);
        Ok(())
    }

    /// Stop the metrics collector
    pub async fn stop(&self) -> Result<(), String> {
        let mut running = self.running.lock().await;
        if !*running {
            return Err("Metrics collector is not running".to_string());
        }

        *running = false;
        
        if let Some(handle) = self.task_handle.lock().await.take() {
            if !handle.is_finished() {
                // Wait for the task to complete cleanly
                tokio::time::timeout(Duration::from_secs(5), handle).await
                    .map_err(|_| "Timeout waiting for metrics collector to stop".to_string())?
                    .map_err(|e| format!("Error stopping metrics collector: {}", e))?;
            }
        }

        Ok(())
    }

    /// Collect metrics manually (one-time collection)
    pub fn collect_now(&self) -> Result<(), String> {
        let mut sys = System::new_all();
        let metrics_types = self.config.metrics_types;
        
        Self::collect_metrics(&mut sys, self.system_metrics.as_ref(), self.metrics_registry.as_ref(), &metrics_types, true);
        
        Ok(())
    }

    /// Get a reference to the system metrics
    pub fn system_metrics(&self) -> Arc<SystemMetrics> {
        self.system_metrics.clone()
    }

    /// Private method to collect metrics based on config
    fn collect_metrics(
        sys: &mut System, 
        system_metrics: &SystemMetrics, 
        metrics_registry: &MetricsRegistry,
        metrics_types: &MetricsTypes,
        collect_extended: bool
    ) {
        // Always refresh basic system information
        sys.refresh_memory();
        sys.refresh_cpu();
        sys.refresh_processes();
        
        // Collect extended info if due
        if collect_extended {
            sys.refresh_disks_list();
            sys.refresh_networks_list();
        }

        // Memory metrics
        if metrics_types.memory {
            let total_memory = sys.total_memory() * 1024; // Convert KB to bytes
            let used_memory = sys.used_memory() * 1024;
            let total_swap = sys.total_swap() * 1024;
            let used_swap = sys.used_swap() * 1024;
            
            system_metrics.record_memory_usage(total_memory, used_memory, total_swap, used_swap);
            
            // Record to metrics registry
            metrics_registry.gauge("system.memory.total_bytes", total_memory as f64);
            metrics_registry.gauge("system.memory.used_bytes", used_memory as f64);
            metrics_registry.gauge("system.memory.utilization_pct", (used_memory as f64 / total_memory as f64) * 100.0);
            metrics_registry.gauge("system.swap.total_bytes", total_swap as f64);
            metrics_registry.gauge("system.swap.used_bytes", used_swap as f64);
        }

        // CPU metrics
        if metrics_types.cpu {
            let cpu_count = sys.cpus().len() as u64;
            let global_cpu_usage = sys.global_cpu_info().cpu_usage();
            
            system_metrics.record_cpu_usage(global_cpu_usage, cpu_count);
            
            // Record to metrics registry
            metrics_registry.gauge("system.cpu.usage_pct", global_cpu_usage);
            metrics_registry.gauge("system.cpu.count", cpu_count as f64);
            
            // Per-CPU metrics
            for (i, cpu) in sys.cpus().iter().enumerate() {
                metrics_registry.gauge(&format!("system.cpu.{}.usage_pct", i), cpu.cpu_usage());
            }
        }

        // Process metrics
        if metrics_types.process {
            // Get the current process
            let pid = std::process::id() as i32;
            if let Some(process) = sys.process(sysinfo::Pid::from(pid)) {
                let process_memory = process.memory() * 1024; // Convert KB to bytes
                let process_cpu = process.cpu_usage();
                let process_uptime = process.run_time();
                
                // Get disk I/O if available
                let disk_read = process.disk_usage().read_bytes;
                let disk_written = process.disk_usage().written_bytes;
                
                system_metrics.record_process_metrics(
                    process_memory,
                    process_cpu,
                    process_uptime,
                    disk_read,
                    disk_written
                );
                
                // Record to metrics registry
                metrics_registry.gauge("process.memory_bytes", process_memory as f64);
                metrics_registry.gauge("process.cpu_pct", process_cpu);
                metrics_registry.gauge("process.uptime_sec", process_uptime as f64);
                metrics_registry.gauge("process.disk_read_bytes", disk_read as f64);
                metrics_registry.gauge("process.disk_written_bytes", disk_written as f64);
            }
        }

        // Disk metrics (only when extended collection is due)
        if metrics_types.disk && collect_extended {
            for disk in sys.disks() {
                let name = disk.name().to_string_lossy().to_string();
                let total_space = disk.total_space();
                let available_space = disk.available_space();
                let disk_type = format!("{:?}", disk.type_());
                
                system_metrics.record_disk_metrics(
                    name.clone(),
                    total_space,
                    available_space,
                    disk_type
                );
                
                // Record to metrics registry
                let mount_point = disk.mount_point().to_string_lossy().to_string();
                let mount_point_safe = mount_point.replace("/", "_").replace(":", "_");
                let metric_prefix = format!("system.disk.{}", mount_point_safe);
                
                metrics_registry.gauge(&format!("{}.total_bytes", metric_prefix), total_space as f64);
                metrics_registry.gauge(&format!("{}.available_bytes", metric_prefix), available_space as f64);
                let used = total_space.saturating_sub(available_space);
                metrics_registry.gauge(&format!("{}.used_bytes", metric_prefix), used as f64);
                
                if total_space > 0 {
                    let usage_pct = (used as f64 / total_space as f64) * 100.0;
                    metrics_registry.gauge(&format!("{}.usage_pct", metric_prefix), usage_pct);
                }
            }
        }

        // Network metrics (only when extended collection is due)
        if metrics_types.network && collect_extended {
            for (interface_name, network) in sys.networks() {
                let received_bytes = network.total_received();
                let transmitted_bytes = network.total_transmitted();
                let received_packets = network.total_packets_received();
                let transmitted_packets = network.total_packets_transmitted();
                let receive_errors = network.total_errors_on_received();
                let transmit_errors = network.total_errors_on_transmitted();
                
                system_metrics.record_network_metrics(
                    interface_name.to_string(),
                    received_bytes,
                    transmitted_bytes,
                    received_packets,
                    transmitted_packets,
                    receive_errors,
                    transmit_errors
                );
                
                // Record to metrics registry
                let interface_safe = interface_name.replace(".", "_");
                let metric_prefix = format!("system.network.{}", interface_safe);
                
                metrics_registry.gauge(&format!("{}.received_bytes", metric_prefix), received_bytes as f64);
                metrics_registry.gauge(&format!("{}.transmitted_bytes", metric_prefix), transmitted_bytes as f64);
                metrics_registry.gauge(&format!("{}.received_packets", metric_prefix), received_packets as f64);
                metrics_registry.gauge(&format!("{}.transmitted_packets", metric_prefix), transmitted_packets as f64);
                metrics_registry.gauge(&format!("{}.receive_errors", metric_prefix), receive_errors as f64);
                metrics_registry.gauge(&format!("{}.transmit_errors", metric_prefix), transmit_errors as f64);
            }
        }

        // Record general uptime
        metrics_registry.gauge("system.uptime_sec", system_metrics.node_uptime() as f64);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use tokio::time::sleep;

    #[tokio::test]
    async fn test_metrics_collector_lifecycle() {
        let config = MetricsCollectorConfig::default();
        let registry = Arc::new(MetricsRegistry::default());
        let collector = MetricsCollector::new(config, registry);

        // Start the collector
        let start_result = collector.start().await;
        assert!(start_result.is_ok());

        // Sleep to allow some metrics collection
        sleep(Duration::from_millis(100)).await;

        // Stop the collector
        let stop_result = collector.stop().await;
        assert!(stop_result.is_ok());

        // Manual collection should still work
        let collect_result = collector.collect_now();
        assert!(collect_result.is_ok());
    }
} 