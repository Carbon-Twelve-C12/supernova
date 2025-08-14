//! Secure Fork Resolution V2 - Following Bitcoin's Proven Approach
//! 
//! This module implements fork resolution based on accumulated proof-of-work,
//! the fundamental security mechanism of Nakamoto Consensus.

use std::cmp::Ordering;
use thiserror::Error;
use crate::types::block::BlockHeader;

/// Fork resolution errors
#[derive(Debug, Error)]
pub enum ForkResolutionError {
    #[error("Block not found: {0}")]
    BlockNotFound(String),
    
    #[error("Invalid difficulty: {0}")]
    InvalidDifficulty(u32),
    
    #[error("Chain traversal depth exceeded")]
    DepthExceeded,
}

/// Result type for fork resolution
pub type ForkResolutionResult<T> = Result<T, ForkResolutionError>;

/// Fork resolution based purely on accumulated proof-of-work
pub struct ProofOfWorkForkResolver {
    /// Maximum depth to traverse when calculating chainwork
    max_depth: u32,
}

impl ProofOfWorkForkResolver {
    pub fn new(max_depth: u32) -> Self {
        Self { max_depth }
    }
    
    /// Compare two chains based on accumulated proof-of-work
    /// Returns Ordering::Greater if chain_a has more work, Less if chain_b has more work
    pub fn compare_chains(
        &self,
        chain_a_tip: &[u8; 32],
        chain_b_tip: &[u8; 32],
        get_header: impl Fn(&[u8; 32]) -> Option<BlockHeader>,
    ) -> ForkResolutionResult<Ordering> {
        let work_a = self.calculate_chainwork(chain_a_tip, &get_header)?;
        let work_b = self.calculate_chainwork(chain_b_tip, &get_header)?;
        
        Ok(work_a.cmp(&work_b))
    }
    
    /// Calculate total accumulated work for a chain
    fn calculate_chainwork(
        &self,
        tip_hash: &[u8; 32],
        get_header: &impl Fn(&[u8; 32]) -> Option<BlockHeader>,
    ) -> ForkResolutionResult<U256> {
        let mut current_hash = *tip_hash;
        let mut total_work = U256::zero();
        let mut depth = 0;
        
        loop {
            if depth >= self.max_depth {
                return Err(ForkResolutionError::DepthExceeded);
            }
            
            let header = get_header(&current_hash)
                .ok_or_else(|| ForkResolutionError::BlockNotFound(hex::encode(current_hash)))?;
            
            // Calculate work for this block: work = 2^256 / (target + 1)
            let block_work = self.calculate_block_work(header.bits())?;
            total_work = total_work.saturating_add(block_work);
            
            // Stop at genesis
            if header.prev_block_hash() == &[0; 32] {
                break;
            }
            
            current_hash = *header.prev_block_hash();
            depth += 1;
        }
        
        Ok(total_work)
    }
    
    /// Calculate work for a single block based on its difficulty target
    fn calculate_block_work(&self, bits: u32) -> ForkResolutionResult<U256> {
        let target = self.bits_to_target(bits)?;
        
        // Work = 2^256 / (target + 1)
        // To avoid division by zero, we ensure target is valid
        if target == U256::max_value() {
            return Ok(U256::one()); // Minimum work
        }
        
        let max_target = U256::from_be_bytes([0xff; 32]);
        let work = max_target / (target + U256::one());
        
        Ok(work)
    }
    
    /// Convert compact difficulty bits to 256-bit target
    fn bits_to_target(&self, bits: u32) -> ForkResolutionResult<U256> {
        let exponent = ((bits >> 24) & 0xFF) as usize;
        let mantissa = bits & 0x00FFFFFF;
        
        // Validate difficulty
        if mantissa > 0x7fffff || exponent > 34 || (mantissa != 0 && exponent == 0) {
            return Err(ForkResolutionError::InvalidDifficulty(bits));
        }
        
        let mut target = [0u8; 32];
        
        if exponent <= 3 {
            let value = mantissa >> (8 * (3 - exponent));
            target[31] = value as u8;
            if value > 0xff {
                target[30] = (value >> 8) as u8;
            }
            if value > 0xffff {
                target[29] = (value >> 16) as u8;
            }
        } else {
            let byte_offset = exponent - 3;
            if byte_offset < 30 {
                target[32 - byte_offset - 1] = mantissa as u8;
                target[32 - byte_offset - 2] = (mantissa >> 8) as u8;
                target[32 - byte_offset - 3] = (mantissa >> 16) as u8;
            }
        }
        
        Ok(U256::from_be_bytes(target))
    }
}

/// 256-bit unsigned integer for chainwork calculations
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct U256([u64; 4]); // Little-endian u64 array

impl U256 {
    pub fn zero() -> Self {
        U256([0; 4])
    }
    
    pub fn one() -> Self {
        U256([1, 0, 0, 0])
    }
    
    pub fn max_value() -> Self {
        U256([u64::MAX; 4])
    }
    
    pub fn from_be_bytes(bytes: [u8; 32]) -> Self {
        let mut words = [0u64; 4];
        for i in 0..4 {
            let mut word_bytes = [0u8; 8];
            word_bytes.copy_from_slice(&bytes[24 - i * 8..32 - i * 8]);
            words[i] = u64::from_be_bytes(word_bytes);
        }
        U256(words)
    }
    
    pub fn saturating_add(self, rhs: Self) -> Self {
        let mut result = [0u64; 4];
        let mut carry = 0u64;
        
        for i in 0..4 {
            let (sum1, carry1) = self.0[i].overflowing_add(rhs.0[i]);
            let (sum2, carry2) = sum1.overflowing_add(carry);
            result[i] = sum2;
            carry = (carry1 as u64) + (carry2 as u64);
        }
        
        U256(result)
    }
}

// Simplified division for work calculation
impl std::ops::Div for U256 {
    type Output = Self;
    
    fn div(self, rhs: Self) -> Self::Output {
        // Simplified division - in production, use a proper big integer library
        // For now, we'll use a basic implementation that works for our use case
        if rhs == Self::zero() {
            panic!("Division by zero");
        }
        
        // For the specific case of max_target / (target + 1), we can approximate
        // This is sufficient for fork resolution comparison
        let mut quotient = Self::zero();
        let mut remainder = self;
        
        // Simple bit-shift division
        for bit_pos in (0..256).rev() {
            let mut test_divisor = rhs;
            
            // Shift divisor left by bit_pos
            for _ in 0..bit_pos {
                test_divisor = test_divisor.shift_left_one();
                if test_divisor > remainder {
                    break;
                }
            }
            
            if test_divisor <= remainder {
                remainder = remainder.saturating_sub(test_divisor);
                quotient = quotient.set_bit(bit_pos);
            }
        }
        
        quotient
    }
}

impl std::ops::Add for U256 {
    type Output = Self;
    
    fn add(self, rhs: Self) -> Self::Output {
        self.saturating_add(rhs)
    }
}

impl U256 {
    fn shift_left_one(self) -> Self {
        let mut result = [0u64; 4];
        let mut carry = 0u64;
        
        for i in 0..4 {
            let new_carry = self.0[i] >> 63;
            result[i] = (self.0[i] << 1) | carry;
            carry = new_carry;
        }
        
        U256(result)
    }
    
    fn set_bit(mut self, bit: usize) -> Self {
        let word = bit / 64;
        let bit_in_word = bit % 64;
        if word < 4 {
            self.0[word] |= 1u64 << bit_in_word;
        }
        self
    }
    
    fn saturating_sub(self, rhs: Self) -> Self {
        let mut result = [0u64; 4];
        let mut borrow = 0u64;
        
        for i in 0..4 {
            let (diff1, borrow1) = self.0[i].overflowing_sub(rhs.0[i]);
            let (diff2, borrow2) = diff1.overflowing_sub(borrow);
            result[i] = diff2;
            borrow = (borrow1 as u64) + (borrow2 as u64);
        }
        
        U256(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_bits_to_target() {
        let resolver = ProofOfWorkForkResolver::new(1000);
        
        // Test cases from Bitcoin
        let test_cases = vec![
            (0x1d00ffff, "00000000ffff0000000000000000000000000000000000000000000000000000"),
            (0x1b0404cb, "00000000000404cb000000000000000000000000000000000000000000000000"),
            (0x04123456, "0000000000000000000000000000000000000000000000000000000000123456"),
        ];
        
        for (bits, expected_hex) in test_cases {
            let target = resolver.bits_to_target(bits).unwrap();
            let mut expected_bytes = [0u8; 32];
            hex::decode_to_slice(expected_hex, &mut expected_bytes).unwrap();
            assert_eq!(target, U256::from_be_bytes(expected_bytes));
        }
    }
    
    #[test]
    fn test_chainwork_accumulation() {
        let resolver = ProofOfWorkForkResolver::new(10);
        
        // Create test headers with different difficulties
        let mut headers = std::collections::HashMap::new();
        
        // Genesis (easy difficulty)
        let genesis = BlockHeader::new(0, [0; 32], [0; 32], 0, 0x1f7fffff, 0);
        headers.insert([0; 32], genesis);
        
        // Block 1 (harder difficulty)
        let block1_hash = [1; 32];
        let block1 = BlockHeader::new(1, [0; 32], [0; 32], 600, 0x1d00ffff, 0);
        headers.insert(block1_hash, block1);
        
        // Block 2 (even harder)
        let block2_hash = [2; 32];
        let block2 = BlockHeader::new(2, block1_hash, [0; 32], 1200, 0x1c00ffff, 0);
        headers.insert(block2_hash, block2);
        
        let get_header = |hash: &[u8; 32]| headers.get(hash).cloned();
        
        // Compare chains - chain with block 2 should have more work
        let ordering = resolver.compare_chains(&block2_hash, &block1_hash, get_header).unwrap();
        assert_eq!(ordering, Ordering::Greater);
    }
}
