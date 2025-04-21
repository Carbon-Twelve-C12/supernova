use std::time::{Duration, Instant};
use std::str::FromStr;
use btclib::crypto::quantum::{
    QuantumScheme, QuantumKeyPair, QuantumSecurityLevel, QuantumError, QuantumSignature,
    ClassicalScheme
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("SuperNova Quantum Signatures Demo");
    println!("=================================\n");

    // Demonstrate Dilithium signatures at different security levels
    println!("DILITHIUM SCHEME DEMONSTRATION");
    println!("-----------------------------");
    demo_security_level(QuantumScheme::Dilithium, QuantumSecurityLevel::Low)?;
    demo_security_level(QuantumScheme::Dilithium, QuantumSecurityLevel::Medium)?;
    demo_security_level(QuantumScheme::Dilithium, QuantumSecurityLevel::High)?;

    // Compare with classical schemes
    println!("\nCLASSICAL SCHEME COMPARISON");
    println!("---------------------------");
    demo_classical_scheme(ClassicalScheme::Secp256k1)?;
    demo_classical_scheme(ClassicalScheme::Ed25519)?;

    // Demonstrate error handling for unsupported schemes
    println!("\nUNSUPPORTED SCHEME ERROR HANDLING");
    println!("--------------------------------");
    demo_unsupported_scheme()?;

    // Demonstrate verification failure
    println!("\nVERIFICATION FAILURE DEMONSTRATION");
    println!("---------------------------------");
    demo_verification_failure()?;

    Ok(())
}

fn demo_security_level(scheme: QuantumScheme, level: QuantumSecurityLevel) -> Result<(), Box<dyn std::error::Error>> {
    let scheme_name = format!("{:?}", scheme);
    let level_name = format!("{:?}", level);
    
    println!("\n{} - {} Security Level:", scheme_name, level_name);
    
    // Generate key pair and measure the time
    let start = Instant::now();
    let key_pair = QuantumKeyPair::generate(scheme, level)?;
    let key_gen_time = start.elapsed();
    
    println!("  Key Generation Time: {:?}", key_gen_time);
    println!("  Public Key Size: {} bytes", key_pair.public_key.len());
    println!("  Secret Key Size: {} bytes", key_pair.secret_key.len());
    
    // Sign a message and measure the time
    let message = b"SuperNova: The future of quantum-resistant blockchain technology";
    let start = Instant::now();
    let signature = key_pair.sign(message)?;
    let sign_time = start.elapsed();
    
    println!("  Signature Size: {} bytes", signature.signature.len());
    println!("  Signing Time: {:?}", sign_time);
    
    // Verify the signature
    let start = Instant::now();
    let valid = key_pair.verify(message, &signature)?;
    let verify_time = start.elapsed();
    
    println!("  Verification Time: {:?}", verify_time);
    println!("  Verification Result: {}", valid);
    
    // Performance ratios compared to typical classical signatures
    let classical_sign_time = Duration::from_micros(100); // Approximate time for secp256k1
    let ratio = sign_time.as_micros() as f64 / classical_sign_time.as_micros() as f64;
    
    println!("  Size Increase vs ECDSA: {}x", signature.signature.len() / 64);
    println!("  Signing Time Increase vs ECDSA: {:.1}x", ratio);
    
    Ok(())
}

fn demo_classical_scheme(scheme: ClassicalScheme) -> Result<(), Box<dyn std::error::Error>> {
    let scheme_name = format!("{:?}", scheme);
    
    println!("\n{} Classical Scheme:", scheme_name);
    
    // Generate key pair and measure the time
    let start = Instant::now();
    let key_pair = QuantumKeyPair::generate_classical(scheme)?;
    let key_gen_time = start.elapsed();
    
    println!("  Key Generation Time: {:?}", key_gen_time);
    println!("  Public Key Size: {} bytes", key_pair.public_key.len());
    println!("  Secret Key Size: {} bytes", key_pair.secret_key.len());
    
    // Sign a message and measure the time
    let message = b"SuperNova: The future of quantum-resistant blockchain technology";
    let start = Instant::now();
    let signature = key_pair.sign(message)?;
    let sign_time = start.elapsed();
    
    println!("  Signature Size: {} bytes", signature.signature.len());
    println!("  Signing Time: {:?}", sign_time);
    
    // Verify the signature
    let start = Instant::now();
    let valid = key_pair.verify(message, &signature)?;
    let verify_time = start.elapsed();
    
    println!("  Verification Time: {:?}", verify_time);
    println!("  Verification Result: {}", valid);
    
    Ok(())
}

fn demo_unsupported_scheme() -> Result<(), QuantumError> {
    println!("Attempting to use unsupported Falcon scheme...");
    
    match QuantumKeyPair::generate(QuantumScheme::Falcon, QuantumSecurityLevel::Medium) {
        Ok(_) => {
            println!("  Unexpectedly succeeded with unsupported scheme!");
            Ok(())
        },
        Err(err) => {
            println!("  Expected Error: {}", err);
            Ok(())
        }
    }
}

fn demo_verification_failure() -> Result<(), Box<dyn std::error::Error>> {
    println!("Testing verification with incorrect message...");
    
    // Generate key pair
    let key_pair = QuantumKeyPair::generate(QuantumScheme::Dilithium, QuantumSecurityLevel::Low)?;
    
    // Sign a message
    let original_message = b"Original message for signing";
    let signature = key_pair.sign(original_message)?;
    
    // Try to verify with a different message
    let wrong_message = b"Different message that wasn't signed";
    match key_pair.verify(wrong_message, &signature) {
        Ok(false) => println!("  ✓ Verification correctly failed for different message"),
        Ok(true) => println!("  ✗ Verification unexpectedly succeeded for different message!"),
        Err(e) => println!("  ✗ Verification threw an error: {}", e)
    }
    
    println!("\nTesting verification with incorrect key...");
    
    // Generate a different key pair
    let different_key_pair = QuantumKeyPair::generate(QuantumScheme::Dilithium, QuantumSecurityLevel::Low)?;
    
    // Try to verify with a different key
    match different_key_pair.verify(original_message, &signature) {
        Ok(false) => println!("  ✓ Verification correctly failed for different key"),
        Ok(true) => println!("  ✗ Verification unexpectedly succeeded for different key!"),
        Err(e) => println!("  ✗ Verification threw an error: {}", e)
    }
    
    // Try to verify with an invalid signature
    println!("\nTesting verification with corrupted signature...");
    let mut corrupted_signature = signature.clone();
    if let Some(byte) = corrupted_signature.signature.get_mut(0) {
        *byte ^= 0xFF; // Flip all bits in the first byte
    }
    
    match key_pair.verify(original_message, &corrupted_signature) {
        Ok(false) => println!("  ✓ Verification correctly failed for corrupted signature"),
        Ok(true) => println!("  ✗ Verification unexpectedly succeeded for corrupted signature!"),
        Err(e) => println!("  ✗ Verification threw an error: {}", e)
    }
    
    Ok(())
} 