//! Heap-allocation profiler for the mempool-admission hot path.
//!
//! Runs the same structural validation the mempool runs on every
//! incoming transaction, in a tight loop, with the `dhat` global
//! allocator installed. `dhat` writes a `dhat-heap.json` profile to the
//! working directory on drop; open it with the dhat viewer at
//! <https://nnethercote.github.io/dh_view/dh_view.html> to see peak
//! allocation, total allocation, and per-callsite breakdown.
//!
//! This is a *first-principles* budget probe, not a full-node trace.
//! The node/miner/wallet budgets in
//! `docs/operations/PERFORMANCE_TUNING.md` are derived from:
//!
//!   - the UTXO cache limit (configurable, default 1 GiB),
//!   - the mempool byte-size cap (configurable, default 300 MiB),
//!   - the peer-buffer footprint (bounded by message-size limits),
//!   - the per-transaction admission cost measured here.
//!
//! Run:
//!
//! ```
//! cargo run --release --example memory_profile \
//!     --features dhat-heap -- --tx-count 10000
//! ```
//!
//! A subsequent track (E4 follow-up) should wire this into CI so a
//! regression in per-tx allocation shows up as a ratio change against
//! a pinned baseline profile.

use std::env;

use supernova_core::types::transaction::{
    Transaction, TransactionInput, TransactionOutput,
};
use supernova_core::validation::validate_transaction;

#[global_allocator]
static ALLOC: dhat::Alloc = dhat::Alloc;

/// One-input / two-output transaction, same shape as the criterion
/// harness fixture. Seed varies so inputs aren't bit-identical and
/// allocator pooling can't hide real costs.
fn synthetic_tx(seed: u8) -> Transaction {
    let input = TransactionInput::new([seed; 32], 0, vec![seed; 64], 0xffffffff);
    let payout = TransactionOutput::new(1_000_000, vec![seed; 32]);
    let change = TransactionOutput::new(900_000, vec![seed; 32]);
    Transaction::new(1, vec![input], vec![payout, change], 0)
}

fn parse_tx_count() -> usize {
    // Look for `--tx-count N` or default to 10 000. Deliberately
    // argument-light: the heavy lifting is in the dhat profile, not
    // command-line ergonomics.
    let mut args = env::args().skip(1);
    while let Some(flag) = args.next() {
        if flag == "--tx-count" {
            if let Some(val) = args.next() {
                if let Ok(n) = val.parse::<usize>() {
                    return n;
                }
            }
        }
    }
    10_000
}

fn main() {
    let tx_count = parse_tx_count();

    // Profiler stays alive until end of `main`. Dropping it emits the
    // JSON report.
    let _profiler = dhat::Profiler::new_heap();

    let mut accepted = 0usize;
    for i in 0..tx_count {
        let tx = synthetic_tx((i % 255) as u8);
        if validate_transaction(&tx).is_ok() {
            accepted += 1;
        }
    }

    println!(
        "memory_profile: ran {tx_count} validations, {accepted} accepted. \
         dhat-heap.json written to cwd."
    );
}
