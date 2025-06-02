// Transaction module
// Re-exports from types::transaction for backwards compatibility

pub use crate::types::transaction::{
    Transaction,
    TransactionInput,
    TransactionOutput,
    TransactionError,
    OutPoint,
    TxIn,
    TxOut,
}; 