#!/bin/bash

# Script to test and verify the environmental features implementation

set -e  # Exit on error

echo "Building the project..."
cargo build -p btclib

echo "Running environmental demo..."
cargo run --example environmental_demo

echo "Testing the Quantum signature implementation..."
# Build a simple test that uses the quantum signatures
cat << EOF > btclib/examples/quantum_test.rs
use btclib::crypto::quantum::{QuantumScheme, QuantumParameters, QuantumKeyPair};
use btclib::validation::SecurityLevel;
use rand::rngs::OsRng;

fn main() {
    println!("Testing Quantum Signature Implementation");
    println!("=======================================");
    
    let mut rng = OsRng;
    
    // Test different security levels
    let security_levels = [SecurityLevel::Low, SecurityLevel::Medium, SecurityLevel::High];
    
    for &level in &security_levels {
        println!("\nTesting with security level: {:?}", level);
        let level_value: u8 = level.into();
        
        let params = QuantumParameters::with_security_level(QuantumScheme::Dilithium, level_value);
        
        match QuantumKeyPair::generate(&mut rng, params) {
            Ok(keypair) => {
                println!("  Key generation successful");
                println!("  Public key length: {} bytes", keypair.public_key.len());
                
                // Test signing
                let message = b"This is a test message for quantum signing";
                match keypair.sign(message) {
                    Ok(signature) => {
                        println!("  Signature successful");
                        println!("  Signature length: {} bytes", signature.len());
                        
                        // Test verification
                        match keypair.verify(message, &signature) {
                            Ok(true) => println!("  Verification successful ✅"),
                            Ok(false) => println!("  Verification failed ❌"),
                            Err(e) => println!("  Verification error: {:?}", e),
                        }
                    },
                    Err(e) => println!("  Signing error: {:?}", e),
                }
            },
            Err(e) => println!("  Key generation error: {:?}", e),
        }
    }
    
    println!("\nQuantum signature test completed!");
}
EOF

cargo run --example quantum_test

echo "All tests completed!" 