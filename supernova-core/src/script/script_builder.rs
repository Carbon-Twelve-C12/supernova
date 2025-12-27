//! Script Builder for Supernova
//!
//! This module provides utilities for building valid scripts.

use crate::script::opcodes::Opcode;
use ripemd::{Digest as RipemdDigest, Ripemd160};
use sha2::Sha256;
use thiserror::Error;

/// Errors that can occur when building scripts
#[derive(Error, Debug, Clone, PartialEq, Eq)]
pub enum ScriptBuilderError {
    /// Invalid pubkey hash length (expected 20 bytes)
    #[error("Invalid pubkey hash length: expected 20 bytes, got {0}")]
    InvalidPubkeyHashLength(usize),

    /// Invalid script hash length (expected 20 or 32 bytes)
    #[error("Invalid script hash length: expected {expected} bytes, got {actual}")]
    InvalidScriptHashLength { expected: usize, actual: usize },

    /// Invalid multisig threshold
    #[error("Invalid multisig threshold: {threshold} must be > 0 and <= {pubkey_count}")]
    InvalidMultisigThreshold { threshold: u8, pubkey_count: usize },

    /// Too many pubkeys for multisig
    #[error("Too many pubkeys for multisig: {0} (max 16)")]
    TooManyPubkeys(usize),
}

/// Script builder
#[derive(Debug, Clone)]
pub struct ScriptBuilder {
    script: Vec<u8>,
}

impl Default for ScriptBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl ScriptBuilder {
    /// Create a new empty script builder
    pub fn new() -> Self {
        Self { script: Vec::new() }
    }

    /// Push an opcode
    pub fn push_opcode(mut self, opcode: Opcode) -> Self {
        self.script.push(opcode.to_byte());
        self
    }

    /// Push data
    pub fn push_data(mut self, data: &[u8]) -> Self {
        let len = data.len();

        if len <= 75 {
            // Direct push
            self.script.push(len as u8);
            self.script.extend_from_slice(data);
        } else if len <= 255 {
            // OP_PUSHDATA1
            self.script.push(Opcode::OP_PUSHDATA1.to_byte());
            self.script.push(len as u8);
            self.script.extend_from_slice(data);
        } else if len <= 65535 {
            // OP_PUSHDATA2
            self.script.push(Opcode::OP_PUSHDATA2.to_byte());
            self.script.extend_from_slice(&(len as u16).to_le_bytes());
            self.script.extend_from_slice(data);
        } else {
            // OP_PUSHDATA4
            self.script.push(Opcode::OP_PUSHDATA4.to_byte());
            self.script.extend_from_slice(&(len as u32).to_le_bytes());
            self.script.extend_from_slice(data);
        }

        self
    }

    /// Push a number
    pub fn push_number(self, num: i64) -> Self {
        match num {
            -1 => self.push_opcode(Opcode::OP_1NEGATE),
            0 => self.push_opcode(Opcode::OP_0),
            1 => self.push_opcode(Opcode::OP_1),
            2 => self.push_opcode(Opcode::OP_2),
            3 => self.push_opcode(Opcode::OP_3),
            4 => self.push_opcode(Opcode::OP_4),
            5 => self.push_opcode(Opcode::OP_5),
            6 => self.push_opcode(Opcode::OP_6),
            7 => self.push_opcode(Opcode::OP_7),
            8 => self.push_opcode(Opcode::OP_8),
            9 => self.push_opcode(Opcode::OP_9),
            10 => self.push_opcode(Opcode::OP_10),
            11 => self.push_opcode(Opcode::OP_11),
            12 => self.push_opcode(Opcode::OP_12),
            13 => self.push_opcode(Opcode::OP_13),
            14 => self.push_opcode(Opcode::OP_14),
            15 => self.push_opcode(Opcode::OP_15),
            16 => self.push_opcode(Opcode::OP_16),
            _ => {
                // Encode as minimal script number
                let bytes = encode_script_number(num);
                self.push_data(&bytes)
            }
        }
    }

    /// Build the final script
    pub fn build(self) -> Vec<u8> {
        self.script
    }

    /// Create a P2PKH script
    ///
    /// # Errors
    /// Returns `ScriptBuilderError::InvalidPubkeyHashLength` if pubkey_hash is not 20 bytes
    pub fn pay_to_pubkey_hash(pubkey_hash: &[u8]) -> Result<Vec<u8>, ScriptBuilderError> {
        if pubkey_hash.len() != 20 {
            return Err(ScriptBuilderError::InvalidPubkeyHashLength(pubkey_hash.len()));
        }

        Ok(Self::new()
            .push_opcode(Opcode::OP_DUP)
            .push_opcode(Opcode::OP_HASH160)
            .push_data(pubkey_hash)
            .push_opcode(Opcode::OP_EQUALVERIFY)
            .push_opcode(Opcode::OP_CHECKSIG)
            .build())
    }

    /// Create a P2SH script
    ///
    /// # Errors
    /// Returns `ScriptBuilderError::InvalidScriptHashLength` if script_hash is not 20 bytes
    pub fn pay_to_script_hash(script_hash: &[u8]) -> Result<Vec<u8>, ScriptBuilderError> {
        if script_hash.len() != 20 {
            return Err(ScriptBuilderError::InvalidScriptHashLength {
                expected: 20,
                actual: script_hash.len(),
            });
        }

        Ok(Self::new()
            .push_opcode(Opcode::OP_HASH160)
            .push_data(script_hash)
            .push_opcode(Opcode::OP_EQUAL)
            .build())
    }

    /// Create a P2WPKH script
    ///
    /// # Errors
    /// Returns `ScriptBuilderError::InvalidPubkeyHashLength` if pubkey_hash is not 20 bytes
    pub fn pay_to_witness_pubkey_hash(pubkey_hash: &[u8]) -> Result<Vec<u8>, ScriptBuilderError> {
        if pubkey_hash.len() != 20 {
            return Err(ScriptBuilderError::InvalidPubkeyHashLength(pubkey_hash.len()));
        }

        Ok(Self::new()
            .push_opcode(Opcode::OP_0)
            .push_data(pubkey_hash)
            .build())
    }

    /// Create a P2WSH script
    ///
    /// # Errors
    /// Returns `ScriptBuilderError::InvalidScriptHashLength` if script_hash is not 32 bytes
    pub fn pay_to_witness_script_hash(script_hash: &[u8]) -> Result<Vec<u8>, ScriptBuilderError> {
        if script_hash.len() != 32 {
            return Err(ScriptBuilderError::InvalidScriptHashLength {
                expected: 32,
                actual: script_hash.len(),
            });
        }

        Ok(Self::new()
            .push_opcode(Opcode::OP_0)
            .push_data(script_hash)
            .build())
    }

    /// Create a multisig script
    ///
    /// # Errors
    /// - Returns `ScriptBuilderError::InvalidMultisigThreshold` if threshold is 0 or > pubkey count
    /// - Returns `ScriptBuilderError::TooManyPubkeys` if more than 16 pubkeys provided
    pub fn multisig(threshold: u8, pubkeys: &[Vec<u8>]) -> Result<Vec<u8>, ScriptBuilderError> {
        if threshold == 0 || threshold > pubkeys.len() as u8 {
            return Err(ScriptBuilderError::InvalidMultisigThreshold {
                threshold,
                pubkey_count: pubkeys.len(),
            });
        }

        if pubkeys.len() > 16 {
            return Err(ScriptBuilderError::TooManyPubkeys(pubkeys.len()));
        }

        let mut builder = Self::new();

        // Push threshold
        builder = builder.push_number(threshold as i64);

        // Push all pubkeys
        for pubkey in pubkeys {
            builder = builder.push_data(pubkey);
        }

        // Push pubkey count
        builder = builder.push_number(pubkeys.len() as i64);

        // Push CHECKMULTISIG
        Ok(builder.push_opcode(Opcode::OP_CHECKMULTISIG).build())
    }

    /// Hash a public key to get pubkey hash
    pub fn hash_pubkey(pubkey: &[u8]) -> Vec<u8> {
        let mut sha = Sha256::new();
        sha.update(pubkey);
        let sha_result = sha.finalize();

        let mut ripemd = Ripemd160::new();
        ripemd.update(sha_result);
        ripemd.finalize().to_vec()
    }
}

/// Encode a number as a script number
fn encode_script_number(num: i64) -> Vec<u8> {
    if num == 0 {
        return vec![];
    }

    let mut bytes = Vec::new();
    let neg = num < 0;
    let mut abs_num = if neg { -num } else { num } as u64;

    while abs_num > 0 {
        bytes.push((abs_num & 0xff) as u8);
        abs_num >>= 8;
    }

    // If the most significant byte has the high bit set,
    // add an extra byte to indicate sign
    // Safety: bytes is never empty here since we only reach this code when num != 0
    // and the while loop above guarantees at least one byte is pushed
    if let Some(&last_byte) = bytes.last() {
        if last_byte & 0x80 != 0 {
            if neg {
                bytes.push(0x80);
            } else {
                bytes.push(0);
            }
        } else if neg {
            let last = bytes.len() - 1;
            bytes[last] |= 0x80;
        }
    }

    bytes
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_p2pkh_script() {
        let pubkey_hash = vec![0u8; 20];
        let script = ScriptBuilder::pay_to_pubkey_hash(&pubkey_hash).unwrap();

        assert_eq!(script[0], 0x76); // OP_DUP
        assert_eq!(script[1], 0xa9); // OP_HASH160
        assert_eq!(script[2], 0x14); // Push 20 bytes
        assert_eq!(&script[3..23], &pubkey_hash[..]);
        assert_eq!(script[23], 0x88); // OP_EQUALVERIFY
        assert_eq!(script[24], 0xac); // OP_CHECKSIG
    }

    #[test]
    fn test_p2pkh_invalid_length() {
        let pubkey_hash = vec![0u8; 19]; // Wrong length
        let result = ScriptBuilder::pay_to_pubkey_hash(&pubkey_hash);
        assert!(matches!(
            result,
            Err(ScriptBuilderError::InvalidPubkeyHashLength(19))
        ));
    }

    #[test]
    fn test_p2sh_invalid_length() {
        let script_hash = vec![0u8; 21]; // Wrong length
        let result = ScriptBuilder::pay_to_script_hash(&script_hash);
        assert!(matches!(
            result,
            Err(ScriptBuilderError::InvalidScriptHashLength {
                expected: 20,
                actual: 21
            })
        ));
    }

    #[test]
    fn test_p2wsh_invalid_length() {
        let script_hash = vec![0u8; 20]; // Wrong length for P2WSH (needs 32)
        let result = ScriptBuilder::pay_to_witness_script_hash(&script_hash);
        assert!(matches!(
            result,
            Err(ScriptBuilderError::InvalidScriptHashLength {
                expected: 32,
                actual: 20
            })
        ));
    }

    #[test]
    fn test_script_number_encoding() {
        assert_eq!(encode_script_number(0), vec![] as Vec<u8>);
        assert_eq!(encode_script_number(1), vec![0x01]);
        assert_eq!(encode_script_number(-1), vec![0x81]);
        assert_eq!(encode_script_number(127), vec![0x7f]);
        assert_eq!(encode_script_number(128), vec![0x80, 0x00]);
        assert_eq!(encode_script_number(255), vec![0xff, 0x00]);
        assert_eq!(encode_script_number(256), vec![0x00, 0x01]);
    }

    #[test]
    fn test_multisig_script() {
        let pubkey1 = vec![0x02; 33];
        let pubkey2 = vec![0x03; 33];
        let script = ScriptBuilder::multisig(2, &[pubkey1.clone(), pubkey2.clone()]).unwrap();

        assert_eq!(script[0], 0x52); // OP_2
        assert_eq!(script[1], 33); // Push 33 bytes
        assert_eq!(&script[2..35], &pubkey1[..]);
        assert_eq!(script[35], 33); // Push 33 bytes
        assert_eq!(&script[36..69], &pubkey2[..]);
        assert_eq!(script[69], 0x52); // OP_2
        assert_eq!(script[70], 0xae); // OP_CHECKMULTISIG
    }

    #[test]
    fn test_multisig_invalid_threshold_zero() {
        let pubkey1 = vec![0x02; 33];
        let result = ScriptBuilder::multisig(0, &[pubkey1]);
        assert!(matches!(
            result,
            Err(ScriptBuilderError::InvalidMultisigThreshold {
                threshold: 0,
                pubkey_count: 1
            })
        ));
    }

    #[test]
    fn test_multisig_invalid_threshold_too_high() {
        let pubkey1 = vec![0x02; 33];
        let result = ScriptBuilder::multisig(3, &[pubkey1]);
        assert!(matches!(
            result,
            Err(ScriptBuilderError::InvalidMultisigThreshold {
                threshold: 3,
                pubkey_count: 1
            })
        ));
    }

    #[test]
    fn test_multisig_too_many_pubkeys() {
        let pubkeys: Vec<Vec<u8>> = (0..17).map(|i| vec![i as u8; 33]).collect();
        let result = ScriptBuilder::multisig(1, &pubkeys);
        assert!(matches!(
            result,
            Err(ScriptBuilderError::TooManyPubkeys(17))
        ));
    }
}
