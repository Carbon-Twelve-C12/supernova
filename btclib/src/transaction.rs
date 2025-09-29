// Transaction module
// Re-exports from types::transaction for backwards compatibility

pub use crate::types::transaction::{
    OutPoint, Transaction, TransactionError, TransactionInput, TransactionOutput, TxIn, TxOut,
};
