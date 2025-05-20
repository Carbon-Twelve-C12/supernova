use sha2::{Sha256, Digest};
use std::fmt;

/// Hash type for blockchain operations
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct Hash([u8; 32]);

impl Hash {
    /// Create a new hash with all zeros
    pub fn zero() -> Self {
        Self([0u8; 32])
    }

    /// Create a hash from a byte array
    pub fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    /// Get the bytes of this hash
    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }

    /// Hash some data using SHA-256
    pub fn sha256<T: AsRef<[u8]>>(data: T) -> Self {
        let mut hasher = Sha256::new();
        hasher.update(data);
        let result = hasher.finalize();
        
        let mut bytes = [0u8; 32];
        bytes.copy_from_slice(&result);
        Self(bytes)
    }

    /// Double-SHA256 hash (common in Bitcoin)
    pub fn double_sha256<T: AsRef<[u8]>>(data: T) -> Self {
        let first_hash = Self::sha256(data);
        Self::sha256(first_hash.0)
    }

    /// Compare the first n bits of two hashes
    pub fn compare_bits(&self, other: &Self, bits: usize) -> bool {
        let bytes = bits / 8;
        let remainder = bits % 8;

        // Check full bytes
        for i in 0..bytes {
            if self.0[i] != other.0[i] {
                return false;
            }
        }

        // Check remaining bits
        if remainder > 0 {
            let mask = 0xFF_u8 << (8 - remainder);
            if (self.0[bytes] & mask) != (other.0[bytes] & mask) {
                return false;
            }
        }

        true
    }

    /// Check if this hash is less than the given target
    pub fn is_below_target(&self, target: &Self) -> bool {
        for i in 0..32 {
            if self.0[i] < target.0[i] {
                return true;
            } else if self.0[i] > target.0[i] {
                return false;
            }
        }
        true // Equal to target
    }
}

impl fmt::Debug for Hash {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Hash({})", hex::encode(self.0))
    }
}

impl fmt::Display for Hash {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", hex::encode(self.0))
    }
}

impl From<[u8; 32]> for Hash {
    fn from(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }
}

impl AsRef<[u8]> for Hash {
    fn as_ref(&self) -> &[u8] {
        &self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sha256() {
        let hash = Hash::sha256("hello world");
        assert_eq!(
            hash.to_string(),
            "b94d27b9934d3e08a52e52d7da7dabfac484efe37a5380ee9088f7ace2efcde9"
        );
    }

    #[test]
    fn test_double_sha256() {
        let hash = Hash::double_sha256("hello world");
        let expected = Hash::sha256(Hash::sha256("hello world").as_bytes());
        assert_eq!(hash, expected);
    }

    #[test]
    fn test_compare_bits() {
        let hash1 = Hash::from_bytes([0xFF; 32]);
        let hash2 = Hash::from_bytes([0xFF; 32]);
        assert!(hash1.compare_bits(&hash2, 256));

        let mut hash3 = Hash::from_bytes([0xFF; 32]);
        hash3.0[31] = 0xFE;
        assert!(hash1.compare_bits(&hash3, 255));
        assert!(!hash1.compare_bits(&hash3, 256));
    }

    #[test]
    fn test_is_below_target() {
        let lower = Hash::from_bytes([0x00, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00]);
        let higher = Hash::from_bytes([0x00, 0x02, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00]);
        
        assert!(lower.is_below_target(&higher));
        assert!(!higher.is_below_target(&lower));
    }
} 