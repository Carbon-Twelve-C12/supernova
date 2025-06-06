pub mod block;
pub mod coinbase;
pub mod difficulty;
pub mod hash;
pub mod merkle;
pub mod script;
pub mod taproot;
pub mod transaction;
pub mod utxo;
pub mod weight;
pub mod units;
pub mod extended_transaction;
pub mod transaction_dependency;
pub mod safe_arithmetic;
pub mod transaction_safe;

#[cfg(test)]
pub mod quantum_test_vectors;
#[cfg(test)]
pub mod overflow_tests;

// Re-export main types
pub use block::Block;
pub use transaction::{Transaction, TransactionInput, TransactionOutput, TransactionError};
pub use extended_transaction::ExtendedTransactionInput;
pub use units::{NovaUnit, UnitError};
pub use transaction_dependency::TransactionDependencyGraph;
pub use safe_arithmetic::{ArithmeticError, safe_add, safe_sub, safe_mul, safe_div, calculate_fee_safe, sum_safe};
pub use transaction_safe::TransactionSafe;
pub use utxo::{UtxoSet, UtxoEntry};
pub use units::{Amount, FeeRate, NOVAS_PER_NOVA}; 
