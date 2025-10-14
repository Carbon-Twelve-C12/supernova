pub mod block;
pub mod coinbase;
pub mod extended_transaction;
pub mod safe_arithmetic;
pub mod transaction;
pub mod transaction_dependency;
pub mod transaction_safe;
pub mod units;

// Export implementations from other modules
pub mod utxo {
    pub use crate::storage::utxo_set::{UtxoEntry, UtxoSet};
}

pub mod difficulty;
pub mod script;
pub mod weight;

// Merkle functionality is implemented in block.rs as part of the Block struct

pub mod hash {
    pub use crate::hash::{double_sha256, hash256, meets_difficulty};
}

#[cfg(test)]
pub mod overflow_tests;
#[cfg(test)]
pub mod quantum_test_vectors;

// Export main types
pub use block::{Block, BlockHeader};
pub use extended_transaction::ExtendedTransactionInput;
pub use safe_arithmetic::{
    calculate_fee_safe, safe_add, safe_div, safe_mul, safe_sub, sum_safe, ArithmeticError,
};
pub use transaction::{Transaction, TransactionError, TransactionInput, TransactionOutput};
pub use transaction_dependency::TransactionDependencyGraph;
pub use transaction_safe::TransactionSafe;
pub use units::UnitError;
pub use units::{Amount, Attonovas, FeeRate, NovaUnit, ATTONOVAS_PER_NOVA, NOVAS_PER_NOVA};
pub use utxo::{UtxoEntry, UtxoSet};
