use prometheus::{
    Registry, IntGauge, IntGaugeVec, Gauge, GaugeVec, Opts,
    core::{AtomicF64, AtomicI64, GenericGauge, GenericGaugeVec}
};
use std::sync::Arc;
use std::time::Duration;
use tokio::time::interval;
use tokio::task::JoinHandle;
use crate::monitoring::MetricsError;
use tracing::{info, warn, error, debug};
use sysinfo::{System, SystemExt, ProcessorExt, DiskExt, NetworkExt, ComponentExt, CpuExt};

/// System metrics collector
pub struct SystemMetrics {
    /// CPU usage percentage by core
    cpu_usage: GaugeVec,
    /// Load average (1m, 5m, 15m)
    load_average: GaugeVec,
    /// Memory usage
    memory_usage: IntGaugeVec,
    /// Swap usage
    swap_usage: IntGaugeVec,
    /// Disk usage
    disk_usage: GaugeVec,
    /// Disk IO operations
    disk_io: IntGaugeVec,
    /// Network traffic
    network_traffic: IntGaugeVec,
    /// System temperature
    temperature: GaugeVec,
    /// System uptime
    uptime: IntGauge,
    /// Task handle for metrics collection
    collection_task: Option<JoinHandle<()>>,
    /// System information collector
    system: Arc<tokio::sync::Mutex<System>>,
}

impl SystemMetrics {
    /// Create a new system metrics collector
    pub fn new(registry: &Registry, namespace: &str) -> Result<Self, MetricsError> {
        // Create CPU usage metrics
        let cpu_usage = GaugeVec::new(
            Opts::new("cpu_usage_percent", "CPU usage percentage by core")
                .namespace(namespace.to_string())
                .subsystem("system"),
            &["core"],
        )?;
        registry.register(Box::new(cpu_usage.clone()))?;
        
        // Create load average metrics
        let load_average = GaugeVec::new(
            Opts::new("load_average", "System load average")
                .namespace(namespace.to_string())
                .subsystem("system"),
            &["period"],
        )?;
        registry.register(Box::new(load_average.clone()))?;
        
        // Create memory usage metrics
        let memory_usage = IntGaugeVec::new(
            Opts::new("memory_bytes", "Memory usage in bytes")
                .namespace(namespace.to_string())
                .subsystem("system"),
            &["type"],
        )?;
        registry.register(Box::new(memory_usage.clone()))?;
        
        // Create swap usage metrics
        let swap_usage = IntGaugeVec::new(
            Opts::new("swap_bytes", "Swap usage in bytes")
                .namespace(namespace.to_string())
                .subsystem("system"),
            &["type"],
        )?;
        registry.register(Box::new(swap_usage.clone()))?;
        
        // Create disk usage metrics
        let disk_usage = GaugeVec::new(
            Opts::new("disk_usage", "Disk usage information")
                .namespace(namespace.to_string())
                .subsystem("system"),
            &["device", "type"],
        )?;
        registry.register(Box::new(disk_usage.clone()))?;
        
        // Create disk IO metrics
        let disk_io = IntGaugeVec::new(
            Opts::new("disk_io_bytes", "Disk IO operations in bytes")
                .namespace(namespace.to_string())
                .subsystem("system"),
            &["device", "operation"],
        )?;
        registry.register(Box::new(disk_io.clone()))?;
        
        // Create network traffic metrics
        let network_traffic = IntGaugeVec::new(
            Opts::new("network_bytes", "Network traffic in bytes")
                .namespace(namespace.to_string())
                .subsystem("system"),
            &["interface", "direction"],
        )?;
        registry.register(Box::new(network_traffic.clone()))?;
        
        // Create temperature metrics
        let temperature = GaugeVec::new(
            Opts::new("temperature_celsius", "System temperature in Celsius")
                .namespace(namespace.to_string())
                .subsystem("system"),
            &["sensor"],
        )?;
        registry.register(Box::new(temperature.clone()))?;
        
        // Create uptime metric
        let uptime = IntGauge::with_opts(
            Opts::new("uptime_seconds", "System uptime in seconds")
                .namespace(namespace.to_string())
                .subsystem("system")
        )?;
        registry.register(Box::new(uptime.clone()))?;
        
        // Initialize system info collector
        let mut system = System::new_all();
        system.refresh_all();
        
        Ok(Self {
            cpu_usage,
            load_average,
            memory_usage,
            swap_usage,
            disk_usage,
            disk_io,
            network_traffic,
            temperature,
            uptime,
            collection_task: None,
            system: Arc::new(tokio::sync::Mutex::new(system)),
        })
    }
    
    /// Start system metrics collection in the background
    pub fn start_collection(&mut self, interval_duration: Duration) -> Result<(), MetricsError> {
        let cpu_usage = self.cpu_usage.clone();
        let load_average = self.load_average.clone();
        let memory_usage = self.memory_usage.clone();
        let swap_usage = self.swap_usage.clone();
        let disk_usage = self.disk_usage.clone();
        let disk_io = self.disk_io.clone();
        let network_traffic = self.network_traffic.clone();
        let temperature = self.temperature.clone();
        let uptime = self.uptime.clone();
        let system = self.system.clone();
        
        // Create a task to collect system metrics in the background
        let handle = tokio::spawn(async move {
            let mut interval_timer = interval(interval_duration);
            
            loop {
                interval_timer.tick().await;
                
                // Refresh system information
                {
                    let mut sys = system.lock().await;
                    sys.refresh_all();
                    
                    // Collect CPU metrics
                    for (i, processor) in sys.cpus().iter().enumerate() {
                        cpu_usage.with_label_values(&[&i.to_string()]).set(processor.cpu_usage().into());
                    }
                    
                    // Collect load average - direct field access
                    let load = sys.load_average();
                    load_average.with_label_values(&["1m"]).set(load.one);
                    load_average.with_label_values(&["5m"]).set(load.five);
                    load_average.with_label_values(&["15m"]).set(load.fifteen);
                    
                    // Collect memory metrics
                    memory_usage.with_label_values(&["total"]).set(sys.total_memory() as i64);
                    memory_usage.with_label_values(&["used"]).set(sys.used_memory() as i64);
                    memory_usage.with_label_values(&["free"]).set(sys.free_memory() as i64);
                    
                    // Collect swap metrics
                    swap_usage.with_label_values(&["total"]).set(sys.total_swap() as i64);
                    swap_usage.with_label_values(&["used"]).set(sys.used_swap() as i64);
                    swap_usage.with_label_values(&["free"]).set(sys.free_swap() as i64);
                    
                    // Collect disk metrics
                    for disk in sys.disks() {
                        let name = disk.name().to_string_lossy();
                        let mount_point = disk.mount_point().to_string_lossy();
                        let device = format!("{} ({})", name, mount_point);
                        
                        let total = disk.total_space();
                        let used = total - disk.available_space();
                        let usage_percent = if total > 0 {
                            (used as f64 / total as f64) * 100.0
                        } else {
                            0.0
                        };
                        
                        disk_usage.with_label_values(&[&device, "total"]).set(total as f64);
                        disk_usage.with_label_values(&[&device, "used"]).set(used as f64);
                        disk_usage.with_label_values(&[&device, "percent"]).set(usage_percent);
                    }
                    
                    // Collect network metrics
                    for (interface_name, data) in sys.networks() {
                        network_traffic.with_label_values(&[interface_name, "received"]).set(data.received() as i64);
                        network_traffic.with_label_values(&[interface_name, "transmitted"]).set(data.transmitted() as i64);
                    }
                    
                    // Collect temperature information
                    for (i, component) in sys.components().iter().enumerate() {
                        let label = format!("{} ({})", component.label(), i);
                        temperature.with_label_values(&[&label]).set(component.temperature().into());
                    }
                    
                    // Collect uptime
                    uptime.set(sys.uptime() as i64);
                }
                
                debug!("Collected system metrics");
            }
        });
        
        self.collection_task = Some(handle);
        Ok(())
    }
    
    /// Get the latest CPU usage percentage for a specific core
    pub async fn get_cpu_usage(&self, core: usize) -> f64 {
        self.cpu_usage.with_label_values(&[&core.to_string()]).get()
    }
    
    /// Get the latest memory usage
    pub async fn get_memory_usage(&self) -> (u64, u64, u64) {
        let system = self.system.lock().await;
        (system.total_memory(), system.used_memory(), system.free_memory())
    }
    
    /// Get current system load average
    pub async fn get_load_average(&self) -> (f64, f64, f64) {
        let system = self.system.lock().await;
        let load = system.load_average();
        (load.one, load.five, load.fifteen)
    }
    
    /// Get all system metrics as a formatted string
    pub async fn get_summary(&self) -> String {
        let system = self.system.lock().await;
        
        let mut summary = String::new();
        summary.push_str("=== System Metrics Summary ===\n");
        
        // CPU information
        summary.push_str("CPU Usage:\n");
        for (i, processor) in system.cpus().iter().enumerate() {
            summary.push_str(&format!("  Core {}: {:.1}%\n", i, processor.cpu_usage()));
        }
        
        // Memory information
        let total_memory = system.total_memory();
        let used_memory = system.used_memory();
        let free_memory = system.free_memory();
        summary.push_str(&format!(
            "Memory: {:.1} GB total, {:.1} GB used ({:.1}%), {:.1} GB free\n",
            total_memory as f64 / 1_000_000_000.0,
            used_memory as f64 / 1_000_000_000.0,
            (used_memory as f64 / total_memory as f64) * 100.0,
            free_memory as f64 / 1_000_000_000.0,
        ));
        
        // Disk information
        summary.push_str("Disk Usage:\n");
        for disk in system.disks() {
            let name = disk.name().to_string_lossy();
            let mount_point = disk.mount_point().to_string_lossy();
            let total = disk.total_space();
            let available = disk.available_space();
            let used = total - available;
            let percent = if total > 0 { (used as f64 / total as f64) * 100.0 } else { 0.0 };
            
            summary.push_str(&format!(
                "  {} ({}): {:.1} GB total, {:.1} GB used ({:.1}%)\n",
                name,
                mount_point,
                total as f64 / 1_000_000_000.0,
                used as f64 / 1_000_000_000.0,
                percent,
            ));
        }
        
        // Network information
        summary.push_str("Network Traffic:\n");
        for (interface, data) in system.networks() {
            summary.push_str(&format!(
                "  {}: {:.1} MB received, {:.1} MB transmitted\n",
                interface,
                data.received() as f64 / 1_000_000.0,
                data.transmitted() as f64 / 1_000_000.0,
            ));
        }
        
        // Uptime
        let uptime = system.uptime();
        let days = uptime / 86400;
        let hours = (uptime % 86400) / 3600;
        let minutes = (uptime % 3600) / 60;
        let seconds = uptime % 60;
        summary.push_str(&format!(
            "System Uptime: {}d {}h {}m {}s\n",
            days, hours, minutes, seconds
        ));
        
        summary
    }
}

impl Drop for SystemMetrics {
    fn drop(&mut self) {
        // Cancel the metrics collection task if it exists
        if let Some(handle) = self.collection_task.take() {
            handle.abort();
        }
    }
} 