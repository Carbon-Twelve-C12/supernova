use std::collections::HashMap;
use std::time::{Duration, Instant};
use tracing::{info, warn};

/// Faucet for distributing test coins
pub struct Faucet {
    /// Amount of coins to distribute per request
    distribution_amount: u64,
    /// Cooldown period in seconds between requests
    cooldown_period: u64,
    /// Last distribution time per address
    last_distribution: HashMap<String, Instant>,
    /// Total coins distributed
    total_distributed: u64,
    /// Distribution count
    distribution_count: u64,
}

impl Faucet {
    /// Create a new test faucet
    pub fn new(distribution_amount: u64, cooldown_period: u64) -> Self {
        info!(
            "Test faucet initialized: {} satoshis per request, {} seconds cooldown",
            distribution_amount, cooldown_period
        );
        
        Self {
            distribution_amount,
            cooldown_period,
            last_distribution: HashMap::new(),
            total_distributed: 0,
            distribution_count: 0,
        }
    }
    
    /// Distribute coins to a recipient
    pub fn distribute_coins(&mut self, recipient: &str) -> Result<u64, String> {
        // Validate recipient address
        self.validate_address(recipient)?;
        
        // Check cooldown period
        if let Some(last_time) = self.last_distribution.get(recipient) {
            let elapsed = last_time.elapsed();
            let cooldown = Duration::from_secs(self.cooldown_period);
            
            if elapsed < cooldown {
                let remaining = cooldown.as_secs() - elapsed.as_secs();
                return Err(format!(
                    "Cooldown period not elapsed. Please wait {} more seconds",
                    remaining
                ));
            }
        }
        
        // Update distribution records
        self.last_distribution.insert(recipient.to_string(), Instant::now());
        self.total_distributed += self.distribution_amount;
        self.distribution_count += 1;
        
        info!(
            "Faucet distributed {} satoshis to {}",
            self.distribution_amount, recipient
        );
        
        Ok(self.distribution_amount)
    }
    
    /// Get faucet statistics
    pub fn get_statistics(&self) -> FaucetStatistics {
        FaucetStatistics {
            distribution_amount: self.distribution_amount,
            cooldown_period: self.cooldown_period,
            total_distributed: self.total_distributed,
            distribution_count: self.distribution_count,
            unique_recipients: self.last_distribution.len(),
        }
    }
    
    /// Validate a recipient address
    fn validate_address(&self, address: &str) -> Result<(), String> {
        // Basic validation for demonstration
        if address.is_empty() {
            return Err("Empty address is not valid".to_string());
        }
        
        // More comprehensive validation would be implemented in a real system
        if !address.starts_with("test1") && !address.starts_with("tb1") {
            warn!("Address {} does not use testnet prefix", address);
        }
        
        Ok(())
    }
    
    /// Set a new distribution amount
    pub fn set_distribution_amount(&mut self, amount: u64) {
        self.distribution_amount = amount;
        info!("Faucet distribution amount updated to {}", amount);
    }
    
    /// Set a new cooldown period
    pub fn set_cooldown_period(&mut self, period: u64) {
        self.cooldown_period = period;
        info!("Faucet cooldown period updated to {} seconds", period);
    }
    
    /// Clear cooldown for a specific address
    pub fn clear_cooldown(&mut self, address: &str) {
        if self.last_distribution.remove(address).is_some() {
            info!("Cleared cooldown for address {}", address);
        }
    }
    
    /// Reset all cooldowns
    pub fn reset_all_cooldowns(&mut self) {
        let count = self.last_distribution.len();
        self.last_distribution.clear();
        info!("Reset cooldowns for {} addresses", count);
    }
}

/// Statistics about faucet usage
#[derive(Debug, Clone)]
pub struct FaucetStatistics {
    /// Amount distributed per request
    pub distribution_amount: u64,
    /// Cooldown period in seconds
    pub cooldown_period: u64,
    /// Total amount distributed
    pub total_distributed: u64,
    /// Number of distributions made
    pub distribution_count: u64,
    /// Number of unique recipients
    pub unique_recipients: usize,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread::sleep;
    
    #[test]
    fn test_faucet_distribution() {
        let mut faucet = Faucet::new(1000, 1); // 1000 satoshis, 1 second cooldown
        
        // First distribution should succeed
        let result = faucet.distribute_coins("test1abcdef");
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 1000);
        
        // Second immediate distribution should fail due to cooldown
        let result = faucet.distribute_coins("test1abcdef");
        assert!(result.is_err());
        
        // Wait for cooldown to elapse
        sleep(Duration::from_secs(2));
        
        // Distribution should now succeed
        let result = faucet.distribute_coins("test1abcdef");
        assert!(result.is_ok());
        
        // Check statistics
        let stats = faucet.get_statistics();
        assert_eq!(stats.distribution_amount, 1000);
        assert_eq!(stats.total_distributed, 2000);
        assert_eq!(stats.distribution_count, 2);
        assert_eq!(stats.unique_recipients, 1);
    }
} 