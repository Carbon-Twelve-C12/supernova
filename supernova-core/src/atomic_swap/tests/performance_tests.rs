//! Performance benchmarks for atomic swap operations

#[cfg(test)]
mod benchmarks {
    use super::super::test_utils::*;
    use crate::atomic_swap::{htlc::*, crypto::*, monitor::*};
    use std::time::Instant;

    #[test]
    fn bench_htlc_creation() {
        let alice = TestParticipant::new("alice");
        let bob = TestParticipant::new("bob");

        let start = Instant::now();
        for _ in 0..100 {
            let _ = create_test_htlc(&alice.info, &bob.info, 1_000_000);
        }
        let duration = start.elapsed();

        println!("HTLC creation (100 iterations): {:?}", duration);
        assert!(duration.as_millis() < 1000); // Should be fast
    }

    #[test]
    fn bench_hash_generation() {
        use crate::atomic_swap::crypto::{HashFunction, HashLock};

        let start = Instant::now();
        for _ in 0..1000 {
            let _ = HashLock::new(HashFunction::SHA256).unwrap();
        }
        let duration = start.elapsed();

        println!("Hash generation (1000 iterations): {:?}", duration);
        assert!(duration.as_millis() < 100);
    }

    #[test]
    fn bench_signature_verification() {
        let alice = TestParticipant::new("alice");
        let bob = TestParticipant::new("bob");

        let mut htlc = create_test_htlc(&alice.info, &bob.info, 1_000_000);
        let preimage = htlc.hash_lock.preimage.unwrap();
        let msg = htlc.create_claim_message(&preimage).unwrap();
        let sig = bob.private_key.sign(&msg);

        let start = Instant::now();
        for _ in 0..100 {
            let _ = htlc.participant.pubkey.verify(&msg, &sig);
        }
        let duration = start.elapsed();

        println!("Signature verification (100 iterations): {:?}", duration);
    }
}