//! Atomic-swap refund-flow trait demonstration.
//!
//! Walks through the full Phase 3 C4 refund pipeline end-to-end against
//! the public API surface introduced this session:
//!
//! 1. Build an unsigned refund tx via `SupernovaHTLC::build_refund_transaction`
//! 2. Produce a signature via the `RefundSigner` trait (mock impl here)
//! 3. Embed the signature in the input's `signature_script`
//! 4. Broadcast via the `RefundBroadcaster` trait (mock impl here)
//!
//! Mocks stand in for the wallet/network adapters that will live in the
//! `node` crate. They print each step rather than touching real keys or
//! peers, so this example is safe to run on a developer machine without
//! any infrastructure.
//!
//! Run with:
//!     cargo run --example refund_flow_demo --features atomic-swap

extern crate supernova_core as btclib;

use async_trait::async_trait;
use std::sync::Arc;

use btclib::atomic_swap::crypto::{HashFunction, HashLock};
use btclib::atomic_swap::{
    FeeStructure, HTLCState, ParticipantInfo, RefundBroadcastError, RefundBroadcaster,
    RefundSigner, RefundSignerError, SupernovaHTLC, TimeLock,
};
use btclib::crypto::{MLDSAPublicKey, MLDSASignature};
use btclib::types::{Transaction, TransactionInput};

/// Mock signer that returns a deterministic 64-byte "signature" so the
/// demo is reproducible. A real implementation routes the message
/// through the wallet's quantum-key signing pipeline.
struct MockSigner;

#[async_trait]
impl RefundSigner for MockSigner {
    async fn sign_refund(
        &self,
        htlc: &SupernovaHTLC,
        message: &[u8],
    ) -> Result<MLDSASignature, RefundSignerError> {
        println!(
            "  [signer] message bytes ({}): {}",
            message.len(),
            String::from_utf8_lossy(message)
        );
        // Deterministic mock signature: 64 bytes of (htlc_id[0] XOR i).
        let mut bytes = vec![0u8; 64];
        let xor_byte = htlc.htlc_id[0];
        for (i, b) in bytes.iter_mut().enumerate() {
            *b = (i as u8) ^ xor_byte;
        }
        Ok(MLDSASignature { bytes })
    }
}

/// Mock broadcaster that just prints what it would send. A real
/// implementation submits the tx via the node's `NetworkProxy`.
struct MockBroadcaster;

#[async_trait]
impl RefundBroadcaster for MockBroadcaster {
    async fn broadcast_refund(
        &self,
        tx: &Transaction,
    ) -> Result<(), RefundBroadcastError> {
        let txid = tx.hash();
        println!(
            "  [broadcaster] would submit tx {} ({} input{}, {} output{})",
            hex::encode(txid),
            tx.inputs().len(),
            if tx.inputs().len() == 1 { "" } else { "s" },
            tx.outputs().len(),
            if tx.outputs().len() == 1 { "" } else { "s" },
        );
        let sig_len = tx.inputs()[0].signature_script().len();
        println!("  [broadcaster] input[0].signature_script: {} bytes", sig_len);
        Ok(())
    }
}

/// Build an HTLC literally — `SupernovaHTLC::new` would validate
/// timeouts against wall-clock time, which makes demos non-reproducible.
fn make_demo_htlc() -> SupernovaHTLC {
    SupernovaHTLC {
        htlc_id: [0xCD; 32],
        initiator: ParticipantInfo {
            pubkey: MLDSAPublicKey::default(),
            address: "nova1qinitiator_demo_addr".to_string(),
            refund_address: None,
        },
        participant: ParticipantInfo {
            pubkey: MLDSAPublicKey::default(),
            address: "nova1qparticipant_demo_addr".to_string(),
            refund_address: None,
        },
        hash_lock: HashLock {
            hash_type: HashFunction::SHA256,
            hash_value: [0x42; 32],
            preimage: None,
        },
        time_lock: TimeLock {
            absolute_timeout: 4_000_000_000,
            relative_timeout: 144,
            grace_period: 6,
        },
        amount: 100_000_000,
        fee_structure: FeeStructure {
            claim_fee: 1000,
            refund_fee: 1000,
            service_fee: None,
        },
        state: HTLCState::Funded,
        created_at: 0,
        bitcoin_tx_ref: None,
        memo: None,
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let htlc = make_demo_htlc();
    let funding_txid = [0xAB; 32];
    let funding_vout = 0;

    println!("== Atomic-swap refund flow demo ==");
    println!(
        "HTLC: id={} amount={} refund_fee={}",
        hex::encode(htlc.htlc_id),
        htlc.amount,
        htlc.fee_structure.refund_fee
    );

    // 1. Build the unsigned refund tx.
    let unsigned_tx = htlc.build_refund_transaction(funding_txid, funding_vout)?;
    let unsigned_txid = hex::encode(unsigned_tx.hash());
    println!("\n[1] build_refund_transaction:");
    println!("  unsigned txid:    {}", unsigned_txid);
    println!("  refund amount:    {}", unsigned_tx.outputs()[0].value());
    println!("  funding outpoint: {}:{}", hex::encode(funding_txid), funding_vout);

    // 2. Sign the canonical refund message.
    let signer: Arc<dyn RefundSigner> = Arc::new(MockSigner);
    let message = htlc.create_refund_message()?;
    println!("\n[2] sign_refund:");
    let signature = signer.sign_refund(&htlc, &message).await?;
    println!("  signature bytes:  {}", signature.bytes.len());

    // 3. Rebuild the tx with the signature embedded — same logic as
    //    `AtomicSwapRpcImpl::refund_swap` does internally.
    let original = &unsigned_tx.inputs()[0];
    let signed_input = TransactionInput::new(
        original.prev_tx_hash(),
        original.prev_output_index(),
        signature.bytes.clone(),
        original.sequence(),
    );
    let signed_tx = Transaction::new(
        unsigned_tx.version(),
        vec![signed_input],
        unsigned_tx.outputs().to_vec(),
        unsigned_tx.lock_time(),
    );
    let signed_txid = hex::encode(signed_tx.hash());
    println!("\n[3] embed signature:");
    println!("  signed txid:      {}", signed_txid);
    assert_ne!(
        unsigned_txid, signed_txid,
        "embedding the signature must change the txid"
    );

    // 4. Broadcast.
    let broadcaster: Arc<dyn RefundBroadcaster> = Arc::new(MockBroadcaster);
    println!("\n[4] broadcast_refund:");
    broadcaster.broadcast_refund(&signed_tx).await?;

    println!("\n== Demo complete ==");
    Ok(())
}
