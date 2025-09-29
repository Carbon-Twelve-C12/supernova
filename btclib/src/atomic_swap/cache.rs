//! Caching layer for atomic swap operations
//!
//! Provides efficient caching for frequently accessed data including:
//! - Swap session data
//! - Bitcoin transaction lookups
//! - Script validation results
//! - Exchange rate data

use crate::atomic_swap::{error::CacheError, SwapSession, SwapState};
use bitcoin::{Transaction as BitcoinTransaction, Txid};
use dashmap::DashMap;
use lru::LruCache;
use serde::{Deserialize, Serialize};
use std::num::NonZeroUsize;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;

/// Cached item with expiration
#[derive(Clone, Debug)]
struct CachedItem<T> {
    data: T,
    expires_at: Instant,
}

impl<T> CachedItem<T> {
    fn new(data: T, ttl: Duration) -> Self {
        Self {
            data,
            expires_at: Instant::now() + ttl,
        }
    }

    fn is_expired(&self) -> bool {
        Instant::now() > self.expires_at
    }
}

/// Cache configuration
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CacheConfig {
    /// Maximum number of swap sessions to cache
    pub max_swap_sessions: usize,

    /// Maximum number of Bitcoin transactions to cache
    pub max_bitcoin_txs: usize,

    /// Maximum number of script validation results to cache
    pub max_script_results: usize,

    /// TTL for swap session cache entries
    pub swap_session_ttl: Duration,

    /// TTL for Bitcoin transaction cache entries
    pub bitcoin_tx_ttl: Duration,

    /// TTL for script validation results
    pub script_result_ttl: Duration,

    /// TTL for exchange rate data
    pub rate_cache_ttl: Duration,
}

impl Default for CacheConfig {
    fn default() -> Self {
        Self {
            max_swap_sessions: 10_000,
            max_bitcoin_txs: 50_000,
            max_script_results: 100_000,
            swap_session_ttl: Duration::from_secs(300), // 5 minutes
            bitcoin_tx_ttl: Duration::from_secs(3600),  // 1 hour
            script_result_ttl: Duration::from_secs(86400), // 24 hours
            rate_cache_ttl: Duration::from_secs(60),    // 1 minute
        }
    }
}

/// Multi-layer cache for atomic swap operations
pub struct AtomicSwapCache {
    /// Configuration
    config: CacheConfig,

    /// Swap session cache (hot cache)
    swap_sessions: Arc<DashMap<[u8; 32], CachedItem<SwapSession>>>,

    /// Bitcoin transaction cache
    bitcoin_txs: Arc<RwLock<LruCache<Txid, CachedItem<BitcoinTransaction>>>>,

    /// Script validation results cache
    script_results: Arc<DashMap<Vec<u8>, CachedItem<bool>>>,

    /// Exchange rate cache
    rate_cache: Arc<RwLock<LruCache<String, CachedItem<f64>>>>,

    /// Cache statistics
    stats: Arc<CacheStats>,
}

/// Cache statistics for monitoring
#[derive(Default, Debug)]
pub struct CacheStats {
    pub swap_hits: std::sync::atomic::AtomicU64,
    pub swap_misses: std::sync::atomic::AtomicU64,
    pub btc_hits: std::sync::atomic::AtomicU64,
    pub btc_misses: std::sync::atomic::AtomicU64,
    pub script_hits: std::sync::atomic::AtomicU64,
    pub script_misses: std::sync::atomic::AtomicU64,
    pub evictions: std::sync::atomic::AtomicU64,
}

impl AtomicSwapCache {
    /// Create a new cache instance
    pub fn new(config: CacheConfig) -> Self {
        Self {
            swap_sessions: Arc::new(DashMap::new()),
            bitcoin_txs: Arc::new(RwLock::new(LruCache::new(
                NonZeroUsize::new(config.max_bitcoin_txs).unwrap(),
            ))),
            script_results: Arc::new(DashMap::new()),
            rate_cache: Arc::new(RwLock::new(LruCache::new(
                NonZeroUsize::new(100).unwrap(), // Fixed size for rate cache
            ))),
            config,
            stats: Arc::new(CacheStats::default()),
        }
    }

    /// Get a swap session from cache
    pub async fn get_swap_session(&self, swap_id: &[u8; 32]) -> Option<SwapSession> {
        if let Some(entry) = self.swap_sessions.get(swap_id) {
            if !entry.is_expired() {
                self.stats
                    .swap_hits
                    .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                return Some(entry.data.clone());
            } else {
                // Remove expired entry
                self.swap_sessions.remove(swap_id);
                self.stats
                    .evictions
                    .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            }
        }

        self.stats
            .swap_misses
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        None
    }

    /// Cache a swap session
    pub async fn cache_swap_session(&self, swap_id: [u8; 32], session: SwapSession) {
        // Check cache size limit
        if self.swap_sessions.len() >= self.config.max_swap_sessions {
            // Evict oldest entries
            self.evict_expired_swaps().await;
        }

        let cached = CachedItem::new(session, self.config.swap_session_ttl);
        self.swap_sessions.insert(swap_id, cached);
    }

    /// Get a Bitcoin transaction from cache
    pub async fn get_bitcoin_tx(&self, txid: &Txid) -> Option<BitcoinTransaction> {
        let mut cache = self.bitcoin_txs.write().await;

        if let Some(entry) = cache.get(txid) {
            if !entry.is_expired() {
                self.stats
                    .btc_hits
                    .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                return Some(entry.data.clone());
            } else {
                cache.pop(txid);
                self.stats
                    .evictions
                    .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            }
        }

        self.stats
            .btc_misses
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        None
    }

    /// Cache a Bitcoin transaction
    pub async fn cache_bitcoin_tx(&self, txid: Txid, tx: BitcoinTransaction) {
        let mut cache = self.bitcoin_txs.write().await;
        let cached = CachedItem::new(tx, self.config.bitcoin_tx_ttl);
        cache.put(txid, cached);
    }

    /// Get script validation result from cache
    pub async fn get_script_result(&self, script_hash: &[u8]) -> Option<bool> {
        let key = script_hash.to_vec();

        if let Some(entry) = self.script_results.get(&key) {
            if !entry.is_expired() {
                self.stats
                    .script_hits
                    .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                return Some(entry.data);
            } else {
                self.script_results.remove(&key);
                self.stats
                    .evictions
                    .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            }
        }

        self.stats
            .script_misses
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        None
    }

    /// Cache script validation result
    pub async fn cache_script_result(&self, script_hash: Vec<u8>, is_valid: bool) {
        if self.script_results.len() >= self.config.max_script_results {
            self.evict_expired_scripts().await;
        }

        let cached = CachedItem::new(is_valid, self.config.script_result_ttl);
        self.script_results.insert(script_hash, cached);
    }

    /// Get exchange rate from cache
    pub async fn get_exchange_rate(&self, pair: &str) -> Option<f64> {
        let mut cache = self.rate_cache.write().await;

        if let Some(entry) = cache.get(pair) {
            if !entry.is_expired() {
                return Some(entry.data);
            }
        }

        None
    }

    /// Cache exchange rate
    pub async fn cache_exchange_rate(&self, pair: String, rate: f64) {
        let mut cache = self.rate_cache.write().await;
        let cached = CachedItem::new(rate, self.config.rate_cache_ttl);
        cache.put(pair, cached);
    }

    /// Evict expired swap sessions
    async fn evict_expired_swaps(&self) {
        let expired: Vec<_> = self
            .swap_sessions
            .iter()
            .filter(|entry| entry.value().is_expired())
            .map(|entry| *entry.key())
            .collect();

        for key in expired {
            self.swap_sessions.remove(&key);
            self.stats
                .evictions
                .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        }
    }

    /// Evict expired script results
    async fn evict_expired_scripts(&self) {
        let expired: Vec<_> = self
            .script_results
            .iter()
            .filter(|entry| entry.value().is_expired())
            .map(|entry| entry.key().clone())
            .collect();

        for key in expired {
            self.script_results.remove(&key);
            self.stats
                .evictions
                .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        }
    }

    /// Clear all caches
    pub async fn clear_all(&self) {
        self.swap_sessions.clear();
        self.bitcoin_txs.write().await.clear();
        self.script_results.clear();
        self.rate_cache.write().await.clear();
    }

    /// Get cache statistics
    pub fn stats(&self) -> &CacheStats {
        &self.stats
    }

    /// Get cache hit rate
    pub fn hit_rate(&self) -> f64 {
        let total_hits = self
            .stats
            .swap_hits
            .load(std::sync::atomic::Ordering::Relaxed)
            + self
                .stats
                .btc_hits
                .load(std::sync::atomic::Ordering::Relaxed)
            + self
                .stats
                .script_hits
                .load(std::sync::atomic::Ordering::Relaxed);

        let total_misses = self
            .stats
            .swap_misses
            .load(std::sync::atomic::Ordering::Relaxed)
            + self
                .stats
                .btc_misses
                .load(std::sync::atomic::Ordering::Relaxed)
            + self
                .stats
                .script_misses
                .load(std::sync::atomic::Ordering::Relaxed);

        let total = total_hits + total_misses;
        if total > 0 {
            total_hits as f64 / total as f64
        } else {
            0.0
        }
    }
}

/// Swap state cache for quick lookups
pub struct SwapStateCache {
    states: Arc<DashMap<[u8; 32], (SwapState, Instant)>>,
    ttl: Duration,
}

impl SwapStateCache {
    pub fn new(ttl: Duration) -> Self {
        Self {
            states: Arc::new(DashMap::new()),
            ttl,
        }
    }

    pub fn get(&self, swap_id: &[u8; 32]) -> Option<SwapState> {
        self.states.get(swap_id).and_then(|entry| {
            if entry.1 > Instant::now() {
                Some(entry.0.clone())
            } else {
                None
            }
        })
    }

    pub fn set(&self, swap_id: [u8; 32], state: SwapState) {
        let expires = Instant::now() + self.ttl;
        self.states.insert(swap_id, (state, expires));
    }

    pub fn remove(&self, swap_id: &[u8; 32]) {
        self.states.remove(swap_id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_swap_session_cache() {
        let cache = AtomicSwapCache::new(CacheConfig::default());
        let swap_id = [1u8; 32];

        // Cache miss
        assert!(cache.get_swap_session(&swap_id).await.is_none());
        assert_eq!(
            cache
                .stats
                .swap_misses
                .load(std::sync::atomic::Ordering::Relaxed),
            1
        );

        // Create dummy session
        let session = SwapSession {
            setup: Default::default(),
            secret: Some([42u8; 32]),
            nova_htlc: Default::default(),
            btc_htlc: Default::default(),
            state: SwapState::Active,
            created_at: 0,
            updated_at: 0,
        };

        // Cache it
        cache.cache_swap_session(swap_id, session.clone()).await;

        // Cache hit
        let cached = cache.get_swap_session(&swap_id).await;
        assert!(cached.is_some());
        assert_eq!(
            cache
                .stats
                .swap_hits
                .load(std::sync::atomic::Ordering::Relaxed),
            1
        );
    }

    #[tokio::test]
    async fn test_cache_expiration() {
        let mut config = CacheConfig::default();
        config.swap_session_ttl = Duration::from_millis(100);

        let cache = AtomicSwapCache::new(config);
        let swap_id = [2u8; 32];

        let session = SwapSession {
            setup: Default::default(),
            secret: None,
            nova_htlc: Default::default(),
            btc_htlc: Default::default(),
            state: SwapState::Active,
            created_at: 0,
            updated_at: 0,
        };

        cache.cache_swap_session(swap_id, session).await;

        // Should be cached
        assert!(cache.get_swap_session(&swap_id).await.is_some());

        // Wait for expiration
        tokio::time::sleep(Duration::from_millis(150)).await;

        // Should be expired
        assert!(cache.get_swap_session(&swap_id).await.is_none());
        assert_eq!(
            cache
                .stats
                .evictions
                .load(std::sync::atomic::Ordering::Relaxed),
            1
        );
    }
}
