/// Script implementation (opcode-compatible)
use serde::{Deserialize, Serialize};

/// Script type for transaction inputs and outputs
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Script {
    /// Raw script bytes
    bytes: Vec<u8>,
}

impl Script {
    /// Create a new empty script
    pub fn new() -> Self {
        Self { bytes: Vec::new() }
    }

    /// Create a script from bytes
    pub fn from_bytes(bytes: Vec<u8>) -> Self {
        Self { bytes }
    }

    /// Get the script bytes
    pub fn as_bytes(&self) -> &[u8] {
        &self.bytes
    }

    /// Get the script length
    pub fn len(&self) -> usize {
        self.bytes.len()
    }

    /// Check if the script is empty
    pub fn is_empty(&self) -> bool {
        self.bytes.is_empty()
    }

    /// Create a P2PKH (Pay to Public Key Hash) script
    pub fn p2pkh(pubkey_hash: &[u8; 20]) -> Self {
        let mut bytes = Vec::with_capacity(25);
        bytes.push(0x76); // OP_DUP
        bytes.push(0xa9); // OP_HASH160
        bytes.push(0x14); // Push 20 bytes
        bytes.extend_from_slice(pubkey_hash);
        bytes.push(0x88); // OP_EQUALVERIFY
        bytes.push(0xac); // OP_CHECKSIG
        Self { bytes }
    }

    /// Create a P2SH (Pay to Script Hash) script
    pub fn p2sh(script_hash: &[u8; 20]) -> Self {
        let mut bytes = Vec::with_capacity(23);
        bytes.push(0xa9); // OP_HASH160
        bytes.push(0x14); // Push 20 bytes
        bytes.extend_from_slice(script_hash);
        bytes.push(0x87); // OP_EQUAL
        Self { bytes }
    }

    /// Create a P2WPKH (Pay to Witness Public Key Hash) script
    pub fn new_p2wpkh(pubkey_hash: &[u8]) -> Self {
        let mut bytes = Vec::new();
        bytes.push(0x00); // OP_0 (witness version)
        bytes.push(0x14); // Push 20 bytes
        if pubkey_hash.len() >= 20 {
            bytes.extend_from_slice(&pubkey_hash[..20]);
        } else {
            bytes.extend_from_slice(pubkey_hash);
            // Pad with zeros if needed
            bytes.resize(22, 0);
        }
        Self { bytes }
    }

    /// Create a P2WSH (Pay to Witness Script Hash) script
    pub fn new_p2wsh(script_hash: &[u8]) -> Self {
        let mut bytes = Vec::new();
        bytes.push(0x00); // OP_0 (witness version)
        bytes.push(0x20); // Push 32 bytes
        if script_hash.len() >= 32 {
            bytes.extend_from_slice(&script_hash[..32]);
        } else {
            bytes.extend_from_slice(script_hash);
            // Pad with zeros if needed
            bytes.resize(34, 0);
        }
        Self { bytes }
    }
}

impl Default for Script {
    fn default() -> Self {
        Self::new()
    }
}

/// Script opcodes
/// Using the original OP_X naming convention for clarity and compatibility
#[allow(non_camel_case_types)]
#[allow(clippy::upper_case_acronyms)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum OpCode {
    // Push values
    Op0 = 0x00,
    OpPushData1 = 0x4c,
    OpPushData2 = 0x4d,
    OpPushData4 = 0x4e,
    Op1Negate = 0x4f,
    Op1 = 0x51,

    // Control
    OpNop = 0x61,
    OpIf = 0x63,
    OpNotIf = 0x64,
    OpElse = 0x67,
    OpEndIf = 0x68,
    OpVerify = 0x69,
    OpReturn = 0x6a,

    // Stack operations
    OpDup = 0x76,
    OpDrop = 0x75,
    OpSwap = 0x7c,

    // Crypto
    OpHash160 = 0xa9,
    OpCheckSig = 0xac,
    OpCheckSigVerify = 0xad,
    OpCheckMultiSig = 0xae,

    // Comparison
    OpEqual = 0x87,
    OpEqualVerify = 0x88,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_p2pkh_script() {
        let pubkey_hash = [0u8; 20];
        let script = Script::p2pkh(&pubkey_hash);
        assert_eq!(script.len(), 25);
        assert_eq!(script.as_bytes()[0], 0x76); // OP_DUP
    }

    #[test]
    fn test_p2sh_script() {
        let script_hash = [0u8; 20];
        let script = Script::p2sh(&script_hash);
        assert_eq!(script.len(), 23);
        assert_eq!(script.as_bytes()[0], 0xa9); // OP_HASH160
    }
}
