pub mod block;
pub mod transaction;
pub mod extended_transaction;
pub mod units;
pub mod transaction_dependency;

// Re-export main types
pub use block::Block;
pub use transaction::{Transaction, TransactionInput, TransactionOutput, TransactionError};
pub use extended_transaction::ExtendedTransactionInput;
pub use units::{NovaUnit, UnitError};
pub use transaction_dependency::TransactionDependencyGraph; 
