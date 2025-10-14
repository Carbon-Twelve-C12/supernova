use std::collections::HashMap;
use std::time::Duration;
use thiserror::Error;
use tracing::info;

/// Error types for network simulation
#[derive(Debug, Error)]
pub enum NetworkSimulationError {
    #[error("Node not found: {0}")]
    NodeNotFound(usize),

    #[error("Invalid network condition: {0}")]
    InvalidCondition(String),

    #[error("Simulation error: {0}")]
    SimulationError(String),

    #[error("Configuration error: {0}")]
    ConfigError(String),
}

/// Represents the condition of a network connection between two nodes
#[derive(Debug, Clone, Default)]
pub struct NetworkCondition {
    /// Simulated latency in milliseconds
    pub latency_ms: Option<u64>,
    /// Packet loss percentage (0-100)
    pub packet_loss_percent: Option<u8>,
    /// Bandwidth limitation in kilobits per second
    pub bandwidth_kbps: Option<u64>,
    /// Whether the connection is completely severed
    pub is_severed: bool,
}

// Define our own type instead of using a type alias
#[derive(Debug, Clone)]
pub struct SimulationConfig {
    /// Whether to enable network simulation
    pub enabled: bool,
    /// Simulated latency in milliseconds (mean)
    pub latency_ms_mean: u64,
    /// Latency standard deviation in milliseconds
    pub latency_ms_std_dev: u64,
    /// Packet loss percentage (0-100)
    pub packet_loss_percent: u8,
    /// Bandwidth limit in kilobits per second (0 = unlimited)
    pub bandwidth_limit_kbps: u64,
    /// Whether to simulate clock drift
    pub simulate_clock_drift: bool,
    /// Maximum clock drift in milliseconds
    pub max_clock_drift_ms: u64,
    /// Network jitter simulation (random latency variation)
    pub jitter_ms: u64,
}

impl Default for SimulationConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            latency_ms_mean: 100,
            latency_ms_std_dev: 20,
            packet_loss_percent: 0,
            bandwidth_limit_kbps: 1000,
            simulate_clock_drift: false,
            max_clock_drift_ms: 100,
            jitter_ms: 10,
        }
    }
}

/// Network simulator for testing network conditions
pub struct NetworkSimulator {
    /// Global configuration
    config: SimulationConfig,
    /// Network conditions between specific node pairs (from_node, to_node) -> condition
    conditions: HashMap<(usize, usize), NetworkCondition>,
    /// Node clocks with drift in milliseconds (node_id -> drift_ms, can be negative)
    clock_drift_ms: HashMap<usize, i64>,
}

impl NetworkSimulator {
    /// Create a new network simulator
    pub fn new(config: SimulationConfig) -> Self {
        Self {
            config,
            conditions: HashMap::new(),
            clock_drift_ms: HashMap::new(),
        }
    }

    /// Set network conditions for a connection between two nodes
    pub fn set_connection_condition(
        &mut self,
        from_node: usize,
        to_node: usize,
        latency_ms: Option<u64>,
        packet_loss_percent: Option<u8>,
        bandwidth_kbps: Option<u64>,
    ) -> Result<(), String> {
        // Validate inputs
        if let Some(loss) = packet_loss_percent {
            if loss > 100 {
                return Err("Packet loss percentage cannot exceed 100%".to_string());
            }
        }

        let mut condition = self
            .conditions
            .entry((from_node, to_node))
            .or_default()
            .clone();

        // Update condition
        if let Some(latency) = latency_ms {
            condition.latency_ms = Some(latency);
        }

        if let Some(loss) = packet_loss_percent {
            condition.packet_loss_percent = Some(loss);
        }

        if let Some(bandwidth) = bandwidth_kbps {
            condition.bandwidth_kbps = Some(bandwidth);
        }

        // Store updated condition
        self.conditions
            .insert((from_node, to_node), condition.clone());

        info!(
            "Set network condition from node {} to {}: latency={:?}ms, loss={:?}%, bandwidth={:?}kbps",
            from_node,
            to_node,
            condition.latency_ms,
            condition.packet_loss_percent,
            condition.bandwidth_kbps
        );

        Ok(())
    }

    /// Create a network partition between two groups of nodes
    pub fn create_partition(&mut self, group_a: &[usize], group_b: &[usize]) -> Result<(), String> {
        if group_a.is_empty() || group_b.is_empty() {
            return Err("Both groups must contain at least one node".to_string());
        }

        // For each node in group A, sever connection to each node in group B (both directions)
        for &a in group_a {
            for &b in group_b {
                self.sever_connection(a, b)?;
                self.sever_connection(b, a)?;
            }
        }

        info!(
            "Created network partition between groups: {:?} and {:?}",
            group_a, group_b
        );

        Ok(())
    }

    /// Heal a network partition between two groups
    pub fn heal_partition(&mut self, group_a: &[usize], group_b: &[usize]) -> Result<(), String> {
        if group_a.is_empty() || group_b.is_empty() {
            return Err("Both groups must contain at least one node".to_string());
        }

        // For each node in group A, restore connection to each node in group B (both directions)
        for &a in group_a {
            for &b in group_b {
                self.restore_connection(a, b)?;
                self.restore_connection(b, a)?;
            }
        }

        info!(
            "Healed network partition between groups: {:?} and {:?}",
            group_a, group_b
        );

        Ok(())
    }

    /// Sever connection between two nodes
    pub fn sever_connection(&mut self, from_node: usize, to_node: usize) -> Result<(), String> {
        let mut condition = self
            .conditions
            .entry((from_node, to_node))
            .or_default()
            .clone();

        condition.is_severed = true;

        // Store updated condition
        self.conditions.insert((from_node, to_node), condition);

        info!("Severed connection from node {} to {}", from_node, to_node);

        Ok(())
    }

    /// Restore connection between two nodes
    pub fn restore_connection(&mut self, from_node: usize, to_node: usize) -> Result<(), String> {
        if let Some(condition) = self.conditions.get_mut(&(from_node, to_node)) {
            condition.is_severed = false;
            info!("Restored connection from node {} to {}", from_node, to_node);
        }

        Ok(())
    }

    /// Simulate latency for a message between nodes
    pub fn apply_latency(&self, from_node: usize, to_node: usize) -> Duration {
        let base_latency = if let Some(condition) = self.conditions.get(&(from_node, to_node)) {
            if condition.is_severed {
                return Duration::from_secs(3600); // Effectively never delivered
            }

            condition.latency_ms.unwrap_or(self.config.latency_ms_mean)
        } else {
            self.config.latency_ms_mean
        };

        // Add some random variation
        let deviation = self.config.latency_ms_std_dev;
        let jitter = if deviation > 0 {
            // Simple randomization - in a real implementation this would use a proper
            // random number generator with normal distribution
            let random_factor = (from_node % 10) as f64 / 10.0 - 0.5; // -0.5 to 0.5
            (random_factor * deviation as f64) as i64
        } else {
            0
        };

        // Calculate final latency, ensuring it doesn't go negative
        let final_latency = (base_latency as i64 + jitter).max(1) as u64;

        Duration::from_millis(final_latency)
    }

    /// Check if a packet should be dropped based on packet loss settings
    pub fn should_drop_packet(&self, from_node: usize, to_node: usize) -> bool {
        let loss_percent = if let Some(condition) = self.conditions.get(&(from_node, to_node)) {
            if condition.is_severed {
                return true; // Always drop if connection is severed
            }

            condition
                .packet_loss_percent
                .unwrap_or(self.config.packet_loss_percent)
        } else {
            self.config.packet_loss_percent
        };

        if loss_percent == 0 {
            return false;
        }

        // Simple deterministic "random" for demonstration
        // In a real implementation, this would use a proper random number generator
        let pseudo_random = (from_node * 31 + to_node * 17) % 100;
        pseudo_random < loss_percent as usize
    }

    /// Get adjusted timestamp for a node based on clock drift
    pub fn get_adjusted_timestamp(&self, node_id: usize, actual_timestamp: u64) -> u64 {
        if !self.config.simulate_clock_drift {
            return actual_timestamp;
        }

        let drift = self
            .clock_drift_ms
            .get(&node_id)
            .copied()
            .unwrap_or_else(|| {
                // If no drift is recorded for this node, generate a deterministic one
                // In a real implementation, this would use a proper random number generator
                let max_drift = self.config.max_clock_drift_ms as i64;

                ((node_id * 13) % (max_drift as usize * 2)) as i64 - max_drift
            });

        // Apply drift to timestamp
        if drift >= 0 {
            actual_timestamp.saturating_add(drift as u64)
        } else {
            actual_timestamp.saturating_sub((-drift) as u64)
        }
    }

    /// Set specific clock drift for a node
    pub fn set_clock_drift(&mut self, node_id: usize, drift_ms: i64) {
        self.clock_drift_ms.insert(node_id, drift_ms);
        info!("Set clock drift for node {}: {}ms", node_id, drift_ms);
    }

    /// Get all current network conditions
    pub fn get_all_conditions(&self) -> HashMap<(usize, usize), NetworkCondition> {
        self.conditions.clone()
    }

    /// Reset all network conditions
    pub fn reset_all_conditions(&mut self) {
        self.conditions.clear();
        self.clock_drift_ms.clear();
        info!("Reset all network conditions");
    }

    /// Calculate bandwidth delay for a message of given size
    pub fn calculate_bandwidth_delay(
        &self,
        from_node: usize,
        to_node: usize,
        message_bytes: usize,
    ) -> Duration {
        let bandwidth = if let Some(condition) = self.conditions.get(&(from_node, to_node)) {
            if condition.is_severed {
                return Duration::from_secs(3600); // Effectively never delivered
            }

            condition
                .bandwidth_kbps
                .unwrap_or(self.config.bandwidth_limit_kbps)
        } else {
            self.config.bandwidth_limit_kbps
        };

        if bandwidth == 0 {
            return Duration::from_millis(0); // Unlimited bandwidth
        }

        // Calculate transfer time in milliseconds
        // Formula: (message_size_in_bits / bandwidth_in_kbps) * 1000ms
        let message_bits = message_bytes * 8;
        let transfer_ms = (message_bits as f64 / (bandwidth as f64 * 1000.0)) * 1000.0;

        Duration::from_millis(transfer_ms as u64)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_network_partition() {
        let config = SimulationConfig {
            enabled: true,
            latency_ms_mean: 100,
            latency_ms_std_dev: 20,
            packet_loss_percent: 0,
            bandwidth_limit_kbps: 1000,
            simulate_clock_drift: false,
            max_clock_drift_ms: 0,
            jitter_ms: 0,
        };

        let mut simulator = NetworkSimulator::new(config);

        // Create a partition
        let group_a = vec![0, 1, 2];
        let group_b = vec![3, 4, 5];

        assert!(simulator.create_partition(&group_a, &group_b).is_ok());

        // Check that packets are dropped between groups
        assert!(simulator.should_drop_packet(0, 3));
        assert!(simulator.should_drop_packet(3, 0));

        // But not within groups
        assert!(!simulator.should_drop_packet(0, 1));
        assert!(!simulator.should_drop_packet(3, 4));

        // Heal the partition
        assert!(simulator.heal_partition(&group_a, &group_b).is_ok());

        // Now packets should flow
        assert!(!simulator.should_drop_packet(0, 3));
        assert!(!simulator.should_drop_packet(3, 0));
    }
}
