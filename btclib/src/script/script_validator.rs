//! Script Validator for Supernova
//!
//! This module provides high-level script validation for transactions.

use crate::crypto::signature::{verify_signature, SignatureType};
use crate::script::interpreter::{ScriptError, ScriptInterpreter, SignatureChecker};
use crate::script::ScriptVerificationError;
use crate::script::{extract_script_hash, identify_script_type, ScriptType};
use crate::types::transaction::Transaction;
use ripemd::{Digest as RipemdDigest, Ripemd160};
use sha2::{Digest, Sha256};

/// Script validation flags
#[derive(Debug, Clone, Copy)]
pub struct ScriptFlags {
    /// Require minimal push operations
    pub verify_minimaldata: bool,
    /// Verify signature encoding strictly
    pub verify_dersig: bool,
    /// Verify public key encoding strictly
    pub verify_strictenc: bool,
    /// Enable CHECKLOCKTIMEVERIFY
    pub verify_checklocktimeverify: bool,
    /// Enable CHECKSEQUENCEVERIFY
    pub verify_checksequenceverify: bool,
    /// Witness support
    pub verify_witness: bool,
    /// Discourage upgradable witness program
    pub verify_discourage_upgradable_witness_program: bool,
}

impl Default for ScriptFlags {
    fn default() -> Self {
        Self {
            verify_minimaldata: true,
            verify_dersig: true,
            verify_strictenc: true,
            verify_checklocktimeverify: true,
            verify_checksequenceverify: true,
            verify_witness: true,
            verify_discourage_upgradable_witness_program: true,
        }
    }
}

/// Script validator
pub struct ScriptValidator<'a> {
    transaction: &'a Transaction,
    input_index: usize,
    flags: ScriptFlags,
}

impl<'a> ScriptValidator<'a> {
    /// Create a new script validator
    pub fn new(transaction: &'a Transaction, input_index: usize, flags: ScriptFlags) -> Self {
        Self {
            transaction,
            input_index,
            flags,
        }
    }

    /// Validate a script
    pub fn validate(
        &self,
        script_sig: &[u8],
        script_pubkey: &[u8],
        amount: u64,
    ) -> Result<(), ScriptVerificationError> {
        // Identify script type
        let script_type = identify_script_type(script_pubkey);

        match script_type {
            ScriptType::P2PKH => self.validate_p2pkh(script_sig, script_pubkey)?,
            ScriptType::P2SH => self.validate_p2sh(script_sig, script_pubkey)?,
            ScriptType::P2WPKH => {
                if self.flags.verify_witness {
                    self.validate_p2wpkh(script_sig, script_pubkey, amount)?;
                } else {
                    return Err(ScriptVerificationError::ExecutionFailed(
                        "Witness validation disabled".to_string(),
                    ));
                }
            }
            ScriptType::P2WSH => {
                if self.flags.verify_witness {
                    self.validate_p2wsh(script_sig, script_pubkey, amount)?;
                } else {
                    return Err(ScriptVerificationError::ExecutionFailed(
                        "Witness validation disabled".to_string(),
                    ));
                }
            }
            ScriptType::Unknown => {
                // For unknown scripts, just run the script sig followed by script pubkey
                self.validate_raw(script_sig, script_pubkey)?;
            }
        }

        Ok(())
    }

    /// Validate P2PKH script
    fn validate_p2pkh(
        &self,
        script_sig: &[u8],
        script_pubkey: &[u8],
    ) -> Result<(), ScriptVerificationError> {
        // P2PKH script_pubkey: OP_DUP OP_HASH160 <pubkey_hash> OP_EQUALVERIFY OP_CHECKSIG
        // script_sig should contain: <signature> <pubkey>

        // Extract the pubkey hash from script
        let pubkey_hash = extract_script_hash(script_pubkey, ScriptType::P2PKH)
            .ok_or(ScriptVerificationError::InvalidScriptStructure)?;

        // Parse script_sig to extract signature and pubkey
        let (signature, pubkey) = self.parse_p2pkh_script_sig(script_sig)?;

        // Verify pubkey hash matches
        let computed_hash = Self::hash160(&pubkey);
        if computed_hash != pubkey_hash {
            return Err(ScriptVerificationError::PublicKeyVerificationFailed);
        }

        // Create checker and run the scripts
        let checker = TransactionChecker::new(self.transaction, self.input_index);
        let mut interpreter = ScriptInterpreter::new();

        // First run script_sig
        interpreter
            .execute(script_sig, &checker)
            .map_err(|e| ScriptVerificationError::ExecutionFailed(format!("{:?}", e)))?;

        // Then run script_pubkey
        let result = interpreter
            .execute(script_pubkey, &checker)
            .map_err(|e| ScriptVerificationError::ExecutionFailed(format!("{:?}", e)))?;

        if !result {
            return Err(ScriptVerificationError::VerifyFailed);
        }

        Ok(())
    }

    /// Validate P2SH script
    fn validate_p2sh(
        &self,
        script_sig: &[u8],
        script_pubkey: &[u8],
    ) -> Result<(), ScriptVerificationError> {
        // P2SH script_pubkey: OP_HASH160 <script_hash> OP_EQUAL
        // script_sig should contain: ... <redeem_script>

        // Extract the script hash
        let script_hash = extract_script_hash(script_pubkey, ScriptType::P2SH)
            .ok_or(ScriptVerificationError::InvalidScriptStructure)?;

        // Get the redeem script (last element of script_sig)
        let redeem_script = self.extract_redeem_script(script_sig)?;

        // Verify redeem script hash matches
        let computed_hash = Self::hash160(&redeem_script);
        if computed_hash != script_hash {
            return Err(ScriptVerificationError::ScriptHashMismatch);
        }

        // Create checker and interpreter
        let checker = TransactionChecker::new(self.transaction, self.input_index);
        let mut interpreter = ScriptInterpreter::new();

        // Run script_sig (without redeem script) + redeem script
        let script_sig_without_redeem = &script_sig[..script_sig.len() - redeem_script.len() - 1];

        // Execute script_sig first
        interpreter
            .execute(script_sig_without_redeem, &checker)
            .map_err(|e| ScriptVerificationError::ExecutionFailed(format!("{:?}", e)))?;

        // Then execute the redeem script
        let result = interpreter
            .execute(&redeem_script, &checker)
            .map_err(|e| ScriptVerificationError::ExecutionFailed(format!("{:?}", e)))?;

        if !result {
            return Err(ScriptVerificationError::VerifyFailed);
        }

        Ok(())
    }

    /// Validate P2WPKH (witness)
    fn validate_p2wpkh(
        &self,
        _script_sig: &[u8],
        script_pubkey: &[u8],
        _amount: u64,
    ) -> Result<(), ScriptVerificationError> {
        // For P2WPKH, script_sig should be empty
        // Witness data is handled separately

        // Extract witness program
        let _witness_program = extract_script_hash(script_pubkey, ScriptType::P2WPKH)
            .ok_or(ScriptVerificationError::InvalidScriptStructure)?;

        // TODO: Implement witness validation
        // For now, we'll return an error to avoid the always-true vulnerability
        Err(ScriptVerificationError::ExecutionFailed(
            "Witness validation not yet implemented".to_string(),
        ))
    }

    /// Validate P2WSH (witness)
    fn validate_p2wsh(
        &self,
        _script_sig: &[u8],
        script_pubkey: &[u8],
        _amount: u64,
    ) -> Result<(), ScriptVerificationError> {
        // For P2WSH, script_sig should be empty
        // Witness data is handled separately

        // Extract witness program
        let _witness_program = extract_script_hash(script_pubkey, ScriptType::P2WSH)
            .ok_or(ScriptVerificationError::InvalidScriptStructure)?;

        // TODO: Implement witness validation
        // For now, we'll return an error to avoid the always-true vulnerability
        Err(ScriptVerificationError::ExecutionFailed(
            "Witness validation not yet implemented".to_string(),
        ))
    }

    /// Validate raw/unknown script
    fn validate_raw(
        &self,
        script_sig: &[u8],
        script_pubkey: &[u8],
    ) -> Result<(), ScriptVerificationError> {
        let checker = TransactionChecker::new(self.transaction, self.input_index);
        let mut interpreter = ScriptInterpreter::new();

        // Execute script_sig first
        interpreter
            .execute(script_sig, &checker)
            .map_err(|e| ScriptVerificationError::ExecutionFailed(format!("{:?}", e)))?;

        // Then execute script_pubkey
        let result = interpreter
            .execute(script_pubkey, &checker)
            .map_err(|e| ScriptVerificationError::ExecutionFailed(format!("{:?}", e)))?;

        if !result {
            return Err(ScriptVerificationError::VerifyFailed);
        }

        Ok(())
    }

    /// Parse P2PKH script signature
    fn parse_p2pkh_script_sig(
        &self,
        script_sig: &[u8],
    ) -> Result<(Vec<u8>, Vec<u8>), ScriptVerificationError> {
        if script_sig.len() < 2 {
            return Err(ScriptVerificationError::InvalidScriptStructure);
        }

        let mut offset = 0;

        // Read signature
        let sig_len = script_sig[offset] as usize;
        offset += 1;

        if offset + sig_len > script_sig.len() {
            return Err(ScriptVerificationError::InvalidScriptStructure);
        }

        let signature = script_sig[offset..offset + sig_len].to_vec();
        offset += sig_len;

        // Read pubkey
        if offset >= script_sig.len() {
            return Err(ScriptVerificationError::InvalidScriptStructure);
        }

        let pubkey_len = script_sig[offset] as usize;
        offset += 1;

        if offset + pubkey_len != script_sig.len() {
            return Err(ScriptVerificationError::InvalidScriptStructure);
        }

        let pubkey = script_sig[offset..offset + pubkey_len].to_vec();

        Ok((signature, pubkey))
    }

    /// Extract redeem script from P2SH script_sig
    fn extract_redeem_script(&self, script_sig: &[u8]) -> Result<Vec<u8>, ScriptVerificationError> {
        if script_sig.is_empty() {
            return Err(ScriptVerificationError::InvalidScriptStructure);
        }

        // The redeem script is the last push operation
        let last_byte_pos = script_sig.len() - 1;
        let mut pos = last_byte_pos;

        // Find the start of the last push
        while pos > 0 {
            let len = script_sig[pos - 1] as usize;
            if pos > len {
                // Found a valid push
                return Ok(script_sig[pos..pos + len].to_vec());
            }
            pos -= 1;
        }

        Err(ScriptVerificationError::InvalidScriptStructure)
    }

    /// Hash160 (SHA256 + RIPEMD160)
    fn hash160(data: &[u8]) -> Vec<u8> {
        let mut sha = Sha256::new();
        sha.update(data);
        let sha_result = sha.finalize();

        let mut ripemd = Ripemd160::new();
        ripemd.update(sha_result);
        ripemd.finalize().to_vec()
    }
}

/// Transaction signature checker
struct TransactionChecker<'a> {
    transaction: &'a Transaction,
    input_index: usize,
}

impl<'a> TransactionChecker<'a> {
    fn new(transaction: &'a Transaction, input_index: usize) -> Self {
        Self {
            transaction,
            input_index,
        }
    }
}

impl<'a> SignatureChecker for TransactionChecker<'a> {
    fn check_signature(&self, signature: &[u8], pubkey: &[u8]) -> Result<bool, ScriptError> {
        // Calculate sighash for this input
        let sighash = self.transaction.hash(); // Simplified - should use proper sighash calculation

        // Verify the signature
        match verify_signature(SignatureType::Secp256k1, pubkey, &sighash, signature) {
            Ok(valid) => Ok(valid),
            Err(_) => Err(ScriptError::SignatureFailed),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::transaction::{TransactionInput, TransactionOutput};

    #[test]
    fn test_script_type_identification() {
        // P2PKH script
        let p2pkh = vec![
            0x76, 0xa9, 0x14, // OP_DUP OP_HASH160 PUSH20
            0x89, 0xab, 0xcd, 0xef, 0xab, 0xba, 0xab, 0xba, 0xab, 0xba, 0xab, 0xba, 0xab, 0xba,
            0xab, 0xba, 0xab, 0xba, 0xab, 0xba, // 20 bytes
            0x88, 0xac, // OP_EQUALVERIFY OP_CHECKSIG
        ];

        assert_eq!(identify_script_type(&p2pkh), ScriptType::P2PKH);
    }

    #[test]
    fn test_script_validation_rejects_invalid() {
        // Create a test transaction
        let tx = Transaction::new(
            1,
            vec![TransactionInput::new([0; 32], 0, vec![], 0)],
            vec![TransactionOutput::new(5000, vec![])],
            0,
        );

        let validator = ScriptValidator::new(&tx, 0, ScriptFlags::default());

        // Try to validate empty scripts (should fail)
        let result = validator.validate(&[], &[], 0);
        assert!(result.is_err());
    }
}
