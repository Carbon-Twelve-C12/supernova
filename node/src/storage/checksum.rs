//! Checksum utilities for storage integrity
//!
//! SECURITY MODULE (P1-007): Provides checksum functionality for detecting data corruption.
//!
//! This module provides:
//! - CRC32 checksums for fast integrity verification
//! - SHA256 checksums for cryptographic integrity
//! - Checksummed data wrappers for storage operations
//!
//! Usage:
//! ```rust
//! use supernova_node::storage::checksum::{ChecksummedData, calculate_crc32, verify_crc32};
//!
//! // Wrap data with checksum for storage
//! let data = vec![1, 2, 3, 4, 5];
//! let checksummed = ChecksummedData::new(data);
//!
//! // Serialize for storage
//! let bytes = checksummed.to_bytes();
//!
//! // Deserialize and verify
//! let recovered = ChecksummedData::from_bytes(&bytes)?;
//! assert!(recovered.verify());
//! ```

use crc32fast::Hasher;
use sha2::{Digest, Sha256};
use thiserror::Error;

/// Size of CRC32 checksum in bytes
pub const CRC32_SIZE: usize = 4;

/// Size of SHA256 checksum in bytes  
pub const SHA256_SIZE: usize = 32;

/// Errors related to checksum operations
#[derive(Debug, Error)]
pub enum ChecksumError {
    /// Data is too short to contain a checksum
    #[error("Data too short for checksum: expected at least {expected} bytes, got {actual}")]
    DataTooShort { expected: usize, actual: usize },

    /// Checksum verification failed - data is corrupted
    #[error("Checksum mismatch: data corrupted (expected {expected:08x}, got {actual:08x})")]
    ChecksumMismatch { expected: u32, actual: u32 },

    /// SHA256 checksum mismatch
    #[error("SHA256 checksum mismatch: data corrupted")]
    Sha256Mismatch,

    /// Invalid checksum format
    #[error("Invalid checksum format: {0}")]
    InvalidFormat(String),
}

// ============================================================================
// CRC32 Functions
// ============================================================================

/// Calculate CRC32 checksum for data
///
/// Uses the crc32fast implementation which is hardware-accelerated
/// on supported platforms (SSE 4.2 on x86, NEON on ARM).
#[inline]
pub fn calculate_crc32(data: &[u8]) -> u32 {
    let mut hasher = Hasher::new();
    hasher.update(data);
    hasher.finalize()
}

/// Verify CRC32 checksum
#[inline]
pub fn verify_crc32(data: &[u8], expected: u32) -> bool {
    calculate_crc32(data) == expected
}

/// Calculate CRC32 and return as byte array
#[inline]
pub fn calculate_crc32_bytes(data: &[u8]) -> [u8; CRC32_SIZE] {
    calculate_crc32(data).to_le_bytes()
}

/// Verify CRC32 from byte array
#[inline]
pub fn verify_crc32_bytes(data: &[u8], expected: &[u8; CRC32_SIZE]) -> bool {
    calculate_crc32_bytes(data) == *expected
}

// ============================================================================
// SHA256 Functions
// ============================================================================

/// Calculate SHA256 checksum for data
///
/// Use this for cryptographic integrity verification where collision
/// resistance is important (e.g., block hashes, merkle roots).
pub fn calculate_sha256(data: &[u8]) -> [u8; SHA256_SIZE] {
    let mut hasher = Sha256::new();
    hasher.update(data);
    let result = hasher.finalize();
    let mut output = [0u8; SHA256_SIZE];
    output.copy_from_slice(&result);
    output
}

/// Verify SHA256 checksum
pub fn verify_sha256(data: &[u8], expected: &[u8; SHA256_SIZE]) -> bool {
    calculate_sha256(data) == *expected
}

// ============================================================================
// Checksummed Data Wrapper
// ============================================================================

/// Wrapper for data with CRC32 checksum
///
/// This struct provides a convenient way to store and verify data integrity.
/// The checksum is appended to the data when serialized and verified on
/// deserialization.
#[derive(Debug, Clone)]
pub struct ChecksummedData {
    /// The actual data
    pub data: Vec<u8>,
    /// CRC32 checksum of the data
    pub checksum: u32,
}

impl ChecksummedData {
    /// Create new checksummed data, calculating the checksum automatically
    pub fn new(data: Vec<u8>) -> Self {
        let checksum = calculate_crc32(&data);
        Self { data, checksum }
    }

    /// Verify the data integrity
    pub fn verify(&self) -> bool {
        verify_crc32(&self.data, self.checksum)
    }

    /// Serialize to bytes with checksum appended
    ///
    /// Format: [data bytes][4-byte CRC32 checksum (little-endian)]
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut bytes = self.data.clone();
        bytes.extend_from_slice(&self.checksum.to_le_bytes());
        bytes
    }

    /// Deserialize from bytes and verify checksum
    ///
    /// Returns error if data is too short or checksum doesn't match.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, ChecksumError> {
        if bytes.len() < CRC32_SIZE {
            return Err(ChecksumError::DataTooShort {
                expected: CRC32_SIZE,
                actual: bytes.len(),
            });
        }

        let (data, checksum_bytes) = bytes.split_at(bytes.len() - CRC32_SIZE);
        let stored_checksum = u32::from_le_bytes(
            checksum_bytes
                .try_into()
                .map_err(|_| ChecksumError::InvalidFormat("Invalid checksum bytes".into()))?,
        );

        let computed_checksum = calculate_crc32(data);

        if stored_checksum != computed_checksum {
            return Err(ChecksumError::ChecksumMismatch {
                expected: stored_checksum,
                actual: computed_checksum,
            });
        }

        Ok(Self {
            data: data.to_vec(),
            checksum: stored_checksum,
        })
    }

    /// Get the raw data without checksum
    pub fn into_data(self) -> Vec<u8> {
        self.data
    }

    /// Get reference to the data
    pub fn data(&self) -> &[u8] {
        &self.data
    }

    /// Get the checksum value
    pub fn checksum(&self) -> u32 {
        self.checksum
    }

    /// Get the total size (data + checksum)
    pub fn total_size(&self) -> usize {
        self.data.len() + CRC32_SIZE
    }
}

// ============================================================================
// Storage-Specific Checksum Utilities
// ============================================================================

/// Verify block data integrity
///
/// For blocks, we use CRC32 for fast verification on read, and the
/// block hash (which is SHA256-based) provides cryptographic verification.
pub fn verify_block_checksum(serialized_block: &[u8], stored_checksum: u32) -> bool {
    verify_crc32(serialized_block, stored_checksum)
}

/// Calculate checksum for block storage
pub fn calculate_block_checksum(serialized_block: &[u8]) -> u32 {
    calculate_crc32(serialized_block)
}

/// Verify UTXO entry integrity
pub fn verify_utxo_checksum(serialized_utxo: &[u8], stored_checksum: u32) -> bool {
    verify_crc32(serialized_utxo, stored_checksum)
}

/// Calculate checksum for UTXO storage
pub fn calculate_utxo_checksum(serialized_utxo: &[u8]) -> u32 {
    calculate_crc32(serialized_utxo)
}

// ============================================================================
// Streaming Checksum Calculator
// ============================================================================

/// Streaming checksum calculator for large data
///
/// Use this when data is too large to fit in memory at once.
/// ```rust
/// let mut calculator = StreamingChecksum::new();
/// calculator.update(&chunk1);
/// calculator.update(&chunk2);
/// let checksum = calculator.finalize();
/// ```
pub struct StreamingChecksum {
    hasher: Hasher,
    bytes_processed: usize,
}

impl StreamingChecksum {
    /// Create a new streaming checksum calculator
    pub fn new() -> Self {
        Self {
            hasher: Hasher::new(),
            bytes_processed: 0,
        }
    }

    /// Update with more data
    pub fn update(&mut self, data: &[u8]) {
        self.hasher.update(data);
        self.bytes_processed += data.len();
    }

    /// Finalize and return the checksum
    pub fn finalize(self) -> u32 {
        self.hasher.finalize()
    }

    /// Get the number of bytes processed
    pub fn bytes_processed(&self) -> usize {
        self.bytes_processed
    }

    /// Reset the calculator for reuse
    pub fn reset(&mut self) {
        self.hasher = Hasher::new();
        self.bytes_processed = 0;
    }
}

impl Default for StreamingChecksum {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_crc32_basic() {
        let data = b"hello world";
        let checksum = calculate_crc32(data);
        assert!(verify_crc32(data, checksum));
        assert!(!verify_crc32(b"hello worle", checksum));
    }

    #[test]
    fn test_crc32_empty() {
        let data = b"";
        let checksum = calculate_crc32(data);
        assert!(verify_crc32(data, checksum));
    }

    #[test]
    fn test_sha256_basic() {
        let data = b"hello world";
        let checksum = calculate_sha256(data);
        assert!(verify_sha256(data, &checksum));
        assert!(!verify_sha256(b"hello worle", &checksum));
    }

    #[test]
    fn test_checksummed_data_roundtrip() {
        let original = vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10];
        let checksummed = ChecksummedData::new(original.clone());
        assert!(checksummed.verify());

        let bytes = checksummed.to_bytes();
        assert_eq!(bytes.len(), original.len() + CRC32_SIZE);

        let recovered = ChecksummedData::from_bytes(&bytes).unwrap();
        assert_eq!(recovered.data, original);
        assert!(recovered.verify());
    }

    #[test]
    fn test_checksummed_data_corruption_detected() {
        let original = vec![1, 2, 3, 4, 5];
        let checksummed = ChecksummedData::new(original);
        let mut bytes = checksummed.to_bytes();

        // Corrupt a data byte
        bytes[0] ^= 0xFF;

        let result = ChecksummedData::from_bytes(&bytes);
        assert!(matches!(result, Err(ChecksumError::ChecksumMismatch { .. })));
    }

    #[test]
    fn test_checksummed_data_too_short() {
        let result = ChecksummedData::from_bytes(&[1, 2, 3]);
        assert!(matches!(result, Err(ChecksumError::DataTooShort { .. })));
    }

    #[test]
    fn test_streaming_checksum() {
        let data = b"hello world, this is a test of streaming checksum";

        // Full calculation
        let expected = calculate_crc32(data);

        // Streaming calculation
        let mut streaming = StreamingChecksum::new();
        streaming.update(&data[..10]);
        streaming.update(&data[10..30]);
        streaming.update(&data[30..]);
        let actual = streaming.finalize();

        assert_eq!(expected, actual);
    }

    #[test]
    fn test_streaming_checksum_bytes_processed() {
        let mut streaming = StreamingChecksum::new();
        streaming.update(&[1, 2, 3]);
        streaming.update(&[4, 5]);
        assert_eq!(streaming.bytes_processed(), 5);
    }

    #[test]
    fn test_block_checksum_functions() {
        let block_data = vec![0u8; 1000];
        let checksum = calculate_block_checksum(&block_data);
        assert!(verify_block_checksum(&block_data, checksum));
    }

    #[test]
    fn test_utxo_checksum_functions() {
        let utxo_data = vec![1, 2, 3, 4, 5, 6, 7, 8];
        let checksum = calculate_utxo_checksum(&utxo_data);
        assert!(verify_utxo_checksum(&utxo_data, checksum));
    }
}

