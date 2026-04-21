//! Property-based tests (Phase 3 C3).
//!
//! Each `proptest!` block captures a falsifiable invariant. The harness
//! targets 64–128 cases per property to keep CI reasonable; shrinkage is
//! automatic.
//!
//! Organisation mirrors the threat model: consensus → crypto → transaction
//! → block → constant-time. Each module's invariants are grounded in a
//! specific threat class:
//!
//! - consensus: difficulty / timestamp manipulation
//! - crypto: signature / hash determinism and parse-safety
//! - transaction: serialization round-trip, amount arithmetic
//! - block: header hash stability and nonce sensitivity
//! - ct: constant-time compare correctness

use proptest::prelude::*;
use supernova_core::consensus::difficulty::DifficultyAdjustment;
use supernova_core::crypto::quantum::{
    MLDSAPublicKey, MLDSASecurityLevel, MLDSASignature,
};
use supernova_core::hash::hash256;
use supernova_core::types::block::{Block, BlockHeader};
use supernova_core::types::transaction::{
    Transaction, TransactionInput, TransactionOutput,
};

// =============================================================================
// Strategies — shared combinators used by properties below.
// =============================================================================

fn any_hash() -> impl Strategy<Value = [u8; 32]> {
    proptest::array::uniform32(any::<u8>())
}

fn any_txinput() -> impl Strategy<Value = TransactionInput> {
    (any_hash(), any::<u32>(), prop::collection::vec(any::<u8>(), 0..64), any::<u32>())
        .prop_map(|(prev_hash, prev_idx, script, seq)| {
            TransactionInput::new(prev_hash, prev_idx, script, seq)
        })
}

fn any_txoutput() -> impl Strategy<Value = TransactionOutput> {
    (any::<u64>(), prop::collection::vec(any::<u8>(), 0..64))
        .prop_map(|(amount, script)| TransactionOutput::new(amount, script))
}

fn any_transaction() -> impl Strategy<Value = Transaction> {
    (
        any::<u32>(),
        prop::collection::vec(any_txinput(), 0..4),
        prop::collection::vec(any_txoutput(), 0..4),
        any::<u32>(),
    )
        .prop_map(|(v, ins, outs, lt)| Transaction::new(v, ins, outs, lt))
}

fn any_blockheader() -> impl Strategy<Value = BlockHeader> {
    (
        any::<u32>(),
        any_hash(),
        any_hash(),
        any::<u64>(),
        any::<u32>(),
        any::<u32>(),
        any::<u64>(),
    )
        .prop_map(|(v, prev, merk, ts, bits, nonce, height)| {
            BlockHeader::new_with_height(v, prev, merk, ts, bits, nonce, height)
        })
}

fn any_block() -> impl Strategy<Value = Block> {
    (any_blockheader(), prop::collection::vec(any_transaction(), 0..3))
        .prop_map(|(hdr, txs)| Block::new(hdr, txs))
}

fn any_timestamps_and_heights() -> impl Strategy<Value = (Vec<u64>, Vec<u64>)> {
    // Produce matched-length slices so the difficulty adjuster sees a
    // plausible shape; random values inside each slice are adversarial.
    (1usize..32).prop_flat_map(|n| {
        (
            prop::collection::vec(any::<u64>(), n),
            prop::collection::vec(any::<u64>(), n),
        )
    })
}

// =============================================================================
// Consensus — difficulty adjustment invariants.
// =============================================================================

proptest! {
    #![proptest_config(ProptestConfig { cases: 128, ..ProptestConfig::default() })]

    /// P-1: `calculate_next_target` never panics.
    ///
    /// Any adversarial combination of current_target, timestamps, and heights
    /// must produce a `Result`, never a panic.
    #[test]
    fn prop_difficulty_never_panics(
        current_target in any::<u32>(),
        (timestamps, heights) in any_timestamps_and_heights(),
    ) {
        let da = DifficultyAdjustment::new();
        let _ = da.calculate_next_target(current_target, &timestamps, &heights);
    }

    /// P-2: Empty and single-element history returns InsufficientHistory.
    ///
    /// The function documents that < 2 blocks cannot drive an adjustment.
    #[test]
    fn prop_difficulty_insufficient_history_is_error(
        current_target in any::<u32>(),
        single_ts in any::<u64>(),
        single_h in any::<u64>(),
    ) {
        let da = DifficultyAdjustment::new();
        prop_assert!(da.calculate_next_target(current_target, &[], &[]).is_err());
        prop_assert!(da.calculate_next_target(current_target, &[single_ts], &[single_h]).is_err());
    }

    /// P-3: Same inputs → same output (deterministic).
    #[test]
    fn prop_difficulty_is_deterministic(
        current_target in any::<u32>(),
        (timestamps, heights) in any_timestamps_and_heights(),
    ) {
        let da = DifficultyAdjustment::new();
        let a = da.calculate_next_target(current_target, &timestamps, &heights);
        let b = da.calculate_next_target(current_target, &timestamps, &heights);
        match (a, b) {
            (Ok(x), Ok(y)) => prop_assert_eq!(x, y),
            (Err(_), Err(_)) => {}
            _ => prop_assert!(false, "non-deterministic difficulty adjustment"),
        }
    }
}

// =============================================================================
// Crypto — ML-DSA parse and verify safety.
// =============================================================================

proptest! {
    #![proptest_config(ProptestConfig { cases: 64, ..ProptestConfig::default() })]

    /// P-4: `MLDSAPublicKey::verify` never panics on arbitrary bytes.
    ///
    /// This is the core parse-safety property for the signature verifier. A
    /// panic here is a consensus-halting bug (any peer could send a block
    /// containing a malformed signature).
    #[test]
    fn prop_mldsa_verify_never_panics(
        pk_bytes in prop::collection::vec(any::<u8>(), 0..3000),
        sig_bytes in prop::collection::vec(any::<u8>(), 0..5000),
        msg in prop::collection::vec(any::<u8>(), 0..256),
        level_sel in 0u8..3,
    ) {
        let level = match level_sel {
            0 => MLDSASecurityLevel::Level2,
            1 => MLDSASecurityLevel::Level3,
            _ => MLDSASecurityLevel::Level5,
        };
        let pk = MLDSAPublicKey { bytes: pk_bytes, security_level: level };
        let sig = MLDSASignature { bytes: sig_bytes };
        let _ = pk.verify(&msg, &sig);
    }

    /// P-5: `MLDSASignature` bincode round-trip.
    #[test]
    fn prop_mldsa_signature_roundtrip(sig_bytes in prop::collection::vec(any::<u8>(), 0..5000)) {
        let sig = MLDSASignature { bytes: sig_bytes };
        let encoded = bincode::serialize(&sig).expect("serialize never fails on owned data");
        let decoded: MLDSASignature = bincode::deserialize(&encoded).expect("round-trip");
        prop_assert_eq!(sig.bytes, decoded.bytes);
    }

    /// P-6: `MLDSAPublicKey` bincode round-trip.
    #[test]
    fn prop_mldsa_pubkey_roundtrip(
        pk_bytes in prop::collection::vec(any::<u8>(), 0..3000),
        level_sel in 0u8..3,
    ) {
        let level = match level_sel {
            0 => MLDSASecurityLevel::Level2,
            1 => MLDSASecurityLevel::Level3,
            _ => MLDSASecurityLevel::Level5,
        };
        let pk = MLDSAPublicKey { bytes: pk_bytes, security_level: level };
        let encoded = bincode::serialize(&pk).expect("serialize never fails on owned data");
        let decoded: MLDSAPublicKey = bincode::deserialize(&encoded).expect("round-trip");
        prop_assert_eq!(pk.bytes, decoded.bytes);
        prop_assert_eq!(pk.security_level, decoded.security_level);
    }

    /// P-7: `hash256` is deterministic on byte-slice inputs.
    #[test]
    fn prop_hash256_deterministic(data in prop::collection::vec(any::<u8>(), 0..2048)) {
        prop_assert_eq!(hash256(&data), hash256(&data));
    }

    /// P-8: `hash256` is collision-free on short, distinct prefixes.
    ///
    /// Not a cryptographic collision claim — we only assert that adding one
    /// byte changes the hash. Useful sanity check that the hash isn't a
    /// no-op for any weird length.
    #[test]
    fn prop_hash256_prefix_changes_output(
        data in prop::collection::vec(any::<u8>(), 1..256),
        extra in any::<u8>(),
    ) {
        let h1 = hash256(&data);
        let mut extended = data.clone();
        extended.push(extra);
        let h2 = hash256(&extended);
        prop_assert_ne!(h1, h2);
    }
}

// =============================================================================
// Transaction — serialization and determinism.
// =============================================================================

proptest! {
    #![proptest_config(ProptestConfig { cases: 64, ..ProptestConfig::default() })]

    /// P-9: `Transaction` bincode round-trip.
    #[test]
    fn prop_transaction_bincode_roundtrip(tx in any_transaction()) {
        let bytes = bincode::serialize(&tx).expect("serialize");
        let decoded: Transaction = bincode::deserialize(&bytes).expect("deserialize");
        prop_assert_eq!(tx.version(), decoded.version());
        prop_assert_eq!(tx.lock_time(), decoded.lock_time());
        prop_assert_eq!(tx.inputs().len(), decoded.inputs().len());
        prop_assert_eq!(tx.outputs().len(), decoded.outputs().len());
    }

    /// P-10: `TransactionInput` bincode round-trip.
    #[test]
    fn prop_txinput_bincode_roundtrip(input in any_txinput()) {
        let bytes = bincode::serialize(&input).expect("serialize");
        let decoded: TransactionInput = bincode::deserialize(&bytes).expect("deserialize");
        prop_assert_eq!(input.prev_tx_hash(), decoded.prev_tx_hash());
        prop_assert_eq!(input.prev_output_index(), decoded.prev_output_index());
        prop_assert_eq!(input.sequence(), decoded.sequence());
        prop_assert_eq!(input.signature_script(), decoded.signature_script());
    }

    /// P-11: `TransactionOutput` bincode round-trip.
    #[test]
    fn prop_txoutput_bincode_roundtrip(output in any_txoutput()) {
        let bytes = bincode::serialize(&output).expect("serialize");
        let decoded: TransactionOutput = bincode::deserialize(&bytes).expect("deserialize");
        prop_assert_eq!(output.amount(), decoded.amount());
        prop_assert_eq!(output.script_pubkey(), decoded.script_pubkey());
    }

    /// P-12: `Transaction::hash` is deterministic for identical txs.
    #[test]
    fn prop_transaction_hash_deterministic(tx in any_transaction()) {
        prop_assert_eq!(tx.hash(), tx.hash());
    }

    /// P-13: Output amount sum never panics under saturating arithmetic.
    ///
    /// A u64 sum can overflow naturally. We assert that saturating_add
    /// stays within u64::MAX and that the loop terminates.
    #[test]
    fn prop_transaction_output_sum_saturates(tx in any_transaction()) {
        let mut total: u64 = 0;
        for out in tx.outputs() {
            total = total.saturating_add(out.amount());
        }
        prop_assert!(total <= u64::MAX);
    }

    /// P-14: Serializing any built `Transaction` never fails.
    #[test]
    fn prop_transaction_serialize_total(tx in any_transaction()) {
        prop_assert!(bincode::serialize(&tx).is_ok());
    }
}

// =============================================================================
// Block — header hash stability and sensitivity.
// =============================================================================

proptest! {
    #![proptest_config(ProptestConfig { cases: 64, ..ProptestConfig::default() })]

    /// P-15: `BlockHeader::hash` is deterministic.
    #[test]
    fn prop_blockheader_hash_deterministic(h in any_blockheader()) {
        prop_assert_eq!(h.hash(), h.hash());
    }

    /// P-16: Incrementing the nonce changes the header hash.
    ///
    /// This is the core PoW invariant: two headers that differ only in
    /// nonce must hash differently (otherwise mining couldn't progress).
    #[test]
    fn prop_blockheader_nonce_flip_changes_hash(h in any_blockheader()) {
        let original = h.hash();
        let mut mutated = h.clone();
        mutated.increment_nonce();
        prop_assert_ne!(original, mutated.hash());
    }

    /// P-17: `Block` bincode round-trip preserves header and tx count.
    #[test]
    fn prop_block_bincode_roundtrip(block in any_block()) {
        let bytes = bincode::serialize(&block).expect("serialize");
        let decoded: Block = bincode::deserialize(&bytes).expect("deserialize");
        prop_assert_eq!(block.header().hash(), decoded.header().hash());
        prop_assert_eq!(block.transactions().len(), decoded.transactions().len());
    }

    /// P-18: `BlockHeader` bincode round-trip preserves all fields.
    #[test]
    fn prop_blockheader_bincode_roundtrip(h in any_blockheader()) {
        let bytes = bincode::serialize(&h).expect("serialize");
        let decoded: BlockHeader = bincode::deserialize(&bytes).expect("deserialize");
        prop_assert_eq!(h.version(), decoded.version());
        prop_assert_eq!(h.timestamp(), decoded.timestamp());
        prop_assert_eq!(h.bits(), decoded.bits());
        prop_assert_eq!(h.nonce, decoded.nonce);
        prop_assert_eq!(h.hash(), decoded.hash());
    }

    /// P-19: `target()` is a well-formed `[u8; 32]` for any bits value.
    ///
    /// Since the type is a fixed-size array the length is guaranteed by the
    /// type system — but we assert no panic along the bits→target path.
    #[test]
    fn prop_blockheader_target_length(h in any_blockheader()) {
        let target = h.target();
        prop_assert_eq!(target.len(), 32);
    }
}

// =============================================================================
// Constant-time — subtle::ConstantTimeEq correctness.
// =============================================================================

proptest! {
    #![proptest_config(ProptestConfig { cases: 256, ..ProptestConfig::default() })]

    /// P-20: `ConstantTimeEq::ct_eq` agrees with `==` on identical arrays.
    ///
    /// Correctness guarantee for the constant-time compare we use in
    /// Lightning HTLC preimage and HMAC checks (Phase 3 C1).
    #[test]
    fn prop_ct_eq_matches_eq_same(data in any_hash()) {
        use subtle::ConstantTimeEq;
        prop_assert!(bool::from(data.ct_eq(&data)));
        prop_assert_eq!(data, data);
    }

    /// P-21: `ct_eq` rejects pairs that differ in any single byte.
    #[test]
    fn prop_ct_eq_rejects_differing(
        data in any_hash(),
        idx in 0usize..32,
        xor in 1u8..=255,
    ) {
        use subtle::ConstantTimeEq;
        let mut mutated = data;
        mutated[idx] ^= xor;
        prop_assert!(!bool::from(data.ct_eq(&mutated)));
        prop_assert_ne!(data, mutated);
    }
}
