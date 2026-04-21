//! Fuzz harness: Transaction deserialization + structural inspection.
//!
//! Entry point: bincode-deserialize arbitrary bytes as `Transaction`, then
//! exercise the accessors that drive hashing, witness parsing, and fee /
//! amount arithmetic. Goal: panic-free on any input.

use afl::fuzz;
use supernova_core::types::transaction::Transaction;

fn main() {
    fuzz!(|data: &[u8]| {
        if data.is_empty() {
            return;
        }

        let tx: Transaction = match bincode::deserialize(data) {
            Ok(t) => t,
            Err(_) => return,
        };

        // Exercise hashing + flag paths.
        let _ = tx.hash();
        let _ = tx.version();
        let _ = tx.lock_time();
        let _ = tx.is_coinbase();

        // Walk inputs (witness parsing, prev-output lookup pre-amble).
        for input in tx.inputs() {
            let _ = input.prev_tx_hash();
            let _ = input.prev_output_index();
            let _ = input.script_sig().len();
            let _ = input.sequence();
            let _ = input.has_witness();
            for w in input.witness() {
                let _ = w.len();
            }
        }

        // Walk outputs (amount overflow / scriptPubKey parse pre-amble).
        let mut total: u64 = 0;
        for output in tx.outputs() {
            total = total.saturating_add(output.amount());
            let _ = output.script_pubkey().len();
        }
        let _ = total;

        // Round-trip serialize; any panic here is a finding.
        let _ = bincode::serialize(&tx);
    });
}
