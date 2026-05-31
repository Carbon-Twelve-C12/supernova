//! Atomic reorganization change-set (#5).
//!
//! A blockchain reorg must update several DB trees together — block bytes, the
//! UTXO set, chain metadata, and the height->hash index — and a crash or error
//! partway through must leave NONE of them changed. The database's historical
//! `begin_transaction`/`commit_transaction`/`rollback_transaction` primitives
//! were pure no-ops, so the reorg path had no atomicity at all: a mid-reorg
//! failure left a half-unwound UTXO set and partially rewritten metadata.
//!
//! This type captures every tree mutation a reorg needs as a list of
//! owned-byte operations, computed BEFORE any write touches the database.
//! [`crate::storage::database::BlockchainDB::apply_reorg_atomically`] then
//! commits the whole list inside a single sled multi-tree transaction:
//! all-or-nothing. Because the change-set holds only owned bytes (no DB
//! handles, no `&self`), the transaction closure is pure and safe for sled to
//! retry on contention.
//!
//! Key/value bytes are pre-encoded to the EXACT formats the non-transactional
//! helpers use, so existing readers are unaffected:
//! * UTXO key  = `tx_hash || index.to_be_bytes()` (see `create_utxo_key`)
//! * height-index key = `height.to_be_bytes()` (big-endian, 8 bytes)

/// One tree mutation in a reorg change-set.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReorgOp {
    /// Persist a block's bytes under its 32-byte hash (`blocks` tree).
    PutBlock([u8; 32], Vec<u8>),
    /// Insert or overwrite a UTXO (`utxos` tree); key is a pre-encoded utxo key.
    PutUtxo(Vec<u8>, Vec<u8>),
    /// Remove a spent UTXO (`utxos` tree).
    DelUtxo(Vec<u8>),
    /// Insert or overwrite a metadata entry (`metadata` tree).
    PutMeta(Vec<u8>, Vec<u8>),
    /// Map a height (big-endian key) to a block hash (`block_height_index`).
    PutHeightIndex([u8; 8], [u8; 32]),
    /// Remove a height->hash mapping (`block_height_index`).
    DelHeightIndex([u8; 8]),
    /// Test-only: force the committing transaction to abort AFTER earlier ops
    /// have been staged, so the all-or-nothing discard path can be exercised.
    #[cfg(test)]
    AbortForTest,
}

/// An ordered set of tree mutations applied atomically by
/// `BlockchainDB::apply_reorg_atomically`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ReorgChangeSet {
    /// Ops are applied in order inside one transaction.
    pub ops: Vec<ReorgOp>,
}

impl ReorgChangeSet {
    /// A new, empty change-set.
    pub fn new() -> Self {
        Self { ops: Vec::new() }
    }

    /// Stage a block's bytes (used for the new tip; fork ancestors are already
    /// stored).
    pub fn put_block(&mut self, hash: [u8; 32], bytes: Vec<u8>) {
        self.ops.push(ReorgOp::PutBlock(hash, bytes));
    }

    /// Stage a UTXO insertion (key pre-encoded via `create_utxo_key`).
    pub fn put_utxo(&mut self, key: Vec<u8>, value: Vec<u8>) {
        self.ops.push(ReorgOp::PutUtxo(key, value));
    }

    /// Stage a UTXO removal.
    pub fn del_utxo(&mut self, key: Vec<u8>) {
        self.ops.push(ReorgOp::DelUtxo(key));
    }

    /// Stage a metadata write.
    pub fn put_meta(&mut self, key: Vec<u8>, value: Vec<u8>) {
        self.ops.push(ReorgOp::PutMeta(key, value));
    }

    /// Stage a height->hash index write (encodes the height big-endian).
    pub fn put_height_index(&mut self, height: u64, hash: [u8; 32]) {
        self.ops
            .push(ReorgOp::PutHeightIndex(height.to_be_bytes(), hash));
    }

    /// Stage a height->hash index removal (encodes the height big-endian).
    pub fn del_height_index(&mut self, height: u64) {
        self.ops.push(ReorgOp::DelHeightIndex(height.to_be_bytes()));
    }

    /// Number of staged ops.
    pub fn len(&self) -> usize {
        self.ops.len()
    }

    /// Whether the change-set has no ops.
    pub fn is_empty(&self) -> bool {
        self.ops.is_empty()
    }
}
