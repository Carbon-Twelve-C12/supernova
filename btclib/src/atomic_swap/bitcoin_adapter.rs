//! Bitcoin blockchain adapter for atomic swaps
//!
//! This module provides Bitcoin Script generation, transaction parsing,
//! and RPC client functionality for atomic swaps.

use crate::atomic_swap::crypto::HashLock;
use crate::atomic_swap::error::{BitcoinAdapterError, ExtractionError};
use crate::atomic_swap::BitcoinHTLCReference;
use bitcoin::blockdata::opcodes;
use bitcoin::blockdata::script::{Builder as ScriptBuilder, Instruction};
use bitcoin::hashes::hex::FromHex;
use bitcoin::{Address, Network, ScriptBuf, Transaction as BitcoinTransaction, TxOut};
use serde::{Deserialize, Serialize};
use std::str::FromStr;

/// Bitcoin HTLC script type
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum HTLCScriptType {
    /// Pay-to-Script-Hash (legacy)
    P2SH,
    /// Pay-to-Witness-Script-Hash (SegWit)
    P2WSH,
}

/// Bitcoin HTLC parameters
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BitcoinHTLC {
    /// Hash lock for the HTLC
    pub hash_lock: [u8; 32],
    /// Recipient's Bitcoin public key
    pub recipient_pubkey: bitcoin::PublicKey,
    /// Sender's Bitcoin public key (for refund)
    pub sender_pubkey: bitcoin::PublicKey,
    /// Timeout block height
    pub timeout_height: u32,
    /// Script type to use
    pub script_type: HTLCScriptType,
}

impl BitcoinHTLC {
    /// Create the HTLC redeem script
    ///
    /// Script structure:
    /// ```
    /// OP_IF
    ///     OP_SHA256 <hash_of_secret> OP_EQUALVERIFY
    ///     <recipient_pubkey> OP_CHECKSIG
    /// OP_ELSE
    ///     <timeout> OP_CHECKLOCKTIMEVERIFY OP_DROP
    ///     <sender_pubkey> OP_CHECKSIG
    /// OP_ENDIF
    /// ```
    pub fn create_redeem_script(&self) -> ScriptBuf {
        ScriptBuilder::new()
            .push_opcode(opcodes::all::OP_IF)
            // Claim path
            .push_opcode(opcodes::all::OP_SHA256)
            .push_slice(&self.hash_lock)
            .push_opcode(opcodes::all::OP_EQUALVERIFY)
            .push_key(&self.recipient_pubkey)
            .push_opcode(opcodes::all::OP_CHECKSIG)
            .push_opcode(opcodes::all::OP_ELSE)
            // Refund path
            .push_int(self.timeout_height as i64)
            .push_opcode(opcodes::all::OP_CLTV)
            .push_opcode(opcodes::all::OP_DROP)
            .push_key(&self.sender_pubkey)
            .push_opcode(opcodes::all::OP_CHECKSIG)
            .push_opcode(opcodes::all::OP_ENDIF)
            .into_script()
    }

    /// Create the script pubkey (address script) for the HTLC
    pub fn create_script_pubkey(&self, network: Network) -> Result<ScriptBuf, BitcoinAdapterError> {
        let redeem_script = self.create_redeem_script();

        match self.script_type {
            HTLCScriptType::P2SH => {
                let address = Address::p2sh(&redeem_script, network)
                    .map_err(|e| BitcoinAdapterError::ScriptError(e.to_string()))?;
                Ok(address.script_pubkey())
            }
            HTLCScriptType::P2WSH => {
                let address = Address::p2wsh(&redeem_script, network);
                Ok(address.script_pubkey())
            }
        }
    }

    /// Create a Bitcoin address for the HTLC
    pub fn create_address(&self, network: Network) -> Result<Address, BitcoinAdapterError> {
        let redeem_script = self.create_redeem_script();

        match self.script_type {
            HTLCScriptType::P2SH => Address::p2sh(&redeem_script, network)
                .map_err(|e| BitcoinAdapterError::ScriptError(e.to_string())),
            HTLCScriptType::P2WSH => Ok(Address::p2wsh(&redeem_script, network)),
        }
    }
}

/// Extract secret from Bitcoin transaction
pub fn extract_secret_from_bitcoin_tx(
    tx: &BitcoinTransaction,
) -> Result<[u8; 32], ExtractionError> {
    // Check each input
    for input in &tx.input {
        // Check witness data for SegWit transactions
        if !input.witness.is_empty() && input.witness.len() >= 2 {
            // In a typical HTLC claim, the witness stack looks like:
            // [signature, preimage, redeem_script]
            // The preimage is usually the second element
            if input.witness.len() >= 2 {
                let potential_secret = &input.witness[1];
                if potential_secret.len() == 32 {
                    let mut secret = [0u8; 32];
                    secret.copy_from_slice(potential_secret);
                    return Ok(secret);
                }
            }
        }

        // Check scriptSig for legacy transactions
        let script = &input.script_sig;
        let instructions: Vec<Instruction> =
            script
                .instructions()
                .collect::<Result<Vec<_>, _>>()
                .map_err(|e| ExtractionError::ScriptParseError(e.to_string()))?;

        // Look for 32-byte push operations
        for instruction in instructions {
            if let Instruction::PushBytes(data) = instruction {
                if data.len() == 32 {
                    let mut secret = [0u8; 32];
                    secret.copy_from_slice(data.as_bytes());
                    return Ok(secret);
                }
            }
        }
    }

    Err(ExtractionError::SecretNotFound)
}

/// Parse a Bitcoin transaction to find HTLC outputs
pub fn find_htlc_outputs(
    tx: &BitcoinTransaction,
    expected_script: &ScriptBuf,
) -> Vec<(u32, TxOut)> {
    tx.output
        .iter()
        .enumerate()
        .filter(|(_, output)| output.script_pubkey == *expected_script)
        .map(|(index, output)| (index as u32, output.clone()))
        .collect()
}

/// Bitcoin RPC client wrapper
#[cfg(feature = "atomic-swap")]
pub struct BitcoinRpcClient {
    pub(crate) client: bitcoincore_rpc::Client,
    network: Network,
}

#[cfg(feature = "atomic-swap")]
impl BitcoinRpcClient {
    /// Create a new Bitcoin RPC client
    pub fn new(
        url: &str,
        user: Option<String>,
        pass: Option<String>,
        network: Network,
    ) -> Result<Self, BitcoinAdapterError> {
        use bitcoincore_rpc::{Auth, Client, RpcApi};

        let auth = match (user, pass) {
            (Some(u), Some(p)) => Auth::UserPass(u, p),
            _ => Auth::None,
        };

        let client =
            Client::new(url, auth).map_err(|e| BitcoinAdapterError::RpcError(e.to_string()))?;

        Ok(Self { client, network })
    }

    /// Get current block height
    pub async fn get_block_height(&self) -> Result<u64, BitcoinAdapterError> {
        use bitcoincore_rpc::RpcApi;
        self.client
            .get_block_count()
            .map_err(|e| BitcoinAdapterError::RpcError(e.to_string()))
    }

    /// Get transaction by ID
    pub async fn get_transaction(
        &self,
        txid: &str,
    ) -> Result<BitcoinTransaction, BitcoinAdapterError> {
        use bitcoin::Txid;
        use bitcoincore_rpc::RpcApi;

        let txid =
            Txid::from_str(txid).map_err(|e| BitcoinAdapterError::ParseError(e.to_string()))?;

        self.client
            .get_raw_transaction(&txid, None)
            .map_err(|e| BitcoinAdapterError::RpcError(e.to_string()))
    }

    /// Monitor for HTLC claims in new blocks
    pub async fn monitor_for_claims(
        &self,
        htlc_address: &Address,
        start_height: u64,
    ) -> Result<Vec<BitcoinTransaction>, BitcoinAdapterError> {
        use bitcoin::Block;
        use bitcoincore_rpc::RpcApi;

        let current_height = self.get_block_height().await?;
        let mut claim_txs = Vec::new();

        for height in start_height..=current_height {
            let block_hash = self
                .client
                .get_block_hash(height)
                .map_err(|e| BitcoinAdapterError::RpcError(e.to_string()))?;

            let block: Block = self
                .client
                .get_block(&block_hash)
                .map_err(|e| BitcoinAdapterError::RpcError(e.to_string()))?;

            // Check each transaction in the block
            for tx in &block.txdata {
                // Check if any input spends from our HTLC address
                for input in &tx.input {
                    // This is simplified - in production, we'd need to check the
                    // previous output being spent
                    claim_txs.push(tx.clone());
                    break;
                }
            }
        }

        Ok(claim_txs)
    }
}

/// Utilities for working with Bitcoin scripts
pub mod script_utils {
    use super::*;
    use bitcoin::blockdata::script::Instruction;

    /// Check if a script is an HTLC script
    pub fn is_htlc_script(script: &ScriptBuf) -> bool {
        let instructions: Vec<Instruction> =
            match script.instructions().collect::<Result<Vec<_>, _>>() {
                Ok(inst) => inst,
                Err(_) => return false,
            };

        // Basic pattern matching for HTLC structure
        if instructions.len() < 10 {
            return false;
        }

        // Check for OP_IF at start
        matches!(
            instructions.first(),
            Some(Instruction::Op(opcodes::all::OP_IF))
        )
    }

    /// Extract timeout value from HTLC script
    pub fn extract_timeout_from_script(script: &ScriptBuf) -> Option<u32> {
        let instructions: Vec<Instruction> =
            script.instructions().collect::<Result<Vec<_>, _>>().ok()?;

        // Look for CLTV opcode and preceding push
        for (i, instruction) in instructions.iter().enumerate() {
            if matches!(instruction, Instruction::Op(opcodes::all::OP_CLTV)) {
                // Get the previous instruction which should be the timeout
                if i > 0 {
                    if let Some(Instruction::PushBytes(data)) = instructions.get(i - 1) {
                        if data.len() <= 4 {
                            let mut bytes = [0u8; 4];
                            bytes[..data.len()].copy_from_slice(data.as_bytes());
                            return Some(u32::from_le_bytes(bytes));
                        }
                    }
                }
            }
        }

        None
    }
}

/// Create a reference to a Bitcoin HTLC from a transaction
pub fn create_bitcoin_htlc_reference(
    tx: &BitcoinTransaction,
    vout: u32,
    timeout_height: u32,
) -> Result<BitcoinHTLCReference, BitcoinAdapterError> {
    let output = tx
        .output
        .get(vout as usize)
        .ok_or_else(|| BitcoinAdapterError::ParseError("Invalid output index".to_string()))?;

    // Try to extract address from script
    let address = bitcoin::Address::from_script(&output.script_pubkey, bitcoin::Network::Bitcoin)
        .map(|addr| addr.to_string())
        .unwrap_or_else(|_| "unknown".to_string());

    Ok(BitcoinHTLCReference {
        txid: tx.txid().to_string(),
        vout,
        script_pubkey: output.script_pubkey.to_bytes(),
        amount: output.value,
        timeout_height,
        address,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use bitcoin::secp256k1::Secp256k1;
    use bitcoin::PrivateKey;

    fn create_test_htlc() -> BitcoinHTLC {
        let secp = Secp256k1::new();
        let sender_key = PrivateKey::from_slice(&[1u8; 32], Network::Testnet).unwrap();
        let recipient_key = PrivateKey::from_slice(&[2u8; 32], Network::Testnet).unwrap();

        BitcoinHTLC {
            hash_lock: [0x42; 32],
            recipient_pubkey: sender_key.public_key(&secp),
            sender_pubkey: recipient_key.public_key(&secp),
            timeout_height: 500000,
            script_type: HTLCScriptType::P2WSH,
        }
    }

    #[test]
    fn test_htlc_script_creation() {
        let htlc = create_test_htlc();
        let script = htlc.create_redeem_script();

        // Verify script is not empty
        assert!(!script.is_empty());

        // Verify it starts with OP_IF
        let first_op = script.instructions().next().unwrap().unwrap();
        assert!(matches!(first_op, Instruction::Op(opcodes::all::OP_IF)));
    }

    #[test]
    fn test_htlc_address_creation() {
        let htlc = create_test_htlc();
        let address = htlc.create_address(Network::Testnet).unwrap();

        // Verify we get a valid testnet address
        assert!(address.to_string().starts_with("tb1") || address.to_string().starts_with("2"));
    }

    #[test]
    fn test_script_utils() {
        let htlc = create_test_htlc();
        let script = htlc.create_redeem_script();

        // Test is_htlc_script
        assert!(script_utils::is_htlc_script(&script));

        // Test timeout extraction
        let timeout = script_utils::extract_timeout_from_script(&script);
        assert_eq!(timeout, Some(500000));
    }
}
