//! Script Interpreter for Supernova
//!
//! This module provides the core script execution engine that validates
//! transaction scripts by executing opcodes and maintaining the stack.

use crate::script::opcodes::Opcode;
use ripemd::{Digest as RipemdDigest, Ripemd160};
use sha2::{Digest, Sha256};

/// Maximum script size in bytes
pub const MAX_SCRIPT_SIZE: usize = 10_000;

/// Maximum number of operations allowed
pub const MAX_OPS_PER_SCRIPT: usize = 201;

/// Maximum stack size
pub const MAX_STACK_SIZE: usize = 1000;

/// Maximum gas for script execution (prevents DoS)
pub const MAX_SCRIPT_GAS: u64 = 100_000;

/// Base gas cost per operation
pub const BASE_GAS_COST: u64 = 10;

/// Gas cost for cryptographic operations
pub const CRYPTO_GAS_COST: u64 = 100;

/// Gas cost for hash operations
pub const HASH_GAS_COST: u64 = 50;

/// Maximum script element size
pub const MAX_SCRIPT_ELEMENT_SIZE: usize = 520;

/// Script execution errors
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ScriptError {
    /// Script size exceeded
    ScriptTooLarge,
    /// Too many operations
    TooManyOps,
    /// Stack overflow
    StackOverflow,
    /// Stack underflow
    StackUnderflow,
    /// Invalid opcode
    InvalidOpcode(u8),
    /// Disabled opcode
    DisabledOpcode(String),
    /// Verify operation failed
    VerifyFailed,
    /// Equal verification failed
    EqualVerifyFailed,
    /// Invalid stack operation
    InvalidStackOperation,
    /// Invalid number encoding
    InvalidNumber,
    /// Signature verification failed
    SignatureFailed,
    /// Pubkey verification failed
    PubkeyFailed,
    /// Unexpected end of script
    UnexpectedEndOfScript,
    /// Unbalanced conditional
    UnbalancedConditional,
    /// Invalid signature encoding
    InvalidSignatureEncoding,
    /// Invalid pubkey encoding
    InvalidPubkeyEncoding,
    /// Element too large
    ElementTooLarge,
    /// Gas limit exceeded (DoS prevention)
    GasExhausted { used: u64, limit: u64 },
}

/// Stack for script execution
#[derive(Debug, Clone)]
pub struct ExecutionStack {
    items: Vec<Vec<u8>>,
}

impl Default for ExecutionStack {
    fn default() -> Self {
        Self::new()
    }
}

impl ExecutionStack {
    /// Create a new empty stack
    pub fn new() -> Self {
        Self { items: Vec::new() }
    }

    /// Push an item onto the stack
    pub fn push(&mut self, item: Vec<u8>) -> Result<(), ScriptError> {
        if self.items.len() >= MAX_STACK_SIZE {
            return Err(ScriptError::StackOverflow);
        }
        if item.len() > MAX_SCRIPT_ELEMENT_SIZE {
            return Err(ScriptError::ElementTooLarge);
        }
        self.items.push(item);
        Ok(())
    }

    /// Pop an item from the stack
    pub fn pop(&mut self) -> Result<Vec<u8>, ScriptError> {
        self.items.pop().ok_or(ScriptError::StackUnderflow)
    }

    /// Peek at the top item without removing it
    pub fn peek(&self) -> Result<&Vec<u8>, ScriptError> {
        self.items.last().ok_or(ScriptError::StackUnderflow)
    }

    /// Get the stack size
    pub fn len(&self) -> usize {
        self.items.len()
    }

    /// Check if stack is empty
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// Duplicate the top item
    pub fn dup(&mut self) -> Result<(), ScriptError> {
        let top = self.peek()?.clone();
        self.push(top)
    }

    /// Swap the top two items
    pub fn swap(&mut self) -> Result<(), ScriptError> {
        if self.items.len() < 2 {
            return Err(ScriptError::StackUnderflow);
        }
        let len = self.items.len();
        self.items.swap(len - 1, len - 2);
        Ok(())
    }

    /// Remove an item at index from top
    pub fn remove(&mut self, index: usize) -> Result<Vec<u8>, ScriptError> {
        if index >= self.items.len() {
            return Err(ScriptError::StackUnderflow);
        }
        let pos = self.items.len() - 1 - index;
        Ok(self.items.remove(pos))
    }

    /// Get item at index from top
    pub fn get(&self, index: usize) -> Result<&Vec<u8>, ScriptError> {
        if index >= self.items.len() {
            return Err(ScriptError::StackUnderflow);
        }
        let pos = self.items.len() - 1 - index;
        Ok(&self.items[pos])
    }
}

/// Script interpreter with gas limits for DoS prevention
pub struct ScriptInterpreter {
    /// Main stack
    stack: ExecutionStack,
    /// Alt stack
    alt_stack: ExecutionStack,
    /// Conditional execution state
    cond_stack: Vec<bool>,
    /// Operation count
    op_count: usize,
    /// Gas used for execution (prevents DoS via complex scripts)
    gas_used: u64,
    /// Maximum gas allowed
    gas_limit: u64,
}

impl Default for ScriptInterpreter {
    fn default() -> Self {
        Self::new()
    }
}

impl ScriptInterpreter {
    /// Create a new script interpreter
    pub fn new() -> Self {
        Self {
            stack: ExecutionStack::new(),
            alt_stack: ExecutionStack::new(),
            cond_stack: Vec::new(),
            op_count: 0,
            gas_used: 0,
            gas_limit: MAX_SCRIPT_GAS,
        }
    }

    /// Create a new script interpreter with custom gas limit
    pub fn with_gas_limit(gas_limit: u64) -> Self {
        Self {
            stack: ExecutionStack::new(),
            alt_stack: ExecutionStack::new(),
            cond_stack: Vec::new(),
            op_count: 0,
            gas_used: 0,
            gas_limit,
        }
    }

    /// Push an item onto the main stack (for witness script execution)
    pub fn push_stack(&mut self, item: Vec<u8>) -> Result<(), ScriptError> {
        self.stack.push(item)
    }

    /// Execute a script
    pub fn execute(
        &mut self,
        script: &[u8],
        checker: &dyn SignatureChecker,
    ) -> Result<bool, ScriptError> {
        if script.len() > MAX_SCRIPT_SIZE {
            return Err(ScriptError::ScriptTooLarge);
        }

        let mut pc = 0;

        while pc < script.len() {
            // Check operation limit
            self.op_count += 1;
            if self.op_count > MAX_OPS_PER_SCRIPT {
                return Err(ScriptError::TooManyOps);
            }

            // Check if we should execute (not in false conditional branch)
            let executing = self.cond_stack.iter().all(|&b| b);

            // Read next instruction
            let opcode_byte = script[pc];
            pc += 1;

            // Handle push operations (1-75 bytes)
            if (1..=75).contains(&opcode_byte) {
                let push_len = opcode_byte as usize;
                if pc + push_len > script.len() {
                    return Err(ScriptError::UnexpectedEndOfScript);
                }

                if executing {
                    let data = script[pc..pc + push_len].to_vec();
                    self.stack.push(data)?;
                }
                pc += push_len;
                continue;
            }

            // Get opcode
            let opcode = match Opcode::from_byte(opcode_byte) {
                Some(op) => op,
                None => return Err(ScriptError::InvalidOpcode(opcode_byte)),
            };

            // Check for disabled opcodes
            if opcode.is_disabled() {
                return Err(ScriptError::DisabledOpcode(format!("{}", opcode)));
            }

            // Handle push data opcodes
            match opcode {
                Opcode::OP_PUSHDATA1 => {
                    if pc >= script.len() {
                        return Err(ScriptError::UnexpectedEndOfScript);
                    }
                    let len = script[pc] as usize;
                    pc += 1;

                    if pc + len > script.len() {
                        return Err(ScriptError::UnexpectedEndOfScript);
                    }

                    if executing {
                        let data = script[pc..pc + len].to_vec();
                        self.stack.push(data)?;
                    }
                    pc += len;
                    continue;
                }
                Opcode::OP_PUSHDATA2 => {
                    if pc + 2 > script.len() {
                        return Err(ScriptError::UnexpectedEndOfScript);
                    }
                    let len = u16::from_le_bytes([script[pc], script[pc + 1]]) as usize;
                    pc += 2;

                    if pc + len > script.len() {
                        return Err(ScriptError::UnexpectedEndOfScript);
                    }

                    if executing {
                        let data = script[pc..pc + len].to_vec();
                        self.stack.push(data)?;
                    }
                    pc += len;
                    continue;
                }
                Opcode::OP_PUSHDATA4 => {
                    if pc + 4 > script.len() {
                        return Err(ScriptError::UnexpectedEndOfScript);
                    }
                    let len = u32::from_le_bytes([
                        script[pc],
                        script[pc + 1],
                        script[pc + 2],
                        script[pc + 3],
                    ]) as usize;
                    pc += 4;

                    if pc + len > script.len() {
                        return Err(ScriptError::UnexpectedEndOfScript);
                    }

                    if executing {
                        let data = script[pc..pc + len].to_vec();
                        self.stack.push(data)?;
                    }
                    pc += len;
                    continue;
                }
                _ => {}
            }

            // Execute opcode if we're in an executing branch
            if executing {
                // Consume gas before executing opcode
                let gas_cost = self.get_opcode_gas_cost(&opcode);
                self.consume_gas(gas_cost)?;

                self.execute_opcode(opcode, checker)?;
            } else {
                // Still need to track conditionals even when not executing
                match opcode {
                    Opcode::OP_IF | Opcode::OP_NOTIF => {
                        self.cond_stack.push(false);
                    }
                    Opcode::OP_ELSE => {
                        if self.cond_stack.is_empty() {
                            return Err(ScriptError::UnbalancedConditional);
                        }
                    }
                    Opcode::OP_ENDIF => {
                        if self.cond_stack.is_empty() {
                            return Err(ScriptError::UnbalancedConditional);
                        }
                        self.cond_stack.pop();
                    }
                    _ => {}
                }
            }
        }

        // Check for unbalanced conditionals
        if !self.cond_stack.is_empty() {
            return Err(ScriptError::UnbalancedConditional);
        }

        // Script succeeds if stack is not empty and top value is true
        if self.stack.is_empty() {
            Ok(false)
        } else {
            Ok(self.is_true(self.stack.peek()?))
        }
    }

    /// Get gas cost for an opcode
    fn get_opcode_gas_cost(&self, opcode: &Opcode) -> u64 {
        match opcode {
            // Cryptographic operations are expensive
            Opcode::OP_CHECKSIG | Opcode::OP_CHECKSIGVERIFY => CRYPTO_GAS_COST,

            // Hash operations are moderately expensive
            Opcode::OP_RIPEMD160 | Opcode::OP_SHA256 | Opcode::OP_HASH160 | Opcode::OP_HASH256 => {
                HASH_GAS_COST
            }

            // All other operations have base cost
            _ => BASE_GAS_COST,
        }
    }

    /// Consume gas for operation
    fn consume_gas(&mut self, amount: u64) -> Result<(), ScriptError> {
        self.gas_used = self.gas_used.saturating_add(amount);
        if self.gas_used > self.gas_limit {
            Err(ScriptError::GasExhausted {
                used: self.gas_used,
                limit: self.gas_limit,
            })
        } else {
            Ok(())
        }
    }

    /// Execute a single opcode
    fn execute_opcode(
        &mut self,
        opcode: Opcode,
        checker: &dyn SignatureChecker,
    ) -> Result<(), ScriptError> {
        match opcode {
            // Constants
            Opcode::OP_0 => {
                self.stack.push(vec![])?;
                Ok(())
            }
            Opcode::OP_1 => {
                self.stack.push(vec![1])?;
                Ok(())
            }
            Opcode::OP_2 => {
                self.stack.push(vec![0x02])?;
                Ok(())
            }
            Opcode::OP_3 => {
                self.stack.push(vec![0x03])?;
                Ok(())
            }
            Opcode::OP_4 => {
                self.stack.push(vec![0x04])?;
                Ok(())
            }
            Opcode::OP_5 => {
                self.stack.push(vec![0x05])?;
                Ok(())
            }
            Opcode::OP_6 => {
                self.stack.push(vec![0x06])?;
                Ok(())
            }
            Opcode::OP_7 => {
                self.stack.push(vec![0x07])?;
                Ok(())
            }
            Opcode::OP_8 => {
                self.stack.push(vec![0x08])?;
                Ok(())
            }
            Opcode::OP_9 => {
                self.stack.push(vec![0x09])?;
                Ok(())
            }
            Opcode::OP_10 => {
                self.stack.push(vec![0x0a])?;
                Ok(())
            }
            Opcode::OP_11 => {
                self.stack.push(vec![0x0b])?;
                Ok(())
            }
            Opcode::OP_12 => {
                self.stack.push(vec![0x0c])?;
                Ok(())
            }
            Opcode::OP_13 => {
                self.stack.push(vec![0x0d])?;
                Ok(())
            }
            Opcode::OP_14 => {
                self.stack.push(vec![0x0e])?;
                Ok(())
            }
            Opcode::OP_15 => {
                self.stack.push(vec![0x0f])?;
                Ok(())
            }
            Opcode::OP_16 => {
                self.stack.push(vec![0x10])?;
                Ok(())
            }

            // Flow control
            Opcode::OP_NOP => Ok(()),
            Opcode::OP_IF => {
                let mut execute = false;
                if !self.stack.is_empty() {
                    let value = self.stack.pop()?;
                    execute = self.is_true(&value);
                }
                self.cond_stack.push(execute);
                Ok(())
            }
            Opcode::OP_NOTIF => {
                let mut execute = true;
                if !self.stack.is_empty() {
                    let value = self.stack.pop()?;
                    execute = !self.is_true(&value);
                }
                self.cond_stack.push(execute);
                Ok(())
            }
            Opcode::OP_ELSE => {
                if self.cond_stack.is_empty() {
                    return Err(ScriptError::UnbalancedConditional);
                }
                let last = self.cond_stack.len() - 1;
                self.cond_stack[last] = !self.cond_stack[last];
                Ok(())
            }
            Opcode::OP_ENDIF => {
                if self.cond_stack.is_empty() {
                    return Err(ScriptError::UnbalancedConditional);
                }
                self.cond_stack.pop();
                Ok(())
            }
            Opcode::OP_VERIFY => {
                if self.stack.is_empty() {
                    return Err(ScriptError::VerifyFailed);
                }
                let value = self.stack.pop()?;
                if !self.is_true(&value) {
                    return Err(ScriptError::VerifyFailed);
                }
                Ok(())
            }
            Opcode::OP_RETURN => Err(ScriptError::VerifyFailed),

            // Stack operations
            Opcode::OP_TOALTSTACK => {
                let value = self.stack.pop()?;
                self.alt_stack.push(value)?;
                Ok(())
            }
            Opcode::OP_FROMALTSTACK => {
                let value = self.alt_stack.pop()?;
                self.stack.push(value)?;
                Ok(())
            }
            Opcode::OP_DROP => {
                self.stack.pop()?;
                Ok(())
            }
            Opcode::OP_DUP => {
                self.stack.dup()?;
                Ok(())
            }
            Opcode::OP_SWAP => {
                self.stack.swap()?;
                Ok(())
            }

            // Crypto operations
            Opcode::OP_RIPEMD160 => {
                let data = self.stack.pop()?;
                let mut hasher = Ripemd160::new();
                hasher.update(&data);
                let result = hasher.finalize();
                self.stack.push(result.to_vec())?;
                Ok(())
            }
            Opcode::OP_SHA256 => {
                let data = self.stack.pop()?;
                let mut hasher = Sha256::new();
                hasher.update(&data);
                let result = hasher.finalize();
                self.stack.push(result.to_vec())?;
                Ok(())
            }
            Opcode::OP_HASH160 => {
                let data = self.stack.pop()?;
                // SHA256 then RIPEMD160
                let mut sha = Sha256::new();
                sha.update(&data);
                let sha_result = sha.finalize();

                let mut ripemd = Ripemd160::new();
                ripemd.update(sha_result);
                let result = ripemd.finalize();
                self.stack.push(result.to_vec())?;
                Ok(())
            }
            Opcode::OP_HASH256 => {
                let data = self.stack.pop()?;
                // Double SHA256
                let mut sha1 = Sha256::new();
                sha1.update(&data);
                let result1 = sha1.finalize();

                let mut sha2 = Sha256::new();
                sha2.update(result1);
                let result2 = sha2.finalize();
                self.stack.push(result2.to_vec())?;
                Ok(())
            }

            // Comparison
            Opcode::OP_EQUAL => {
                if self.stack.len() < 2 {
                    return Err(ScriptError::StackUnderflow);
                }
                let b = self.stack.pop()?;
                let a = self.stack.pop()?;
                let equal = a == b;
                self.stack.push(if equal { vec![1] } else { vec![] })?;
                Ok(())
            }
            Opcode::OP_EQUALVERIFY => {
                if self.stack.len() < 2 {
                    return Err(ScriptError::StackUnderflow);
                }
                let b = self.stack.pop()?;
                let a = self.stack.pop()?;
                if a != b {
                    return Err(ScriptError::EqualVerifyFailed);
                }
                Ok(())
            }

            // Signature checking
            Opcode::OP_CHECKSIG => {
                if self.stack.len() < 2 {
                    return Err(ScriptError::StackUnderflow);
                }
                let pubkey = self.stack.pop()?;
                let signature = self.stack.pop()?;

                let valid = checker.check_signature(&signature, &pubkey)?;
                self.stack.push(if valid { vec![1] } else { vec![] })?;
                Ok(())
            }
            Opcode::OP_CHECKSIGVERIFY => {
                if self.stack.len() < 2 {
                    return Err(ScriptError::StackUnderflow);
                }
                let pubkey = self.stack.pop()?;
                let signature = self.stack.pop()?;

                if !checker.check_signature(&signature, &pubkey)? {
                    return Err(ScriptError::SignatureFailed);
                }
                Ok(())
            }

            _ => {
                // Unimplemented opcode
                Err(ScriptError::InvalidOpcode(opcode as u8))
            }
        }
    }

    /// Check if a stack value is true (non-zero)
    fn is_true(&self, value: &[u8]) -> bool {
        // Empty array is false
        if value.is_empty() {
            return false;
        }

        // Check if all bytes are zero (with potential negative zero)
        for (i, &byte) in value.iter().enumerate() {
            if byte != 0 {
                // Negative zero check (0x80 as last byte)
                if i == value.len() - 1 && byte == 0x80 {
                    return false;
                }
                return true;
            }
        }

        false
    }
}

/// Trait for signature verification
pub trait SignatureChecker {
    /// Check if a signature is valid for a public key
    fn check_signature(&self, signature: &[u8], pubkey: &[u8]) -> Result<bool, ScriptError>;
}

#[cfg(test)]
mod tests {
    use super::*;

    struct MockChecker;

    impl SignatureChecker for MockChecker {
        fn check_signature(&self, _signature: &[u8], _pubkey: &[u8]) -> Result<bool, ScriptError> {
            Ok(true) // Always valid for testing
        }
    }

    #[test]
    fn test_stack_operations() {
        let mut stack = ExecutionStack::new();

        // Test push/pop
        stack.push(vec![1, 2, 3]).unwrap();
        stack.push(vec![4, 5, 6]).unwrap();

        assert_eq!(stack.pop().unwrap(), vec![4, 5, 6]);
        assert_eq!(stack.pop().unwrap(), vec![1, 2, 3]);

        // Test underflow
        assert!(stack.pop().is_err());
    }

    #[test]
    fn test_script_execution() {
        let mut interpreter = ScriptInterpreter::new();
        let checker = MockChecker;

        // Simple script: push 1, push 1, OP_EQUAL
        let script = vec![0x51, 0x51, 0x87]; // OP_1 OP_1 OP_EQUAL

        let result = interpreter.execute(&script, &checker).unwrap();
        assert!(result);
    }

    #[test]
    fn test_hash_operations() {
        let mut interpreter = ScriptInterpreter::new();
        let checker = MockChecker;

        // Script: push data, OP_SHA256
        let script = vec![
            0x04, // Push 4 bytes
            0x01, 0x02, 0x03, 0x04, // Data
            0xa8, // OP_SHA256
        ];

        let result = interpreter.execute(&script, &checker).unwrap();
        assert!(result);

        // Check that we have a 32-byte hash on the stack
        assert_eq!(interpreter.stack.peek().unwrap().len(), 32);
    }
}
