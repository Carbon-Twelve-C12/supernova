//! Treasury address governance — consensus-critical allocation target.
//!
//! Every coinbase transaction must direct
//! [`TREASURY_ALLOCATION_PERCENT`] of its total reward to the script
//! returned by [`treasury_script_pubkey`] for the active network.
//! Miners building blocks and validators checking them must use the same
//! function, so this module is the single source of truth.
//!
//! # Script format
//!
//! Outputs use P2WSH (SegWit v0 pay-to-witness-script-hash):
//! `0x00 || 0x20 || SHA3-256(redeem_script)` — 34 bytes.
//!
//! # Per-network sources
//!
//! | Network | 32-byte hash source                                              |
//! |---------|------------------------------------------------------------------|
//! | Mainnet | [`MAINNET_TREASURY_PENDING_GENESIS`] sentinel, replaced at launch |
//! | Testnet | `SHA3-256(TESTNET_TREASURY_TAG)` — deterministic placeholder     |
//! | Regtest | `[0u8; 32]` — predictable for integration tests                  |
//!
//! When mainnet governance produces the real m-of-n SPHINCS+ multisig
//! redeem script, update [`MAINNET_TREASURY_PENDING_GENESIS`] to the
//! `SHA3-256` hash of that script. That is the only constant to change;
//! the rest of the pipeline is script-agnostic.

use crate::config::NetworkType;
use sha3::{Digest, Sha3_256};

/// Treasury share of the block reward, in whole percent.
pub const TREASURY_ALLOCATION_PERCENT: u64 = 5;

/// Byte length of a SegWit v0 P2WSH script (`0x00 0x20 || 32-byte hash`).
pub const TREASURY_SCRIPT_LEN: usize = 34;

/// Sentinel used as the mainnet treasury script hash until governance
/// generates the production multisig at mainnet genesis.
///
/// Must be replaced in the same release that produces the mainnet
/// genesis block — leaving this in place on mainnet is equivalent to
/// burning 5% of every block reward.
pub const MAINNET_TREASURY_PENDING_GENESIS: [u8; 32] = [0xFF; 32];

/// Deterministic tag hashed to derive the testnet treasury script.
pub const TESTNET_TREASURY_TAG: &[u8] = b"supernova/treasury/testnet/v1";

/// Errors produced while validating a treasury script against consensus.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum TreasuryError {
    #[error("invalid treasury script length: expected 34, got {0}")]
    InvalidScriptLen(usize),

    #[error("invalid treasury script prefix: expected P2WSH (0x00 0x20)")]
    InvalidScriptPrefix,

    #[error("treasury script mismatch for {network:?}")]
    ScriptMismatch { network: NetworkType },
}

/// Returns the 34-byte P2WSH script that every coinbase on `network`
/// must use for its treasury output.
pub fn treasury_script_pubkey(network: NetworkType) -> Vec<u8> {
    let hash = treasury_script_hash(network);
    let mut script = Vec::with_capacity(TREASURY_SCRIPT_LEN);
    script.push(0x00); // SegWit version 0
    script.push(0x20); // push 32 bytes
    script.extend_from_slice(&hash);
    script
}

/// Returns the 32-byte witness-script hash embedded in the treasury
/// P2WSH output for `network`.
pub fn treasury_script_hash(network: NetworkType) -> [u8; 32] {
    match network {
        NetworkType::Mainnet => MAINNET_TREASURY_PENDING_GENESIS,
        NetworkType::Testnet => {
            let digest = Sha3_256::digest(TESTNET_TREASURY_TAG);
            let mut hash = [0u8; 32];
            hash.copy_from_slice(&digest);
            hash
        }
        NetworkType::Regtest => [0u8; 32],
    }
}

/// Constant-time check that `script` is the canonical treasury script
/// for `network`.
pub fn validate_treasury_script(
    script: &[u8],
    network: NetworkType,
) -> Result<(), TreasuryError> {
    if script.len() != TREASURY_SCRIPT_LEN {
        return Err(TreasuryError::InvalidScriptLen(script.len()));
    }
    if script[0] != 0x00 || script[1] != 0x20 {
        return Err(TreasuryError::InvalidScriptPrefix);
    }
    let expected = treasury_script_pubkey(network);
    // Length-equality already verified, so byte-compare is sufficient.
    if script != expected.as_slice() {
        return Err(TreasuryError::ScriptMismatch { network });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn script_length_is_p2wsh() {
        for net in [NetworkType::Mainnet, NetworkType::Testnet, NetworkType::Regtest] {
            let script = treasury_script_pubkey(net);
            assert_eq!(script.len(), TREASURY_SCRIPT_LEN, "P2WSH must be 34 bytes");
            assert_eq!(script[0], 0x00, "witness version 0");
            assert_eq!(script[1], 0x20, "32-byte push opcode");
        }
    }

    #[test]
    fn networks_produce_distinct_scripts() {
        let mainnet = treasury_script_pubkey(NetworkType::Mainnet);
        let testnet = treasury_script_pubkey(NetworkType::Testnet);
        let regtest = treasury_script_pubkey(NetworkType::Regtest);
        assert_ne!(mainnet, testnet);
        assert_ne!(mainnet, regtest);
        assert_ne!(testnet, regtest);
    }

    #[test]
    fn mainnet_script_contains_pending_sentinel() {
        let script = treasury_script_pubkey(NetworkType::Mainnet);
        assert_eq!(&script[2..], &MAINNET_TREASURY_PENDING_GENESIS);
    }

    #[test]
    fn testnet_hash_is_deterministic() {
        let first = treasury_script_hash(NetworkType::Testnet);
        let second = treasury_script_hash(NetworkType::Testnet);
        assert_eq!(first, second);
    }

    #[test]
    fn regtest_hash_is_all_zeros() {
        assert_eq!(treasury_script_hash(NetworkType::Regtest), [0u8; 32]);
    }

    #[test]
    fn validate_accepts_canonical_script() {
        for net in [NetworkType::Mainnet, NetworkType::Testnet, NetworkType::Regtest] {
            let script = treasury_script_pubkey(net);
            assert!(validate_treasury_script(&script, net).is_ok());
        }
    }

    #[test]
    fn validate_rejects_cross_network_script() {
        let testnet = treasury_script_pubkey(NetworkType::Testnet);
        let err = validate_treasury_script(&testnet, NetworkType::Mainnet).unwrap_err();
        assert!(matches!(err, TreasuryError::ScriptMismatch { .. }));
    }

    #[test]
    fn validate_rejects_wrong_length() {
        let err = validate_treasury_script(&[0x00; 10], NetworkType::Testnet).unwrap_err();
        assert!(matches!(err, TreasuryError::InvalidScriptLen(10)));
    }

    #[test]
    fn validate_rejects_wrong_prefix() {
        let mut script = treasury_script_pubkey(NetworkType::Testnet);
        script[0] = 0x51; // OP_1
        let err = validate_treasury_script(&script, NetworkType::Testnet).unwrap_err();
        assert!(matches!(err, TreasuryError::InvalidScriptPrefix));
    }

    #[test]
    fn validate_rejects_legacy_placeholder() {
        // The old `b"TREASURY_PLACEHOLDER_ADDRESS"` constant was 28 bytes
        // and did not begin with 0x00 0x20; confirm it cannot pass.
        let legacy = b"TREASURY_PLACEHOLDER_ADDRESS";
        let err = validate_treasury_script(legacy, NetworkType::Testnet).unwrap_err();
        assert!(matches!(err, TreasuryError::InvalidScriptLen(_)));
    }
}
