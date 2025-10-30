//! Key Encapsulation Mechanism (KEM) for post-quantum key exchange
//!
//! This module provides post-quantum secure key encapsulation using
//! NIST-approved ML-KEM (Kyber) algorithm.
//!
//! SECURITY FIX (P0-001): Replaced placeholder implementation with actual
//! pqcrypto-kyber implementation. This provides quantum-resistant key exchange
//! for P2P communication layer.

use pqcrypto_kyber::kyber768;
use pqcrypto_traits::kem::{PublicKey, SecretKey, Ciphertext, SharedSecret};
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// KEM errors
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum KemError {
    #[error("Invalid public key: {0}")]
    InvalidPublicKey(String),

    #[error("Invalid ciphertext: {0}")]
    InvalidCiphertext(String),

    #[error("Decapsulation failed: {0}")]
    DecapsulationFailed(String),

    #[error("Key generation failed: {0}")]
    KeyGenerationFailed(String),

    #[error("Serialization error: {0}")]
    SerializationError(String),
}

/// KEM key pair
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KemKeyPair {
    pub public_key: Vec<u8>,
    pub secret_key: Vec<u8>,
}

impl KemKeyPair {
    /// Generate a new KEM key pair using ML-KEM (Kyber-768)
    ///
    /// SECURITY FIX (P0-001): Implements actual Kyber-768 key generation
    /// instead of placeholder random bytes. Uses NIST-standardized ML-KEM
    /// for quantum-resistant key exchange.
    ///
    /// # Returns
    /// * `Ok(KemKeyPair)` - Successfully generated key pair
    /// * `Err(KemError)` - Key generation failed
    pub fn generate() -> Result<Self, KemError> {
        // Generate Kyber-768 keypair (NIST Level 3 security)
        let (public_key, secret_key) = kyber768::keypair();

        Ok(Self {
            public_key: public_key.as_bytes().to_vec(),
            secret_key: secret_key.as_bytes().to_vec(),
        })
    }

    /// Get public key bytes
    pub fn public_key_bytes(&self) -> &[u8] {
        &self.public_key
    }

    /// Get secret key bytes
    pub fn secret_key_bytes(&self) -> &[u8] {
        &self.secret_key
    }
}

/// Encapsulate a shared secret using ML-KEM (Kyber-768)
///
/// SECURITY FIX (P0-001): Implements actual Kyber-768 encapsulation
/// instead of placeholder random bytes. This provides quantum-resistant
/// key exchange for P2P communication.
///
/// # Arguments
/// * `public_key` - Recipient's public key bytes
///
/// # Returns
/// * `Ok((ciphertext, shared_secret))` - Successfully encapsulated
/// * `Err(KemError)` - Encapsulation failed
///
/// # Attack Vector Mitigation
/// Original placeholder generated random bytes without cryptographic security.
/// This fix ensures actual ML-KEM encapsulation provides IND-CCA2 security.
pub fn encapsulate(public_key: &[u8]) -> Result<(Vec<u8>, Vec<u8>), KemError> {
    // Validate public key size (Kyber-768 public key is 1184 bytes)
    if public_key.len() != kyber768::public_key_bytes() {
        return Err(KemError::InvalidPublicKey(format!(
            "Expected {} bytes, got {}",
            kyber768::public_key_bytes(),
            public_key.len()
        )));
    }

    // Parse public key
    let pk = kyber768::PublicKey::from_bytes(public_key)
        .map_err(|e| KemError::InvalidPublicKey(format!("Failed to parse public key: {:?}", e)))?;

    // Encapsulate shared secret
    let (ciphertext, shared_secret) = kyber768::encapsulate(&pk);

    Ok((
        ciphertext.as_bytes().to_vec(),
        shared_secret.as_bytes().to_vec(),
    ))
}

/// Decapsulate a shared secret using ML-KEM (Kyber-768)
///
/// SECURITY FIX (P0-001): Implements actual Kyber-768 decapsulation
/// instead of placeholder random bytes. This provides quantum-resistant
/// key exchange for P2P communication.
///
/// # Arguments
/// * `secret_key` - Recipient's secret key bytes
/// * `ciphertext` - Encapsulated ciphertext bytes
///
/// # Returns
/// * `Ok(shared_secret)` - Successfully decapsulated
/// * `Err(KemError)` - Decapsulation failed (invalid ciphertext or key)
///
/// # Attack Vector Mitigation
/// Original placeholder generated random bytes without cryptographic security.
/// This fix ensures actual ML-KEM decapsulation provides IND-CCA2 security.
pub fn decapsulate(secret_key: &[u8], ciphertext: &[u8]) -> Result<Vec<u8>, KemError> {
    // Validate secret key size (Kyber-768 secret key is 2400 bytes)
    if secret_key.len() != kyber768::secret_key_bytes() {
        return Err(KemError::InvalidCiphertext(format!(
            "Invalid secret key size: expected {} bytes, got {}",
            kyber768::secret_key_bytes(),
            secret_key.len()
        )));
    }

    // Validate ciphertext size (Kyber-768 ciphertext is 1088 bytes)
    if ciphertext.len() != kyber768::ciphertext_bytes() {
        return Err(KemError::InvalidCiphertext(format!(
            "Invalid ciphertext size: expected {} bytes, got {}",
            kyber768::ciphertext_bytes(),
            ciphertext.len()
        )));
    }

    // Parse secret key
    let sk = kyber768::SecretKey::from_bytes(secret_key)
        .map_err(|e| KemError::InvalidCiphertext(format!("Failed to parse secret key: {:?}", e)))?;

    // Parse ciphertext
    let ct = kyber768::Ciphertext::from_bytes(ciphertext)
        .map_err(|e| KemError::InvalidCiphertext(format!("Failed to parse ciphertext: {:?}", e)))?;

    // Decapsulate shared secret
    let shared_secret = kyber768::decapsulate(&ct, &sk);

    Ok(shared_secret.as_bytes().to_vec())
}

#[cfg(test)]
mod tests {
    use super::*;
    use pqcrypto_kyber::kyber768;

#[test]
fn test_kem_key_generation() {
    // SECURITY FIX (P0-001): Verify key generation produces valid Kyber-768 keys
    let keypair = KemKeyPair::generate().expect("Key generation should succeed");

    // Verify key sizes match Kyber-768 specification
    assert_eq!(
        keypair.public_key.len(),
        kyber768::public_key_bytes(),
        "Public key size must match Kyber-768 specification"
    );
    assert_eq!(
        keypair.secret_key.len(),
        kyber768::secret_key_bytes(),
        "Secret key size must match Kyber-768 specification"
    );

    // Verify keys are not all zeros (were actually generated)
    assert!(
        keypair.public_key.iter().any(|&b| b != 0),
        "Public key should not be all zeros"
    );
    assert!(
        keypair.secret_key.iter().any(|&b| b != 0),
        "Secret key should not be all zeros"
    );
}

#[test]
fn test_kem_encapsulation_decapsulation() {
    // SECURITY FIX (P0-001): Verify encapsulation/decapsulation works correctly
    let keypair = KemKeyPair::generate().expect("Key generation should succeed");

    // Encapsulate shared secret
    let (ciphertext, shared_secret1) = encapsulate(&keypair.public_key)
        .expect("Encapsulation should succeed");

    // Verify ciphertext size
    assert_eq!(
        ciphertext.len(),
        kyber768::ciphertext_bytes(),
        "Ciphertext size must match Kyber-768 specification"
    );

    // Verify shared secret size (Kyber-768 produces 32 bytes)
    assert_eq!(
        shared_secret1.len(),
        kyber768::shared_secret_bytes(),
        "Shared secret size must match Kyber-768 specification"
    );

    // Decapsulate shared secret
    let shared_secret2 = decapsulate(&keypair.secret_key, &ciphertext)
        .expect("Decapsulation should succeed");

    // CRITICAL: Shared secrets must match (this proves actual cryptographic security)
    assert_eq!(
        shared_secret1, shared_secret2,
        "Shared secrets must match - this proves actual ML-KEM is working"
    );
}

#[test]
fn test_kem_multiple_encapsulations() {
    // SECURITY FIX (P0-001): Verify each encapsulation produces different ciphertexts
    // but same decapsulation works
    let keypair = KemKeyPair::generate().expect("Key generation should succeed");

    let (ciphertext1, shared_secret1) = encapsulate(&keypair.public_key)
        .expect("First encapsulation should succeed");
    let (ciphertext2, shared_secret2) = encapsulate(&keypair.public_key)
        .expect("Second encapsulation should succeed");

    // Each encapsulation should produce different ciphertexts (randomness)
    assert_ne!(
        ciphertext1, ciphertext2,
        "Each encapsulation should produce different ciphertext"
    );

    // But shared secrets should also be different (IND-CCA2 security)
    assert_ne!(
        shared_secret1, shared_secret2,
        "Each encapsulation should produce different shared secret"
    );

    // Both ciphertexts should decrypt correctly
    let decapsulated1 = decapsulate(&keypair.secret_key, &ciphertext1)
        .expect("First decapsulation should succeed");
    let decapsulated2 = decapsulate(&keypair.secret_key, &ciphertext2)
        .expect("Second decapsulation should succeed");

    assert_eq!(shared_secret1, decapsulated1);
    assert_eq!(shared_secret2, decapsulated2);
}

#[test]
fn test_kem_invalid_public_key() {
    // SECURITY FIX (P0-001): Verify invalid public keys are rejected
    let invalid_key = vec![0u8; 100]; // Wrong size

    let result = encapsulate(&invalid_key);
    assert!(
        result.is_err(),
        "Encapsulation with invalid public key should fail"
    );

    match result {
        Err(KemError::InvalidPublicKey(_)) => {}
        _ => panic!("Expected InvalidPublicKey error"),
    }
}

#[test]
fn test_kem_invalid_ciphertext() {
    // SECURITY FIX (P0-001): Verify invalid ciphertexts are rejected
    let keypair = KemKeyPair::generate().expect("Key generation should succeed");

    // Test with wrong ciphertext size
    let invalid_ciphertext = vec![0u8; 100];
    let result = decapsulate(&keypair.secret_key, &invalid_ciphertext);
    assert!(
        result.is_err(),
        "Decapsulation with invalid ciphertext size should fail"
    );

    // Test with wrong secret key size
    let invalid_secret_key = vec![0u8; 100];
    let (valid_ciphertext, _) = encapsulate(&keypair.public_key)
        .expect("Encapsulation should succeed");
    let result = decapsulate(&invalid_secret_key, &valid_ciphertext);
    assert!(
        result.is_err(),
        "Decapsulation with invalid secret key size should fail"
    );
}

#[test]
fn test_kem_corrupted_ciphertext() {
    // SECURITY FIX (P0-001): Verify corrupted ciphertexts produce different shared secret
    // This proves actual cryptographic security (not just random bytes)
    let keypair = KemKeyPair::generate().expect("Key generation should succeed");
    let (ciphertext, original_shared_secret) = encapsulate(&keypair.public_key)
        .expect("Encapsulation should succeed");

    // Corrupt ciphertext
    let mut corrupted_ciphertext = ciphertext.clone();
    corrupted_ciphertext[0] ^= 0xFF; // Flip bits

    // Decapsulation should either fail or produce different shared secret
    let result = decapsulate(&keypair.secret_key, &corrupted_ciphertext);
    
    // ML-KEM should produce different shared secret (not same as original)
    if let Ok(corrupted_shared_secret) = result {
        assert_ne!(
            original_shared_secret,
            corrupted_shared_secret,
            "Corrupted ciphertext should produce different shared secret"
        );
    }
    // It's also acceptable for decapsulation to fail entirely
}

#[test]
fn test_kem_wrong_key_pair() {
    // SECURITY FIX (P0-001): Verify wrong keypair cannot decrypt ciphertext
    // This proves actual cryptographic security
    let keypair1 = KemKeyPair::generate().expect("Key generation should succeed");
    let keypair2 = KemKeyPair::generate().expect("Key generation should succeed");

    let (ciphertext, shared_secret1) = encapsulate(&keypair1.public_key)
        .expect("Encapsulation should succeed");

    // Try to decrypt with wrong secret key
    let result = decapsulate(&keypair2.secret_key, &ciphertext);
    
    // Should produce different shared secret (or fail)
    if let Ok(shared_secret2) = result {
        assert_ne!(
            shared_secret1,
            shared_secret2,
            "Wrong secret key should produce different shared secret"
        );
    }
    // It's also acceptable for decapsulation to fail entirely
}

#[test]
fn test_kem_serialization() {
    // SECURITY FIX (P0-001): Verify keypair can be serialized/deserialized
    use bincode;

    let keypair = KemKeyPair::generate().expect("Key generation should succeed");

    // Serialize
    let serialized = bincode::serialize(&keypair)
        .expect("Serialization should succeed");

    // Deserialize
    let deserialized: KemKeyPair = bincode::deserialize(&serialized)
        .expect("Deserialization should succeed");

    // Verify keys match
    assert_eq!(keypair.public_key, deserialized.public_key);
    assert_eq!(keypair.secret_key, deserialized.secret_key);

    // Verify deserialized keypair still works
    let (ciphertext, shared_secret1) = encapsulate(&deserialized.public_key)
        .expect("Encapsulation should succeed");
    let shared_secret2 = decapsulate(&deserialized.secret_key, &ciphertext)
        .expect("Decapsulation should succeed");

    assert_eq!(shared_secret1, shared_secret2);
}

#[test]
fn test_kem_key_independence() {
    // SECURITY FIX (P0-001): Verify each key generation produces different keys
    let keypair1 = KemKeyPair::generate().expect("Key generation should succeed");
    let keypair2 = KemKeyPair::generate().expect("Key generation should succeed");

    // Keys should be different (high probability)
    assert_ne!(
        keypair1.public_key, keypair2.public_key,
        "Each key generation should produce different public keys"
    );
    assert_ne!(
        keypair1.secret_key, keypair2.secret_key,
        "Each key generation should produce different secret keys"
    );
}

#[test]
fn test_kem_shared_secret_randomness() {
    // SECURITY FIX (P0-001): Verify shared secrets have sufficient entropy
    // Multiple encapsulations should produce different shared secrets
    let keypair = KemKeyPair::generate().expect("Key generation should succeed");

    let mut shared_secrets = Vec::new();
    for _ in 0..10 {
        let (_, shared_secret) = encapsulate(&keypair.public_key)
            .expect("Encapsulation should succeed");
        shared_secrets.push(shared_secret);
    }

    // All shared secrets should be different (with high probability)
    for i in 0..shared_secrets.len() {
        for j in (i + 1)..shared_secrets.len() {
            assert_ne!(
                shared_secrets[i], shared_secrets[j],
                "Multiple encapsulations should produce different shared secrets"
            );
        }
    }
}

#[test]
fn test_kem_stress_test() {
    // SECURITY FIX (P0-001): Stress test to verify implementation stability
    const ITERATIONS: usize = 100;

    for _ in 0..ITERATIONS {
        let keypair = KemKeyPair::generate().expect("Key generation should succeed");
        let (ciphertext, shared_secret1) = encapsulate(&keypair.public_key)
            .expect("Encapsulation should succeed");
        let shared_secret2 = decapsulate(&keypair.secret_key, &ciphertext)
            .expect("Decapsulation should succeed");

        assert_eq!(
            shared_secret1, shared_secret2,
            "Stress test: Shared secrets must match on iteration"
        );
    }
}

#[test]
fn test_kem_key_bytes_accessors() {
    // Verify accessor methods work correctly
    let keypair = KemKeyPair::generate().expect("Key generation should succeed");

    assert_eq!(
        keypair.public_key_bytes(),
        &keypair.public_key,
        "public_key_bytes() should return reference to public_key"
    );
    assert_eq!(
        keypair.secret_key_bytes(),
        &keypair.secret_key,
        "secret_key_bytes() should return reference to secret_key"
    );
}
}
