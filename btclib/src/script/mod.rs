//! Script Validation Module for Supernova
//! 
//! This module provides a secure script interpreter that validates
//! transaction scripts according to Supernova's consensus rules.

use thiserror::Error;

pub mod interpreter;
pub mod opcodes;
pub mod script_builder;
pub mod script_validator;

pub use interpreter::{ScriptInterpreter, ScriptError, ExecutionStack};
pub use opcodes::{Opcode, ALL_OPCODES};
pub use script_builder::ScriptBuilder;
pub use script_validator::{ScriptValidator, ScriptFlags};

/// Standard script types supported by Supernova
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScriptType {
    /// Pay to Public Key Hash
    P2PKH,
    /// Pay to Script Hash
    P2SH,
    /// Pay to Witness Public Key Hash
    P2WPKH,
    /// Pay to Witness Script Hash
    P2WSH,
    /// Unknown/non-standard script
    Unknown,
}

/// Script verification errors
#[derive(Debug, Error)]
pub enum ScriptVerificationError {
    #[error("Script execution failed: {0}")]
    ExecutionFailed(String),
    
    #[error("Invalid opcode: {0:02x}")]
    InvalidOpcode(u8),
    
    #[error("Script too large: {0} bytes")]
    ScriptTooLarge(usize),
    
    #[error("Stack size exceeded")]
    StackSizeExceeded,
    
    #[error("Signature verification failed")]
    SignatureVerificationFailed,
    
    #[error("Public key verification failed")]
    PublicKeyVerificationFailed,
    
    #[error("Script hash mismatch")]
    ScriptHashMismatch,
    
    #[error("Witness program mismatch")]
    WitnessProgramMismatch,
    
    #[error("Disabled opcode used: {0}")]
    DisabledOpcode(String),
    
    #[error("Invalid script structure")]
    InvalidScriptStructure,
    
    #[error("Verify operation failed")]
    VerifyFailed,
}

/// Determine the script type from a public key script
pub fn identify_script_type(script: &[u8]) -> ScriptType {
    // P2PKH: OP_DUP OP_HASH160 <20 bytes> OP_EQUALVERIFY OP_CHECKSIG
    if script.len() == 25 
        && script[0] == 0x76  // OP_DUP
        && script[1] == 0xa9  // OP_HASH160
        && script[2] == 0x14  // Push 20 bytes
        && script[23] == 0x88 // OP_EQUALVERIFY
        && script[24] == 0xac // OP_CHECKSIG
    {
        return ScriptType::P2PKH;
    }
    
    // P2SH: OP_HASH160 <20 bytes> OP_EQUAL
    if script.len() == 23
        && script[0] == 0xa9  // OP_HASH160
        && script[1] == 0x14  // Push 20 bytes
        && script[22] == 0x87 // OP_EQUAL
    {
        return ScriptType::P2SH;
    }
    
    // P2WPKH: OP_0 <20 bytes>
    if script.len() == 22
        && script[0] == 0x00  // OP_0
        && script[1] == 0x14  // Push 20 bytes
    {
        return ScriptType::P2WPKH;
    }
    
    // P2WSH: OP_0 <32 bytes>
    if script.len() == 34
        && script[0] == 0x00  // OP_0
        && script[1] == 0x20  // Push 32 bytes
    {
        return ScriptType::P2WSH;
    }
    
    ScriptType::Unknown
}

/// Extract the hash from a standard script
pub fn extract_script_hash(script: &[u8], script_type: ScriptType) -> Option<Vec<u8>> {
    match script_type {
        ScriptType::P2PKH => {
            if script.len() >= 23 {
                Some(script[3..23].to_vec())
            } else {
                None
            }
        },
        ScriptType::P2SH => {
            if script.len() >= 22 {
                Some(script[2..22].to_vec())
            } else {
                None
            }
        },
        ScriptType::P2WPKH => {
            if script.len() >= 22 {
                Some(script[2..22].to_vec())
            } else {
                None
            }
        },
        ScriptType::P2WSH => {
            if script.len() >= 34 {
                Some(script[2..34].to_vec())
            } else {
                None
            }
        },
        ScriptType::Unknown => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_identify_p2pkh() {
        let script = vec![
            0x76, 0xa9, 0x14, // OP_DUP OP_HASH160 PUSH20
            0x89, 0xab, 0xcd, 0xef, 0xab, 0xba, 0xab, 0xba,
            0xab, 0xba, 0xab, 0xba, 0xab, 0xba, 0xab, 0xba,
            0xab, 0xba, 0xab, 0xba, // 20 bytes of hash
            0x88, 0xac, // OP_EQUALVERIFY OP_CHECKSIG
        ];
        
        assert_eq!(identify_script_type(&script), ScriptType::P2PKH);
    }
    
    #[test]
    fn test_identify_p2sh() {
        let script = vec![
            0xa9, 0x14, // OP_HASH160 PUSH20
            0x89, 0xab, 0xcd, 0xef, 0xab, 0xba, 0xab, 0xba,
            0xab, 0xba, 0xab, 0xba, 0xab, 0xba, 0xab, 0xba,
            0xab, 0xba, 0xab, 0xba, // 20 bytes of hash
            0x87, // OP_EQUAL
        ];
        
        assert_eq!(identify_script_type(&script), ScriptType::P2SH);
    }
    
    #[test]
    fn test_extract_hash() {
        let p2pkh_script = vec![
            0x76, 0xa9, 0x14, // OP_DUP OP_HASH160 PUSH20
            0x89, 0xab, 0xcd, 0xef, 0xab, 0xba, 0xab, 0xba,
            0xab, 0xba, 0xab, 0xba, 0xab, 0xba, 0xab, 0xba,
            0xab, 0xba, 0xab, 0xba, // 20 bytes of hash
            0x88, 0xac, // OP_EQUALVERIFY OP_CHECKSIG
        ];
        
        let hash = extract_script_hash(&p2pkh_script, ScriptType::P2PKH).unwrap();
        assert_eq!(hash.len(), 20);
        assert_eq!(hash[0], 0x89);
    }
}

#[cfg(test)]
mod script_validation_tests; 