pub mod block;
pub mod coinbase;
pub mod transaction;
pub mod units;
pub mod extended_transaction;
pub mod transaction_dependency;
pub mod safe_arithmetic;
pub mod transaction_safe;

// Export implementations from other modules
pub mod utxo {
    pub use crate::storage::utxo_set::{UtxoSet, UtxoEntry};
}

pub mod difficulty;
pub mod script;
pub mod weight;

// Merkle functionality is implemented in block.rs as part of the Block struct

pub mod hash {
    pub use crate::hash::{hash256, double_sha256, meets_difficulty};
}

#[cfg(test)]
pub mod quantum_test_vectors;
#[cfg(test)]
pub mod overflow_tests;

// Export main types
pub use block::Block;
pub use transaction::{Transaction, TransactionInput, TransactionOutput, TransactionError};
pub use extended_transaction::ExtendedTransactionInput;
pub use units::UnitError;
pub use transaction_dependency::TransactionDependencyGraph;
pub use safe_arithmetic::{ArithmeticError, safe_add, safe_sub, safe_mul, safe_div, calculate_fee_safe, sum_safe};
pub use transaction_safe::TransactionSafe;
pub use utxo::{UtxoSet, UtxoEntry};
pub use units::{Amount, FeeRate, NOVAS_PER_NOVA, ATTONOVAS_PER_NOVA, NovaUnit, Attonovas}; 
