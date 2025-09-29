//! Transaction priority for mempool ordering

use std::cmp::Ordering;

/// Transaction priority for mempool ordering
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TransactionPriority {
    /// Fee rate in novas per byte (1 NOVA = 100,000,000 novas)
    pub fee_rate: u64,
    /// Transaction age (older = higher priority)
    pub age: u64,
    /// Size in bytes
    pub size: usize,
}

impl TransactionPriority {
    /// Create new transaction priority
    pub fn new(fee_rate: u64, age: u64, size: usize) -> Self {
        Self {
            fee_rate,
            age,
            size,
        }
    }

    /// Calculate priority score
    pub fn score(&self) -> u64 {
        // Higher fee rate = higher priority
        // Older transactions get slight boost
        self.fee_rate * 1000 + self.age
    }
}

impl Ord for TransactionPriority {
    fn cmp(&self, other: &Self) -> Ordering {
        // Compare by score (higher score = higher priority)
        self.score().cmp(&other.score())
    }
}

impl PartialOrd for TransactionPriority {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
