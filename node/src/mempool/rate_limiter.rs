//! Mempool Rate Limiter - DoS Protection
//!
//! SECURITY MODULE (P1-003): Rate limiting and memory management for mempool
//! 
//! This module prevents denial-of-service attacks through transaction flooding
//! by implementing:
//! - Per-peer rate limiting
//! - Global memory caps
//! - Fee-based transaction eviction
//! - Bandwidth usage tracking

use dashmap::DashMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tracing::{debug, warn};

use crate::mempool::error::MempoolError;

// ============================================================================
// SECURITY FIX (P1-003): Mempool DoS Protection Configuration
// ============================================================================

/// Mempool DoS protection configuration
pub struct MempoolDoSConfig;

impl MempoolDoSConfig {
    /// Maximum transactions per peer per minute
    /// 
    /// SECURITY: Prevents single peer from flooding mempool.
    /// 100 txs/minute = reasonable for legitimate usage, blocks flooding.
    pub const MAX_TXS_PER_PEER_PER_MINUTE: usize = 100;
    
    /// Maximum mempool size in bytes
    /// 
    /// SECURITY: 300MB cap prevents memory exhaustion.
    /// Average tx size ~250 bytes = ~1.2M transactions max
    pub const MAX_MEMPOOL_BYTES: usize = 300 * 1024 * 1024; // 300MB
    
    /// Minimum fee rate (novas per byte)
    /// 
    /// Prevents dust/spam transactions with zero fees.
    pub const MIN_FEE_RATE: u64 = 1000; // 1000 novas/byte
    
    /// Rate limit window duration
    pub const RATE_LIMIT_WINDOW: Duration = Duration::from_secs(60); // 1 minute
    
    /// Maximum transaction size in bytes
    /// 
    /// Prevents single massive transaction from consuming all memory.
    pub const MAX_SINGLE_TX_SIZE: usize = 1 * 1024 * 1024; // 1MB max
}

/// Per-peer rate limit tracker
#[derive(Debug)]
struct PeerRateLimit {
    /// Transaction count in current window
    tx_count: usize,
    /// Window start time
    window_start: Instant,
    /// Total bytes submitted
    bytes_submitted: usize,
}

impl PeerRateLimit {
    fn new() -> Self {
        Self {
            tx_count: 0,
            window_start: Instant::now(),
            bytes_submitted: 0,
        }
    }
    
    /// Check and update rate limit
    /// 
    /// Returns true if within limit, false if exceeded
    fn check_and_update(&mut self, tx_size: usize) -> bool {
        let now = Instant::now();
        
        // Reset window if expired
        if now.duration_since(self.window_start) >= MempoolDoSConfig::RATE_LIMIT_WINDOW {
            self.tx_count = 0;
            self.bytes_submitted = 0;
            self.window_start = now;
        }
        
        // Check if adding this tx would exceed limit
        if self.tx_count >= MempoolDoSConfig::MAX_TXS_PER_PEER_PER_MINUTE {
            return false;
        }
        
        // Update counters
        self.tx_count += 1;
        self.bytes_submitted += tx_size;
        true
    }
}

/// Mempool rate limiter with DoS protection
/// 
/// SECURITY: Multi-layered DoS protection:
/// 1. Per-peer rate limiting
/// 2. Global memory cap
/// 3. Transaction size validation
/// 4. Fee-based prioritization
pub struct MempoolRateLimiter {
    /// Per-peer rate limits using DashMap for concurrent access
    peer_limits: Arc<DashMap<String, PeerRateLimit>>,
    
    /// Global memory usage (atomic for lock-free access)
    global_memory_usage: Arc<AtomicUsize>,
    
    /// Statistics
    rejected_by_rate_limit: Arc<AtomicUsize>,
    rejected_by_memory: Arc<AtomicUsize>,
    rejected_by_size: Arc<AtomicUsize>,
}

impl MempoolRateLimiter {
    /// Create a new rate limiter
    pub fn new() -> Self {
        Self {
            peer_limits: Arc::new(DashMap::new()),
            global_memory_usage: Arc::new(AtomicUsize::new(0)),
            rejected_by_rate_limit: Arc::new(AtomicUsize::new(0)),
            rejected_by_memory: Arc::new(AtomicUsize::new(0)),
            rejected_by_size: Arc::new(AtomicUsize::new(0)),
        }
    }
    
    /// Check if transaction submission is allowed
    /// 
    /// SECURITY: Validates against all DoS protection rules before accepting transaction.
    ///
    /// # Arguments
    /// * `peer_id` - Identifier of submitting peer (IP or node ID)
    /// * `tx_size` - Size of transaction in bytes
    /// * `fee_rate` - Fee rate in novas per byte
    ///
    /// # Returns
    /// * `Ok(())` - Transaction passes all checks
    /// * `Err(MempoolError)` - Transaction violates DoS protection rules
    pub fn check_rate_limit(
        &self,
        peer_id: Option<&str>,
        tx_size: usize,
        fee_rate: u64,
    ) -> Result<(), MempoolError> {
        // Check 1: Transaction size limit
        if tx_size > MempoolDoSConfig::MAX_SINGLE_TX_SIZE {
            self.rejected_by_size.fetch_add(1, Ordering::Relaxed);
            return Err(MempoolError::TransactionTooLarge {
                size: tx_size,
                max: MempoolDoSConfig::MAX_SINGLE_TX_SIZE,
            });
        }
        
        // Check 2: Global memory cap
        let current_usage = self.global_memory_usage.load(Ordering::Relaxed);
        if current_usage + tx_size > MempoolDoSConfig::MAX_MEMPOOL_BYTES {
            self.rejected_by_memory.fetch_add(1, Ordering::Relaxed);
            return Err(MempoolError::MemoryLimitExceeded {
                current: current_usage,
                max: MempoolDoSConfig::MAX_MEMPOOL_BYTES,
                tx_size,
            });
        }
        
        // Check 3: Minimum fee rate
        if fee_rate < MempoolDoSConfig::MIN_FEE_RATE {
            return Err(MempoolError::FeeTooLow {
                required: MempoolDoSConfig::MIN_FEE_RATE,
                provided: fee_rate,
            });
        }
        
        // Check 4: Per-peer rate limit (if peer is known)
        if let Some(peer) = peer_id {
            let mut rate_limit = self.peer_limits
                .entry(peer.to_string())
                .or_insert_with(PeerRateLimit::new);
            
            if !rate_limit.check_and_update(tx_size) {
                self.rejected_by_rate_limit.fetch_add(1, Ordering::Relaxed);
                
                debug!(
                    "Peer {} exceeded rate limit: {} txs/minute",
                    peer,
                    MempoolDoSConfig::MAX_TXS_PER_PEER_PER_MINUTE
                );
                
                return Err(MempoolError::RateLimitExceeded {
                    peer: peer.to_string(),
                    limit: MempoolDoSConfig::MAX_TXS_PER_PEER_PER_MINUTE,
                });
            }
        }
        
        Ok(())
    }
    
    /// Record transaction addition (updates memory usage)
    pub fn record_addition(&self, tx_size: usize) {
        self.global_memory_usage.fetch_add(tx_size, Ordering::Relaxed);
    }
    
    /// Record transaction removal (updates memory usage)
    pub fn record_removal(&self, tx_size: usize) {
        self.global_memory_usage.fetch_sub(tx_size, Ordering::Relaxed);
    }
    
    /// Get current memory usage
    pub fn current_memory_usage(&self) -> usize {
        self.global_memory_usage.load(Ordering::Relaxed)
    }
    
    /// Get DoS protection statistics
    pub fn get_stats(&self) -> MempoolDoSStats {
        MempoolDoSStats {
            current_memory_bytes: self.global_memory_usage.load(Ordering::Relaxed),
            max_memory_bytes: MempoolDoSConfig::MAX_MEMPOOL_BYTES,
            rejected_by_rate_limit: self.rejected_by_rate_limit.load(Ordering::Relaxed),
            rejected_by_memory: self.rejected_by_memory.load(Ordering::Relaxed),
            rejected_by_size: self.rejected_by_size.load(Ordering::Relaxed),
            active_peer_limits: self.peer_limits.len(),
        }
    }
    
    /// Clean up expired peer rate limit entries
    pub fn cleanup_expired_limits(&self) {
        let now = Instant::now();
        self.peer_limits.retain(|_, limit| {
            // Keep if window is still active
            now.duration_since(limit.window_start) < MempoolDoSConfig::RATE_LIMIT_WINDOW * 10
        });
    }
}

/// DoS protection statistics
#[derive(Debug, Clone)]
pub struct MempoolDoSStats {
    pub current_memory_bytes: usize,
    pub max_memory_bytes: usize,
    pub rejected_by_rate_limit: usize,
    pub rejected_by_memory: usize,
    pub rejected_by_size: usize,
    pub active_peer_limits: usize,
}

