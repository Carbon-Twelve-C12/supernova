//! Quantum Signature Test Vectors
//! 
//! This module provides comprehensive test vectors to verify the correctness
//! of quantum-resistant signature implementations in Supernova

use crate::types::extended_transaction::{QuantumTransaction, QuantumTransactionBuilder};
use crate::types::transaction::{Transaction, TransactionInput, TransactionOutput};
use crate::crypto::quantum::{QuantumScheme, ClassicalScheme, QuantumKeyPair, QuantumParameters};
use rand::thread_rng;

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_dilithium_signature_verification() {
        // Test all Dilithium security levels
        let security_levels = vec![2, 3, 5]; // Dilithium2, Dilithium3, Dilithium5
        
        for level in security_levels {
            println!("Testing Dilithium security level {}", level);
            
            // Generate key pair
            let params = QuantumParameters::with_security_level(QuantumScheme::Dilithium, level);
            let mut rng = thread_rng();
            let keypair = QuantumKeyPair::generate(&mut rng, params)
                .expect("Failed to generate Dilithium keypair");
            
            // Create a test transaction
            let tx = create_test_transaction();
            
            // Sign the transaction
            let builder = QuantumTransactionBuilder::new(QuantumScheme::Dilithium, level);
            let signed_tx = builder.sign_transaction(tx.clone(), &keypair.secret_key)
                .expect("Failed to sign transaction");
            
            // Verify the signature
            let is_valid = signed_tx.verify_signature(&keypair.public_key)
                .expect("Failed to verify signature");
            
            assert!(is_valid, "Dilithium{} signature should be valid", level);
            
            // Test with wrong public key
            let wrong_keypair = QuantumKeyPair::generate(&mut rng, params)
                .expect("Failed to generate wrong keypair");
            let is_invalid = signed_tx.verify_signature(&wrong_keypair.public_key)
                .expect("Failed to verify with wrong key");
            
            assert!(!is_invalid, "Dilithium{} signature should be invalid with wrong key", level);
            
            // Test with tampered signature
            let mut tampered_tx = signed_tx.clone();
            let mut tampered_sig = tampered_tx.signature().to_vec();
            tampered_sig[0] ^= 0xFF; // Flip bits in first byte
            tampered_tx = QuantumTransaction::new(
                tampered_tx.transaction().clone(),
                tampered_tx.scheme(),
                tampered_tx.security_level(),
                tampered_sig
            );
            
            let is_tampered_invalid = tampered_tx.verify_signature(&keypair.public_key)
                .expect("Failed to verify tampered signature");
            
            assert!(!is_tampered_invalid, "Dilithium{} tampered signature should be invalid", level);
        }
    }
    
    #[test]
    fn test_falcon_signature_verification() {
        // Test Falcon security levels
        let security_levels = vec![1, 5]; // Falcon-512, Falcon-1024
        
        for level in security_levels {
            println!("Testing Falcon security level {}", level);
            
            // Generate key pair
            let params = QuantumParameters::with_security_level(QuantumScheme::Falcon, level);
            let mut rng = thread_rng();
            let keypair = QuantumKeyPair::generate(&mut rng, params)
                .expect("Failed to generate Falcon keypair");
            
            // Create a test transaction
            let tx = create_test_transaction();
            
            // Sign the transaction
            let builder = QuantumTransactionBuilder::new(QuantumScheme::Falcon, level);
            let signed_tx = builder.sign_transaction(tx.clone(), &keypair.secret_key)
                .expect("Failed to sign transaction");
            
            // Verify the signature
            let is_valid = signed_tx.verify_signature(&keypair.public_key)
                .expect("Failed to verify signature");
            
            assert!(is_valid, "Falcon{} signature should be valid", if level == 1 { 512 } else { 1024 });
        }
    }
    
    #[test]
    fn test_sphincs_signature_verification() {
        println!("Testing SPHINCS+ signature");
        
        // SPHINCS+ with security level 1 (fast variant)
        let params = QuantumParameters::with_security_level(QuantumScheme::SphincsPlus, 1);
        let mut rng = thread_rng();
        let keypair = QuantumKeyPair::generate(&mut rng, params)
            .expect("Failed to generate SPHINCS+ keypair");
        
        // Create a test transaction
        let tx = create_test_transaction();
        
        // Sign the transaction
        let builder = QuantumTransactionBuilder::new(QuantumScheme::SphincsPlus, 1);
        let signed_tx = builder.sign_transaction(tx.clone(), &keypair.secret_key)
            .expect("Failed to sign transaction");
        
        // Verify the signature
        let is_valid = signed_tx.verify_signature(&keypair.public_key)
            .expect("Failed to verify signature");
        
        assert!(is_valid, "SPHINCS+ signature should be valid");
    }
    
    #[test]
    fn test_hybrid_secp256k1_dilithium_signature() {
        println!("Testing Hybrid Secp256k1+Dilithium signature");
        
        // Hybrid scheme with medium security
        let params = QuantumParameters::with_security_level(
            QuantumScheme::Hybrid(ClassicalScheme::Secp256k1), 
            3
        );
        let mut rng = thread_rng();
        let keypair = QuantumKeyPair::generate(&mut rng, params)
            .expect("Failed to generate hybrid keypair");
        
        // Create a test transaction
        let tx = create_test_transaction();
        
        // Sign the transaction
        let builder = QuantumTransactionBuilder::new(
            QuantumScheme::Hybrid(ClassicalScheme::Secp256k1), 
            3
        );
        let signed_tx = builder.sign_transaction(tx.clone(), &keypair.secret_key)
            .expect("Failed to sign transaction");
        
        // Verify the signature
        let is_valid = signed_tx.verify_signature(&keypair.public_key)
            .expect("Failed to verify signature");
        
        assert!(is_valid, "Hybrid Secp256k1+Dilithium signature should be valid");
        
        // Test with wrong key
        let wrong_keypair = QuantumKeyPair::generate(&mut rng, params)
            .expect("Failed to generate wrong keypair");
        let is_invalid = signed_tx.verify_signature(&wrong_keypair.public_key)
            .expect("Failed to verify with wrong key");
        
        assert!(!is_invalid, "Hybrid signature should be invalid with wrong key");
    }
    
    #[test]
    fn test_hybrid_ed25519_dilithium_signature() {
        println!("Testing Hybrid Ed25519+Dilithium signature");
        
        // Hybrid scheme with high security
        let params = QuantumParameters::with_security_level(
            QuantumScheme::Hybrid(ClassicalScheme::Ed25519), 
            5
        );
        let mut rng = thread_rng();
        let keypair = QuantumKeyPair::generate(&mut rng, params)
            .expect("Failed to generate hybrid keypair");
        
        // Create a test transaction
        let tx = create_test_transaction();
        
        // Sign the transaction
        let builder = QuantumTransactionBuilder::new(
            QuantumScheme::Hybrid(ClassicalScheme::Ed25519), 
            5
        );
        let signed_tx = builder.sign_transaction(tx.clone(), &keypair.secret_key)
            .expect("Failed to sign transaction");
        
        // Verify the signature
        let is_valid = signed_tx.verify_signature(&keypair.public_key)
            .expect("Failed to verify signature");
        
        assert!(is_valid, "Hybrid Ed25519+Dilithium signature should be valid");
    }
    
    #[test]
    fn test_signature_malleability_resistance() {
        println!("Testing signature malleability resistance");
        
        // Generate a Dilithium3 keypair
        let params = QuantumParameters::with_security_level(QuantumScheme::Dilithium, 3);
        let mut rng = thread_rng();
        let keypair = QuantumKeyPair::generate(&mut rng, params)
            .expect("Failed to generate keypair");
        
        // Create and sign a transaction
        let tx = create_test_transaction();
        let builder = QuantumTransactionBuilder::new(QuantumScheme::Dilithium, 3);
        let signed_tx = builder.sign_transaction(tx.clone(), &keypair.secret_key)
            .expect("Failed to sign transaction");
        
        // Try various signature manipulations
        let original_sig = signed_tx.signature().to_vec();
        
        // Test 1: Flip random bits
        for i in 0..10 {
            let mut modified_sig = original_sig.clone();
            modified_sig[i * 100] ^= 0x01;
            
            let modified_tx = QuantumTransaction::new(
                signed_tx.transaction().clone(),
                signed_tx.scheme(),
                signed_tx.security_level(),
                modified_sig
            );
            
            let is_valid = modified_tx.verify_signature(&keypair.public_key)
                .unwrap_or(false);
            
            assert!(!is_valid, "Modified signature should be invalid");
        }
        
        // Test 2: Truncate signature
        let truncated_sig = original_sig[..original_sig.len() - 10].to_vec();
        let truncated_tx = QuantumTransaction::new(
            signed_tx.transaction().clone(),
            signed_tx.scheme(),
            signed_tx.security_level(),
            truncated_sig
        );
        
        assert!(
            truncated_tx.verify_signature(&keypair.public_key).is_err(),
            "Truncated signature should cause error"
        );
    }
    
    #[test]
    fn test_cross_scheme_signature_rejection() {
        println!("Testing cross-scheme signature rejection");
        
        let mut rng = thread_rng();
        
        // Generate Dilithium keypair and sign
        let dilithium_params = QuantumParameters::with_security_level(QuantumScheme::Dilithium, 3);
        let dilithium_keypair = QuantumKeyPair::generate(&mut rng, dilithium_params)
            .expect("Failed to generate Dilithium keypair");
        
        let tx = create_test_transaction();
        let dilithium_builder = QuantumTransactionBuilder::new(QuantumScheme::Dilithium, 3);
        let dilithium_signed = dilithium_builder.sign_transaction(tx.clone(), &dilithium_keypair.secret_key)
            .expect("Failed to sign with Dilithium");
        
        // Generate Falcon keypair
        let falcon_params = QuantumParameters::with_security_level(QuantumScheme::Falcon, 1);
        let falcon_keypair = QuantumKeyPair::generate(&mut rng, falcon_params)
            .expect("Failed to generate Falcon keypair");
        
        // Try to verify Dilithium signature with Falcon public key
        // This should fail due to key format mismatch
        assert!(
            dilithium_signed.verify_signature(&falcon_keypair.public_key).is_err(),
            "Cross-scheme verification should fail"
        );
    }
    
    // Helper function to create a test transaction
    fn create_test_transaction() -> Transaction {
        let input = TransactionInput::new(
            [0u8; 32], // previous_tx_hash
            0,         // output_index
            vec![],    // script_sig
            0xFFFFFFFF // sequence
        );
        
        let output = TransactionOutput::new(
            1000000,        // 0.01 NOVA
            vec![0u8; 25]   // script_pubkey
        );
        
        Transaction::new(
            1,              // version
            vec![input],    // inputs
            vec![output],   // outputs
            0               // lock_time
        )
    }
} 