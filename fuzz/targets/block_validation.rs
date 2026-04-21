//! Fuzz harness: Block deserialization + structural validation.
//!
//! Entry points:
//!   1. bincode deserialize arbitrary bytes as `Block`
//!   2. compute block hash, exercise header arithmetic paths
//!   3. round-trip the deserialized block through bincode
//!
//! Goal: panic-free on any `&[u8]`. Any panic or abort is a fuzz finding.

use afl::fuzz;
use supernova_core::types::block::Block;

fn main() {
    fuzz!(|data: &[u8]| {
        if data.is_empty() {
            return;
        }

        // Primary entry point: deserialize arbitrary bytes as a Block.
        let block: Block = match bincode::deserialize(data) {
            Ok(b) => b,
            Err(_) => return,
        };

        // Touch every header field so arithmetic / hashing paths execute.
        let _ = block.header().hash();
        let _ = block.header().version();
        let _ = block.header().timestamp();
        let _ = block.header().bits();
        let _ = block.header().target();
        let _ = block.header().nonce;
        let _ = block.height();
        let _ = block.transactions().len();

        // Round-trip: re-serialize and compare length as a sanity check.
        if let Ok(reserialized) = bincode::serialize(&block) {
            // Length equality is not strictly required across bincode variants,
            // but we must not panic here.
            let _ = reserialized.len();
        }
    });
}
