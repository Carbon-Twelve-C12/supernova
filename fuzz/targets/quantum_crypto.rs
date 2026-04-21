//! Fuzz harness: ML-DSA (Dilithium) signature parse + verify.
//!
//! Strategy:
//!   - Split fuzzer input into (level_tag, pk_bytes, sig_bytes, message)
//!   - Construct an `MLDSAPublicKey` directly from bytes (no key gen — we
//!     want to fuzz the verify path against adversarial inputs, not force
//!     deterministic key gen paths)
//!   - Call `verify`; any panic is a finding
//!
//! Additional entry point: bincode-deserialize `MLDSAPublicKey` and
//! `MLDSASignature` from fuzzer bytes to exercise Serde paths.

use afl::fuzz;
use supernova_core::crypto::quantum::{
    MLDSAPublicKey, MLDSASecurityLevel, MLDSASignature,
};

fn main() {
    fuzz!(|data: &[u8]| {
        if data.len() < 2 {
            return;
        }

        // --- Entry 1: Serde deserialize ---
        //
        // Malformed input usually fails deserialize; that's fine.
        let _ = bincode::deserialize::<MLDSAPublicKey>(data);
        let _ = bincode::deserialize::<MLDSASignature>(data);

        // --- Entry 2: direct verify ---
        //
        // First byte selects security level; the rest is partitioned into
        // (pk, sig, message). Lengths are driven by the input.
        let level = match data[0] % 3 {
            0 => MLDSASecurityLevel::Level2,
            1 => MLDSASecurityLevel::Level3,
            _ => MLDSASecurityLevel::Level5,
        };

        let body = &data[1..];
        if body.len() < 3 {
            return;
        }

        // Pick split points from the next two bytes.
        let split_a = usize::from(body[0]) % body.len().max(1);
        let body = &body[1..];
        if body.is_empty() {
            return;
        }
        let split_b = usize::from(body[0]) % body.len().max(1);
        let body = &body[1..];
        if body.len() < split_a + split_b {
            return;
        }

        let (pk_bytes, rest) = body.split_at(split_a);
        let (sig_bytes, message) = rest.split_at(split_b);

        let pk = MLDSAPublicKey {
            bytes: pk_bytes.to_vec(),
            security_level: level,
        };
        let sig = MLDSASignature {
            bytes: sig_bytes.to_vec(),
        };

        // Verify must never panic.
        let _ = pk.verify(message, &sig);
    });
}
