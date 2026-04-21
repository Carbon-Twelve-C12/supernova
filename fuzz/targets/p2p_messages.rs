//! Fuzz harness: quantum P2P wire-format parsing.
//!
//! Each of the three quantum-P2P wire types — `QuantumHandshake`,
//! `QuantumMessage`, and `QuantumPeerInfo` — derives Serde. The goal is to
//! confirm that bincode deserialization on adversarial bytes never panics,
//! and that accessing the public fields on a (maybe-malformed) result does
//! not panic either.

use afl::fuzz;
use supernova_core::network::quantum_p2p::{
    QuantumHandshake, QuantumMessage, QuantumPeerInfo,
};

fn main() {
    fuzz!(|data: &[u8]| {
        if data.is_empty() {
            return;
        }

        // Dispatch on the first byte so corpus entries can steer the fuzzer
        // to a specific variant.
        match data[0] % 3 {
            0 => {
                if let Ok(msg) = bincode::deserialize::<QuantumHandshake>(&data[1..]) {
                    let _ = msg.version;
                    let _ = msg.quantum_pubkey.len();
                    let _ = msg.kem_pubkey.len();
                    let _ = msg.supported_schemes.len();
                    let _ = msg.timestamp;
                    let _ = msg.signature.len();
                    let _ = msg.classical_signature.as_ref().map(Vec::len);
                    let _ = bincode::serialize(&msg);
                }
            }
            1 => {
                if let Ok(msg) = bincode::deserialize::<QuantumMessage>(&data[1..]) {
                    let _ = msg.id;
                    let _ = msg.encrypted_key.len();
                    let _ = msg.ciphertext.len();
                    let _ = msg.signature.len();
                    let _ = msg.timestamp;
                    let _ = bincode::serialize(&msg);
                }
            }
            _ => {
                if let Ok(msg) = bincode::deserialize::<QuantumPeerInfo>(&data[1..]) {
                    let _ = msg.quantum_pubkey.len();
                    let _ = msg.kem_pubkey.len();
                    let _ = msg.supported_schemes.len();
                    let _ = msg.key_rotation;
                    let _ = msg.security_level;
                    let _ = bincode::serialize(&msg);
                }
            }
        }
    });
}
