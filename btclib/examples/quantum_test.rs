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
    
    println!("\nTesting non-implemented schemes");
    println!("==============================");
    
    // Test Falcon (not implemented)
    println!("\nTesting Falcon (not implemented):");
    let falcon_params = QuantumParameters::with_security_level(QuantumScheme::Falcon, SecurityLevel::Medium.into());
    match QuantumKeyPair::generate(&mut rng, falcon_params) {
        Ok(_) => println!("  Unexpectedly succeeded"),
        Err(e) => println!("  Expected error: {:?}", e),
    }
    
    // Test SPHINCS+ (not implemented)
    println!("\nTesting SPHINCS+ (not implemented):");
    let sphincs_params = QuantumParameters::with_security_level(QuantumScheme::Sphincs, SecurityLevel::Medium.into());
    match QuantumKeyPair::generate(&mut rng, sphincs_params) {
        Ok(_) => println!("  Unexpectedly succeeded"),
        Err(e) => println!("  Expected error: {:?}", e),
    }
    
    // Test Hybrid (not implemented)
    println!("\nTesting Hybrid (not implemented):");
    let hybrid_params = QuantumParameters::with_security_level(
        QuantumScheme::Hybrid(btclib::crypto::quantum::ClassicalScheme::Secp256k1), 
        SecurityLevel::Medium.into()
    );
    match QuantumKeyPair::generate(&mut rng, hybrid_params) {
        Ok(_) => println!("  Unexpectedly succeeded"),
        Err(e) => println!("  Expected error: {:?}", e),
    }
    
    println!("\nQuantum signature test completed!");
} 