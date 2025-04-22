# SuperNova Cryptographic Features

This document provides an overview of the advanced cryptographic features available in the SuperNova blockchain.

## Post-Quantum Cryptography

SuperNova includes support for post-quantum cryptographic algorithms to ensure the blockchain remains secure even if large-scale quantum computers become available. The implementation supports multiple quantum-resistant signature schemes.

### Supported Quantum-Resistant Schemes

- **CRYSTALS-Dilithium**: A lattice-based signature scheme selected for standardization by NIST (Fully implemented)
- **Falcon**: A lattice-based signature scheme with compact signatures (Fully implemented)
- **SPHINCS+**: A hash-based signature scheme with minimal security assumptions (Implementation in progress)
- **Hybrid Schemes**: Combinations of classical (e.g., secp256k1, ed25519) and quantum-resistant schemes (Implementation in progress)

> **Note:** Currently, only the Dilithium scheme is fully implemented. Other schemes (Falcon, SPHINCS+, and Hybrid) will return a `CryptoOperationFailed` error when used for signing or verification operations. Production-ready implementations for these schemes will be available in future releases.

### Usage Examples

#### Key Generation

```rust
use btclib::crypto::quantum::{QuantumKeyPair, QuantumParameters, QuantumScheme};
use rand::rngs::OsRng;

// Create parameters with Dilithium at medium security level
let params = QuantumParameters {
    security_level: 3,
    scheme: QuantumScheme::Dilithium,
    use_compression: false,
};

// Generate a quantum-resistant key pair
let keypair = QuantumKeyPair::generate(QuantumScheme::Dilithium, Some(params))
    .expect("Key generation failed");
```

#### Signing and Verification

```rust
// Sign a message
let message = b"Transaction data to sign";
let signature = keypair.sign(message).expect("Signing failed");

// Verify the signature
let verification_result = keypair.verify(message, &signature)
    .expect("Verification process failed");

if verification_result {
    println!("Signature is valid");
} else {
    println!("Signature is invalid");
}
```

#### Error Handling

```rust
// Attempting to use schemes that are not yet fully implemented
let falcon_params = QuantumParameters {
    security_level: 3,
    scheme: QuantumScheme::Falcon,
    use_compression: false,
};

let falcon_keypair = QuantumKeyPair::generate(QuantumScheme::Falcon, Some(falcon_params))
    .expect("Key generation failed");

// This will return a CryptoOperationFailed error
match falcon_keypair.sign(message) {
    Ok(signature) => println!("Signed successfully"),
    Err(QuantumError::CryptoOperationFailed(msg)) => {
        println!("Operation not yet implemented: {}", msg);
        // Handle the error appropriately
    },
    Err(e) => println!("Other error: {}", e),
}
```

## Zero-Knowledge Proof Systems

SuperNova includes a comprehensive implementation of zero-knowledge proof systems to enable privacy-preserving transactions and other advanced features.

### Supported Zero-Knowledge Proof Types

- **Range Proofs**: Prove that a value is within a specified range without revealing the value
- **Bulletproofs**: Compact range proofs with logarithmic size
- **Schnorr Proofs**: Proofs of knowledge for discrete logarithms
- **zk-SNARKs**: Succinct non-interactive arguments of knowledge for arbitrary computational statements

### Usage Examples

#### Creating Confidential Amounts

```rust
use btclib::crypto::zkp::{commit_pedersen, create_range_proof, ZkpParams, ZkpType};
use rand::rngs::OsRng;

let mut rng = OsRng;
let value = 1000u64; // The value to hide

// Create a Pedersen commitment to the value
let (commitment, blinding) = commit_pedersen(value, &mut rng);

// Create a range proof that the value is positive (fits in 64 bits)
let range_proof = create_range_proof(value, blinding, &mut rng);

// You can now attach this to a transaction
let confidential_tx = ConfidentialTransaction::new(
    1, // version
    inputs.iter().map(|(txid, _)| txid.clone()).collect(), // input txids
    commitments, // amount commitments
    proofs, // range proofs
    0, // locktime
);
```

## Performance Considerations

- Quantum-resistant signatures are generally larger and slower than classical signatures
- Zero-knowledge proofs require additional computation and increase transaction sizes
- Different schemes offer different tradeoffs between security, performance, and size

## Security Recommendations

1. For maximum future-proofing, use hybrid signatures combining classical and quantum-resistant schemes
2. For confidential transactions, use Bulletproofs for an optimal balance of proof size and verification speed
3. Ensure proper random number generation for all cryptographic operations
4. Keep blinding factors secure as they can be used to reveal hidden values
5. Use the highest security level that your application can tolerate in terms of performance

## Known Limitations

1. **Quantum Scheme Implementation Status:**
   - Only the Dilithium scheme is fully implemented
   - Falcon, SPHINCS+, and Hybrid schemes return `CryptoOperationFailed` errors
   - Full implementation of these schemes is planned for future releases

2. **Error Handling:**
   - Applications should properly handle `CryptoOperationFailed` errors when using non-Dilithium schemes
   - Error messages provide information about which scheme implementation is pending

## Future Enhancements

- Integration with advanced smart contract systems
- Multi-party computation protocols
- Verifiable delay functions
- Threshold signatures using post-quantum schemes
- Zero-knowledge virtual machines
- Complete implementation of Falcon, SPHINCS+, and Hybrid signature schemes

## Unified Signature Verification Layer

SuperNova provides a unified cryptographic abstraction layer that allows seamless integration of both classical and post-quantum signature schemes through a common interface.

### Key Features

- **Unified Interface**: Work with different signature schemes through a consistent API
- **Batch Verification**: Efficiently verify multiple signatures in parallel for any supported scheme
- **Pluggable Architecture**: Easily add new signature schemes without changing existing code
- **Type Safety**: Strong typing ensures correct usage of cryptographic primitives

### Usage Example

```rust
use btclib::crypto::signature::{SignatureVerifier, SignatureType};

// Create a signature verifier
let verifier = SignatureVerifier::new();

// Verify signatures using different schemes
let secp_result = verifier.verify(
    SignatureType::Secp256k1,
    &secp_public_key,
    &message,
    &secp_signature
);

let dilithium_result = verifier.verify(
    SignatureType::Dilithium,
    &dilithium_public_key,
    &message,
    &dilithium_signature
);

// Batch verification for improved performance
let batch_result = verifier.batch_verify(
    SignatureType::Dilithium,
    &[&public_key1, &public_key2, &public_key3],
    &[&message1, &message2, &message3],
    &[&signature1, &signature2, &signature3]
);

// Batch verification of transaction signatures
let batch_tx_result = verifier.batch_verify_transactions(&[&tx1, &tx2, &tx3]);
```

### Supported Signature Types

- **Classical Schemes**: 
  - `Secp256k1`: Used in Bitcoin and many other blockchains
  - `Ed25519`: Modern Edwards curve digital signature algorithm

- **Post-Quantum Schemes**:
  - `Dilithium`: CRYSTALS-Dilithium lattice-based signatures
  - `Falcon`: Compact lattice-based signatures
  - `Sphincs`: Hash-based signatures with minimal security assumptions
  - `Hybrid`: Combinations of classical and post-quantum signatures

### Performance Characteristics

Different signature schemes have different performance and size characteristics:

| Scheme | Public Key Size | Signature Size | Verification Speed | Security Assumptions |
|--------|----------------|----------------|--------------------|--------------------|
| Secp256k1 | 33 bytes | 64-65 bytes | Very fast | Discrete logarithm |
| Ed25519 | 32 bytes | 64 bytes | Very fast | Discrete logarithm |
| Dilithium (Medium) | 1,312 bytes | 2,420 bytes | Fast | Lattice (Module-LWE) |
| Falcon-512 | 897 bytes | ~666 bytes | Moderate | Lattice (NTRU) |
| SPHINCS+ | 32-64 bytes | 8-50 KB | Slow | Hash function |
| Hybrid | Sum of both | Sum of both | Depends on schemes | Multiple |

For applications that need to be quantum-resistant while maintaining reasonable signature sizes, Falcon is recommended due to its compact signatures. For maximum security with less concern for signature size, Dilithium or SPHINCS+ are good choices. 