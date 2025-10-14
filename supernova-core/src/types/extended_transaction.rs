use crate::crypto::quantum::{ClassicalScheme, QuantumError, QuantumParameters, QuantumScheme};
use crate::crypto::zkp::{Commitment, ZeroKnowledgeProof, ZkpParams};
use crate::types::transaction::{Transaction, TransactionInput};
use pqcrypto_traits::sign::{
    DetachedSignature as SignDetachedSignatureTrait, PublicKey as SignPublicKeyTrait,
    SecretKey as SignSecretKeyTrait,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// Extended transaction input with support for quantum signatures
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtendedTransactionInput {
    /// Base transaction input
    input: TransactionInput,
    /// Signature scheme used
    signature_scheme: Option<QuantumScheme>,
}

/// Confidential transaction output that hides the amount
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfidentialOutput {
    /// Commitment to the amount (instead of revealing it)
    amount_commitment: Commitment,
    /// Range proof proving the amount is positive
    range_proof: ZeroKnowledgeProof,
    /// Public key script as in regular transactions
    pub_key_script: Vec<u8>,
}

/// Transaction with quantum signature support
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuantumTransaction {
    /// Base transaction
    transaction: Transaction,
    /// Quantum signature scheme used
    scheme: QuantumScheme,
    /// Security level used
    security_level: u8,
    /// The signature data
    signature: Vec<u8>,
}

/// Transaction with confidential amounts
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfidentialTransaction {
    /// Version number
    version: u32,
    /// Transaction inputs
    inputs: Vec<TransactionInput>,
    /// Confidential outputs with hidden amounts
    conf_outputs: Vec<ConfidentialOutput>,
    /// Lock time
    lock_time: u32,
}

impl ExtendedTransactionInput {
    pub fn new(input: TransactionInput, signature_scheme: Option<QuantumScheme>) -> Self {
        Self {
            input,
            signature_scheme,
        }
    }

    pub fn input(&self) -> &TransactionInput {
        &self.input
    }

    pub fn signature_scheme(&self) -> Option<QuantumScheme> {
        self.signature_scheme
    }
}

impl ConfidentialOutput {
    pub fn new(
        amount_commitment: Commitment,
        range_proof: ZeroKnowledgeProof,
        pub_key_script: Vec<u8>,
    ) -> Self {
        Self {
            amount_commitment,
            range_proof,
            pub_key_script,
        }
    }

    pub fn amount_commitment(&self) -> &Commitment {
        &self.amount_commitment
    }

    pub fn range_proof(&self) -> &ZeroKnowledgeProof {
        &self.range_proof
    }

    pub fn pub_key_script(&self) -> &[u8] {
        &self.pub_key_script
    }
}

impl QuantumTransaction {
    pub fn new(
        transaction: Transaction,
        scheme: QuantumScheme,
        security_level: u8,
        signature: Vec<u8>,
    ) -> Self {
        Self {
            transaction,
            scheme,
            security_level,
            signature,
        }
    }

    pub fn transaction(&self) -> &Transaction {
        &self.transaction
    }

    pub fn scheme(&self) -> QuantumScheme {
        self.scheme
    }

    pub fn security_level(&self) -> u8 {
        self.security_level
    }

    pub fn signature(&self) -> &[u8] {
        &self.signature
    }

    /// Verify the quantum signature for this transaction
    ///
    /// # Arguments
    /// * `public_key` - The public key to verify the signature against
    ///
    /// # Returns
    /// * `Result<bool, QuantumError>` - Whether the signature is valid or an error
    ///
    /// # Security considerations
    /// This method implements verification logic for quantum-resistant signatures according to NIST standards.
    /// The actual implementation differs based on the chosen scheme, with different security properties:
    /// - Dilithium: Lattice-based with formal security reductions to hard problems
    /// - Falcon: Compact signatures with fast verification
    /// - SPHINCS+: Hash-based with minimal cryptographic assumptions
    /// - Hybrid: Combines classical and quantum security for stronger guarantees
    ///
    /// The implementation ensures no timing side-channels are exposed during verification.
    pub fn verify_signature(&self, public_key: &[u8]) -> Result<bool, QuantumError> {
        if public_key.is_empty() {
            return Err(QuantumError::InvalidKey("Public key is empty".to_string()));
        }

        let tx_hash = self.transaction.hash();

        // Create parameters for verification
        let params = QuantumParameters {
            security_level: self.security_level,
            scheme: self.scheme,
        };

        // Call the appropriate quantum verification function
        match self.scheme {
            QuantumScheme::Dilithium => {
                // Validate key length based on security level
                let expected_key_len = match self.security_level {
                    2 => 1312, // Dilithium2
                    3 => 1952, // Dilithium3
                    5 => 2592, // Dilithium5
                    _ => {
                        return Err(QuantumError::InvalidKey(
                            "Invalid security level for Dilithium".to_string(),
                        ))
                    }
                };

                if public_key.len() != expected_key_len {
                    return Err(QuantumError::InvalidKey(format!(
                        "Invalid Dilithium public key length: expected {}, got {}",
                        expected_key_len,
                        public_key.len()
                    )));
                }

                // Verify signature length
                let expected_sig_len = match self.security_level {
                    2 => 2420, // Dilithium2
                    3 => 3293, // Dilithium3
                    5 => 4595, // Dilithium5
                    _ => {
                        return Err(QuantumError::InvalidSignature(
                            "Invalid security level".into(),
                        ))
                    }
                };

                if self.signature.len() != expected_sig_len {
                    return Err(QuantumError::InvalidSignature(
                        "Invalid signature length".into(),
                    ));
                }

                // Use the Dilithium verification based on security level
                match self.security_level {
                    2 => {
                        use pqcrypto_dilithium::dilithium2;
                        let pk = dilithium2::PublicKey::from_bytes(public_key).map_err(|_| {
                            QuantumError::InvalidKey("Invalid Dilithium2 public key".to_string())
                        })?;
                        let sig = dilithium2::DetachedSignature::from_bytes(&self.signature)
                            .map_err(|_| {
                                QuantumError::InvalidSignature(
                                    "Invalid Dilithium2 signature".into(),
                                )
                            })?;
                        Ok(dilithium2::verify_detached_signature(&sig, &tx_hash, &pk).is_ok())
                    }
                    3 => {
                        use pqcrypto_dilithium::dilithium3;
                        let pk = dilithium3::PublicKey::from_bytes(public_key).map_err(|_| {
                            QuantumError::InvalidKey("Invalid Dilithium3 public key".to_string())
                        })?;
                        let sig = dilithium3::DetachedSignature::from_bytes(&self.signature)
                            .map_err(|_| {
                                QuantumError::InvalidSignature(
                                    "Invalid Dilithium3 signature".into(),
                                )
                            })?;
                        Ok(dilithium3::verify_detached_signature(&sig, &tx_hash, &pk).is_ok())
                    }
                    5 => {
                        use pqcrypto_dilithium::dilithium5;
                        let pk = dilithium5::PublicKey::from_bytes(public_key).map_err(|_| {
                            QuantumError::InvalidKey("Invalid Dilithium5 public key".to_string())
                        })?;
                        let sig = dilithium5::DetachedSignature::from_bytes(&self.signature)
                            .map_err(|_| {
                                QuantumError::InvalidSignature(
                                    "Invalid Dilithium5 signature".into(),
                                )
                            })?;
                        Ok(dilithium5::verify_detached_signature(&sig, &tx_hash, &pk).is_ok())
                    }
                    _ => Err(QuantumError::InvalidKey(
                        "Invalid security level for Dilithium".to_string(),
                    )),
                }
            }
            QuantumScheme::Falcon => {
                // Validate key length based on security level
                let expected_key_len = match self.security_level {
                    1 => 897,  // Falcon-512 public key
                    5 => 1793, // Falcon-1024 public key
                    _ => {
                        return Err(QuantumError::InvalidKey(
                            "Invalid security level for Falcon".to_string(),
                        ))
                    }
                };

                if public_key.len() != expected_key_len {
                    return Err(QuantumError::InvalidKey(format!(
                        "Invalid Falcon public key length: expected {}, got {}",
                        expected_key_len,
                        public_key.len()
                    )));
                }

                // Use our quantum module for Falcon verification
                use crate::crypto::quantum::QuantumKeyPair;

                let params = QuantumParameters {
                    scheme: QuantumScheme::Falcon,
                    security_level: self.security_level,
                };

                let keypair = QuantumKeyPair {
                    public_key: public_key.to_vec(),
                    secret_key: vec![], // Not needed for verification
                    parameters: params,
                };

                keypair.verify(&tx_hash, &self.signature).map_err(|e| {
                    QuantumError::VerificationFailed(format!("Falcon verification failed: {}", e))
                })
            }
            QuantumScheme::SphincsPlus => {
                // Validate key length based on security level
                let expected_key_len = match self.security_level {
                    1 => 32, // SPHINCS+-128f
                    3 => 48, // SPHINCS+-192f
                    5 => 64, // SPHINCS+-256f
                    _ => {
                        return Err(QuantumError::InvalidKey(
                            "Invalid security level for SPHINCS+".to_string(),
                        ))
                    }
                };

                if public_key.len() != expected_key_len {
                    return Err(QuantumError::InvalidKey(format!(
                        "Invalid SPHINCS+ public key length: expected {}, got {}",
                        expected_key_len,
                        public_key.len()
                    )));
                }

                // Use the SPHINCS+ verification
                use pqcrypto_sphincsplus::sphincsshake128fsimple;

                let pk =
                    sphincsshake128fsimple::PublicKey::from_bytes(public_key).map_err(|_| {
                        QuantumError::InvalidKey("Invalid SPHINCS+ public key".to_string())
                    })?;
                let sig = sphincsshake128fsimple::DetachedSignature::from_bytes(&self.signature)
                    .map_err(|_| {
                        QuantumError::InvalidSignature("Invalid SPHINCS+ signature".into())
                    })?;
                Ok(sphincsshake128fsimple::verify_detached_signature(&sig, &tx_hash, &pk).is_ok())
            }
            QuantumScheme::Hybrid(classical_scheme) => {
                // Split signature into quantum and classical parts
                if self.signature.len() < 64 {
                    // Minimum expected signature size
                    return Err(QuantumError::InvalidSignature(
                        "Hybrid signature too short".into(),
                    ));
                }

                // Parse the hybrid signature format
                // Format: [classical_sig_len (2 bytes)][classical_sig][quantum_sig]
                if self.signature.len() < 2 {
                    return Err(QuantumError::InvalidSignature(
                        "Invalid hybrid signature format".into(),
                    ));
                }

                let classical_sig_len =
                    u16::from_be_bytes([self.signature[0], self.signature[1]]) as usize;
                if self.signature.len() < 2 + classical_sig_len {
                    return Err(QuantumError::InvalidSignature(
                        "Invalid hybrid signature length".into(),
                    ));
                }

                let classical_sig = &self.signature[2..2 + classical_sig_len];
                let quantum_sig = &self.signature[2 + classical_sig_len..];

                // Parse the hybrid public key format
                // Format: [classical_pk_len (2 bytes)][classical_pk][quantum_pk]
                if public_key.len() < 2 {
                    return Err(QuantumError::InvalidKey(
                        "Invalid hybrid public key format".to_string(),
                    ));
                }

                let classical_pk_len = u16::from_be_bytes([public_key[0], public_key[1]]) as usize;
                if public_key.len() < 2 + classical_pk_len {
                    return Err(QuantumError::InvalidKey(
                        "Invalid hybrid public key length".to_string(),
                    ));
                }

                let classical_pk = &public_key[2..2 + classical_pk_len];
                let quantum_pk = &public_key[2 + classical_pk_len..];

                // Verify classical signature
                let classical_valid = match classical_scheme {
                    ClassicalScheme::Secp256k1 => {
                        use secp256k1::{ecdsa::Signature, Message, PublicKey, Secp256k1};

                        let secp = Secp256k1::verification_only();
                        let msg = Message::from_slice(&tx_hash).map_err(|_| {
                            QuantumError::InvalidSignature(
                                "Invalid message hash for secp256k1".into(),
                            )
                        })?;
                        let pk = PublicKey::from_slice(classical_pk).map_err(|_| {
                            QuantumError::InvalidKey("Invalid secp256k1 public key".to_string())
                        })?;
                        let sig = Signature::from_compact(classical_sig).map_err(|_| {
                            QuantumError::InvalidSignature("Invalid secp256k1 signature".into())
                        })?;

                        secp.verify_ecdsa(&msg, &sig, &pk).is_ok()
                    }
                    ClassicalScheme::Ed25519 => {
                        use ed25519_dalek::{Signature, VerifyingKey};

                        let pk =
                            VerifyingKey::from_bytes(classical_pk.try_into().map_err(|_| {
                                QuantumError::InvalidKey(
                                    "Invalid Ed25519 public key length".to_string(),
                                )
                            })?)
                            .map_err(|_| {
                                QuantumError::InvalidKey("Invalid Ed25519 public key".to_string())
                            })?;
                        let sig =
                            Signature::from_bytes(classical_sig.try_into().map_err(|_| {
                                QuantumError::InvalidSignature(
                                    "Invalid Ed25519 signature length".into(),
                                )
                            })?);

                        pk.verify_strict(&tx_hash, &sig).is_ok()
                    }
                };

                if !classical_valid {
                    return Ok(false);
                }

                // Verify quantum signature (assuming Dilithium for quantum part)
                // Determine security level from quantum signature length
                let quantum_security_level = match quantum_sig.len() {
                    2420 => 2, // Dilithium2
                    3293 => 3, // Dilithium3
                    4595 => 5, // Dilithium5
                    _ => {
                        return Err(QuantumError::InvalidSignature(
                            "Invalid quantum signature length in hybrid signature".into(),
                        ))
                    }
                };

                // Create a temporary quantum transaction for verification
                let quantum_tx = QuantumTransaction {
                    transaction: self.transaction.clone(),
                    scheme: QuantumScheme::Dilithium,
                    security_level: quantum_security_level,
                    signature: quantum_sig.to_vec(),
                };

                quantum_tx.verify_signature(quantum_pk)
            }
        }
    }
}

impl ConfidentialTransaction {
    pub fn new(
        version: u32,
        inputs: Vec<TransactionInput>,
        conf_outputs: Vec<ConfidentialOutput>,
        lock_time: u32,
    ) -> Self {
        Self {
            version,
            inputs,
            conf_outputs,
            lock_time,
        }
    }

    pub fn hash(&self) -> [u8; 32] {
        let serialized = bincode::serialize(&self).unwrap();
        let mut hasher = Sha256::new();
        hasher.update(&serialized);
        let result = hasher.finalize();
        let mut hash = [0u8; 32];
        hash.copy_from_slice(&result);
        hash
    }

    pub fn inputs(&self) -> &[TransactionInput] {
        &self.inputs
    }

    pub fn conf_outputs(&self) -> &[ConfidentialOutput] {
        &self.conf_outputs
    }

    /// Verify all range proofs in the transaction
    ///
    /// # Returns
    /// * `bool` - Whether all range proofs are valid
    ///
    /// # Security considerations
    /// Range proofs are critical for confidential transactions as they ensure:
    /// 1. All output amounts are positive, preventing negative value creation
    /// 2. No integer overflow will occur in amount calculations
    /// 3. The prover knows the actual values without revealing them
    ///
    /// Bulletproofs provide succinct zero-knowledge range proofs with:
    /// - Sub-linear proof size: O(log n) rather than O(n)
    /// - No trusted setup requirement
    /// - Post-quantum security when used with appropriate hash functions
    ///
    /// This verification process checks that all transaction outputs contain
    /// valid range proofs without leaking any information about the actual amounts.
    pub fn verify_range_proofs(&self) -> bool {
        if self.conf_outputs.is_empty() {
            return false; // Transaction must have outputs
        }

        // Maximum number of outputs to verify (for DoS protection)
        const MAX_OUTPUTS: usize = 10000;

        if self.conf_outputs.len() > MAX_OUTPUTS {
            return false;
        }

        // Verify each output's range proof
        for output in &self.conf_outputs {
            // 1. Check the proof exists
            if output.range_proof.proof.is_empty() {
                return false;
            }

            // 2. Check that commitment exists and has appropriate size
            if output.amount_commitment.value.is_empty()
                || output.amount_commitment.value.len() != 32
            {
                // Ristretto point is 32 bytes
                return false;
            }

            // 3. Verify the range proof matches the commitment
            let valid = crate::crypto::zkp::verify_range_proof(
                &output.amount_commitment,
                &output.range_proof,
                64, // 64-bit range proof (0 to 2^64-1)
            );

            if !valid {
                return false;
            }
        }

        // 4. For a complete implementation, also verify:
        //    - Sum of inputs equals sum of outputs plus fee
        //    - This involves homomorphic commitment operations
        //    - Using the property: C(a) + C(b) = C(a+b)
        //
        // This would be implemented using Pedersen commitments:
        // TODO: In production, implement:
        // sum(input_commitments) == sum(output_commitments) + C(fee)

        true
    }
}

/// Factory for creating quantum-secured transactions
pub struct QuantumTransactionBuilder {
    scheme: QuantumScheme,
    security_level: u8,
}

impl QuantumTransactionBuilder {
    pub fn new(scheme: QuantumScheme, security_level: u8) -> Self {
        Self {
            scheme,
            security_level,
        }
    }

    /// Sign a transaction with a quantum-resistant signature
    ///
    /// # Arguments
    /// * `transaction` - The transaction to sign
    /// * `private_key` - The quantum-resistant private key used for signing
    ///
    /// # Returns
    /// * `Result<QuantumTransaction, QuantumError>` - The signed transaction or an error
    ///
    /// # Security considerations
    /// This method implements quantum-resistant signing according to NIST post-quantum standards.
    /// The generated signatures provide the following security guarantees:
    /// - EUF-CMA (Existential Unforgeability under Chosen Message Attack) security
    /// - Forward security against quantum computer attacks
    /// - Protection against side-channel leakage during signing through constant-time operations
    ///
    /// Private keys should never be stored in plaintext and should be protected using
    /// appropriate key management practices to prevent key extraction attacks.
    pub fn sign_transaction(
        &self,
        transaction: Transaction,
        private_key: &[u8],
    ) -> Result<QuantumTransaction, QuantumError> {
        // Validate private key
        if private_key.is_empty() {
            return Err(QuantumError::InvalidKey("Private key is empty".to_string()));
        }

        // Validate private key length based on scheme and security level
        let expected_key_len = match (self.scheme, self.security_level) {
            // Dilithium key sizes
            (QuantumScheme::Dilithium, 2) => 2528, // Dilithium2 secret key
            (QuantumScheme::Dilithium, 3) => 4000, // Dilithium3 secret key
            (QuantumScheme::Dilithium, 5) => 4864, // Dilithium5 secret key

            // Falcon key sizes
            (QuantumScheme::Falcon, 1) => 1281, // Falcon-512 secret key
            (QuantumScheme::Falcon, 5) => 2305, // Falcon-1024 secret key

            // SPHINCS+ key sizes
            (QuantumScheme::SphincsPlus, 1) => 64, // SPHINCS+-128f secret key
            (QuantumScheme::SphincsPlus, 3) => 96, // SPHINCS+-192f secret key
            (QuantumScheme::SphincsPlus, 5) => 128, // SPHINCS+-256f secret key

            // Hybrid schemes combine classical and quantum keys
            (QuantumScheme::Hybrid(ClassicalScheme::Secp256k1), _) => {
                // 32 bytes for secp256k1 + quantum key length
                32 + match self.security_level {
                    2 => 2528, // Dilithium2 + secp256k1
                    3 => 4000, // Dilithium3 + secp256k1
                    5 => 4864, // Dilithium5 + secp256k1
                    _ => {
                        return Err(QuantumError::InvalidKey(
                            "Invalid security level for hybrid Secp256k1 scheme".to_string(),
                        ))
                    }
                }
            }
            (QuantumScheme::Hybrid(ClassicalScheme::Ed25519), _) => {
                // 32 bytes for Ed25519 + quantum key length
                32 + match self.security_level {
                    2 => 2528, // Dilithium2 + Ed25519
                    3 => 4000, // Dilithium3 + Ed25519
                    5 => 4864, // Dilithium5 + Ed25519
                    _ => {
                        return Err(QuantumError::InvalidKey(
                            "Invalid security level for hybrid Ed25519 scheme".to_string(),
                        ))
                    }
                }
            }

            // Invalid security level
            _ => {
                return Err(QuantumError::InvalidKey(format!(
                    "Invalid combination of scheme {:?} and security level {}",
                    self.scheme, self.security_level
                )))
            }
        };

        // Check if the provided key has the expected length
        if private_key.len() != expected_key_len {
            return Err(QuantumError::InvalidKey(format!(
                "Invalid private key length: expected {}, got {}",
                expected_key_len,
                private_key.len()
            )));
        }

        // Get the transaction hash to sign
        let tx_hash = transaction.hash();

        // Generate the signature based on the scheme
        let signature = match self.scheme {
            QuantumScheme::Dilithium => {
                // Use the actual Dilithium signing based on security level
                match self.security_level {
                    2 => {
                        use pqcrypto_dilithium::dilithium2;
                        let sk =
                            <dilithium2::SecretKey as SignSecretKeyTrait>::from_bytes(private_key)
                                .map_err(|_| {
                                    QuantumError::InvalidKey(
                                        "Invalid Dilithium2 secret key".to_string(),
                                    )
                                })?;
                        let sig = dilithium2::detached_sign(&tx_hash, &sk);
                        sig.as_bytes().to_vec()
                    }
                    3 => {
                        use pqcrypto_dilithium::dilithium3;
                        let sk =
                            <dilithium3::SecretKey as SignSecretKeyTrait>::from_bytes(private_key)
                                .map_err(|_| {
                                    QuantumError::InvalidKey(
                                        "Invalid Dilithium3 secret key".to_string(),
                                    )
                                })?;
                        let sig = dilithium3::detached_sign(&tx_hash, &sk);
                        sig.as_bytes().to_vec()
                    }
                    5 => {
                        use pqcrypto_dilithium::dilithium5;
                        let sk =
                            <dilithium5::SecretKey as SignSecretKeyTrait>::from_bytes(private_key)
                                .map_err(|_| {
                                    QuantumError::InvalidKey(
                                        "Invalid Dilithium5 secret key".to_string(),
                                    )
                                })?;
                        let sig = dilithium5::detached_sign(&tx_hash, &sk);
                        sig.as_bytes().to_vec()
                    }
                    _ => return Err(QuantumError::UnsupportedSecurityLevel(self.security_level)),
                }
            }
            QuantumScheme::Falcon => {
                // Use our quantum module for Falcon signing
                use crate::crypto::quantum::QuantumKeyPair;

                let params = QuantumParameters {
                    scheme: QuantumScheme::Falcon,
                    security_level: self.security_level,
                };

                let keypair = QuantumKeyPair {
                    public_key: vec![], // Not needed for signing
                    secret_key: private_key.to_vec(),
                    parameters: params,
                };

                keypair.sign(&tx_hash).map_err(|e| {
                    QuantumError::SigningFailed(format!("Falcon signing failed: {}", e))
                })?
            }
            QuantumScheme::SphincsPlus => {
                // Use the actual SPHINCS+ signing
                use pqcrypto_sphincsplus::sphincsshake128fsimple;

                let sk = <sphincsshake128fsimple::SecretKey as SignSecretKeyTrait>::from_bytes(
                    private_key,
                )
                .map_err(|_| QuantumError::InvalidKey("Invalid SPHINCS+ secret key".to_string()))?;
                let sig = sphincsshake128fsimple::detached_sign(&tx_hash, &sk);
                sig.as_bytes().to_vec()
            }
            QuantumScheme::Hybrid(classical_scheme) => {
                // For hybrid schemes, we need to:
                // 1. Split the private key into classical and quantum parts
                // 2. Sign with both keys
                // 3. Combine the signatures

                match classical_scheme {
                    ClassicalScheme::Secp256k1 => {
                        // Extract keys
                        let classical_sk = &private_key[0..32];
                        let quantum_sk = &private_key[32..];

                        // Sign with secp256k1
                        use secp256k1::{Message, Secp256k1, SecretKey};

                        let secp = Secp256k1::signing_only();
                        let msg = Message::from_slice(&tx_hash).map_err(|_| {
                            QuantumError::SigningFailed(
                                "Invalid message hash for secp256k1".to_string(),
                            )
                        })?;
                        let sk = SecretKey::from_slice(classical_sk).map_err(|_| {
                            QuantumError::InvalidKey("Invalid secp256k1 secret key".to_string())
                        })?;
                        let sig = secp.sign_ecdsa(&msg, &sk);
                        let classical_sig = sig.serialize_compact();

                        // Sign with quantum algorithm (Dilithium)
                        let quantum_sig = match self.security_level {
                            2 => {
                                use pqcrypto_dilithium::dilithium2;
                                let sk = <dilithium2::SecretKey as SignSecretKeyTrait>::from_bytes(
                                    quantum_sk,
                                )
                                .map_err(|_| {
                                    QuantumError::InvalidKey(
                                        "Invalid Dilithium2 secret key in hybrid".to_string(),
                                    )
                                })?;
                                dilithium2::detached_sign(&tx_hash, &sk).as_bytes().to_vec()
                            }
                            3 => {
                                use pqcrypto_dilithium::dilithium3;
                                let sk = <dilithium3::SecretKey as SignSecretKeyTrait>::from_bytes(
                                    quantum_sk,
                                )
                                .map_err(|_| {
                                    QuantumError::InvalidKey(
                                        "Invalid Dilithium3 secret key in hybrid".to_string(),
                                    )
                                })?;
                                dilithium3::detached_sign(&tx_hash, &sk).as_bytes().to_vec()
                            }
                            5 => {
                                use pqcrypto_dilithium::dilithium5;
                                let sk = <dilithium5::SecretKey as SignSecretKeyTrait>::from_bytes(
                                    quantum_sk,
                                )
                                .map_err(|_| {
                                    QuantumError::InvalidKey(
                                        "Invalid Dilithium5 secret key in hybrid".to_string(),
                                    )
                                })?;
                                dilithium5::detached_sign(&tx_hash, &sk).as_bytes().to_vec()
                            }
                            _ => {
                                return Err(QuantumError::UnsupportedSecurityLevel(
                                    self.security_level,
                                ))
                            }
                        };

                        // Combine signatures
                        // Format: [classical_sig_len (2 bytes)][classical_sig][quantum_sig]
                        let mut combined_sig = Vec::new();
                        let classical_sig_len = classical_sig.len() as u16;
                        combined_sig.extend_from_slice(&classical_sig_len.to_be_bytes());
                        combined_sig.extend_from_slice(&classical_sig);
                        combined_sig.extend_from_slice(&quantum_sig);
                        combined_sig
                    }
                    ClassicalScheme::Ed25519 => {
                        // Extract keys
                        let classical_sk = &private_key[0..32]; // Ed25519 secret key is 32 bytes
                        let quantum_sk = &private_key[32..];

                        // Sign with Ed25519
                        use ed25519_dalek::{Signer, SigningKey};

                        let sk = SigningKey::from_bytes(classical_sk.try_into().map_err(|_| {
                            QuantumError::InvalidKey(
                                "Invalid Ed25519 secret key length".to_string(),
                            )
                        })?);
                        let sig = sk.sign(&tx_hash);
                        let classical_sig = sig.to_bytes();

                        // Sign with quantum algorithm (Dilithium)
                        let quantum_sig = match self.security_level {
                            2 => {
                                use pqcrypto_dilithium::dilithium2;
                                let sk = <dilithium2::SecretKey as SignSecretKeyTrait>::from_bytes(
                                    quantum_sk,
                                )
                                .map_err(|_| {
                                    QuantumError::InvalidKey(
                                        "Invalid Dilithium2 secret key in hybrid".to_string(),
                                    )
                                })?;
                                dilithium2::detached_sign(&tx_hash, &sk).as_bytes().to_vec()
                            }
                            3 => {
                                use pqcrypto_dilithium::dilithium3;
                                let sk = <dilithium3::SecretKey as SignSecretKeyTrait>::from_bytes(
                                    quantum_sk,
                                )
                                .map_err(|_| {
                                    QuantumError::InvalidKey(
                                        "Invalid Dilithium3 secret key in hybrid".to_string(),
                                    )
                                })?;
                                dilithium3::detached_sign(&tx_hash, &sk).as_bytes().to_vec()
                            }
                            5 => {
                                use pqcrypto_dilithium::dilithium5;
                                let sk = <dilithium5::SecretKey as SignSecretKeyTrait>::from_bytes(
                                    quantum_sk,
                                )
                                .map_err(|_| {
                                    QuantumError::InvalidKey(
                                        "Invalid Dilithium5 secret key in hybrid".to_string(),
                                    )
                                })?;
                                dilithium5::detached_sign(&tx_hash, &sk).as_bytes().to_vec()
                            }
                            _ => {
                                return Err(QuantumError::UnsupportedSecurityLevel(
                                    self.security_level,
                                ))
                            }
                        };

                        // Combine signatures
                        // Format: [classical_sig_len (2 bytes)][classical_sig][quantum_sig]
                        let mut combined_sig = Vec::new();
                        let classical_sig_len = classical_sig.len() as u16;
                        combined_sig.extend_from_slice(&classical_sig_len.to_be_bytes());
                        combined_sig.extend_from_slice(&classical_sig);
                        combined_sig.extend_from_slice(&quantum_sig);
                        combined_sig
                    }
                }
            }
        };

        // Create and return the quantum transaction
        Ok(QuantumTransaction {
            transaction,
            scheme: self.scheme,
            security_level: self.security_level,
            signature,
        })
    }
}

/// Factory for creating confidential transactions
pub struct ConfidentialTransactionBuilder {
    zkp_params: ZkpParams,
}

impl ConfidentialTransactionBuilder {
    pub fn new(zkp_params: ZkpParams) -> Self {
        Self { zkp_params }
    }

    /// Create a confidential transaction from regular inputs and outputs
    ///
    /// # Arguments
    /// * `version` - Transaction version number
    /// * `inputs` - Vector of transaction inputs
    /// * `outputs` - Vector of (amount, pub_key_script) pairs
    /// * `lock_time` - Transaction lock time
    /// * `rng` - Secure random number generator
    ///
    /// # Returns
    /// * `ConfidentialTransaction` - The created confidential transaction
    ///
    /// # Security considerations
    /// This method creates confidential transactions that hide output amounts using Pedersen commitments.
    /// Key security properties include:
    ///
    /// 1. **Hiding**: The value is concealed using a cryptographically secure blinding factor
    /// 2. **Binding**: The commitment cannot be modified to represent a different value
    /// 3. **Homomorphic**: Supports operations on committed values without revealing them
    ///
    /// Blinding factors are critical secrets and must be stored securely. Loss of a blinding
    /// factor will prevent spending the corresponding output. The RNG used must be
    /// cryptographically secure to prevent prediction of blinding factors.
    ///
    /// See https://eprint.iacr.org/2017/1066.pdf for mathematical details on Bulletproofs.
    pub fn create_transaction<R: rand::CryptoRng + rand::RngCore>(
        &self,
        version: u32,
        inputs: Vec<TransactionInput>,
        outputs: Vec<(u64, Vec<u8>)>, // (amount, pub_key_script)
        lock_time: u32,
        rng: &mut R,
    ) -> Result<(ConfidentialTransaction, Vec<Vec<u8>>), &'static str> {
        // Validate inputs and outputs
        if inputs.is_empty() {
            return Err("Confidential transaction must have at least one input");
        }

        if outputs.is_empty() {
            return Err("Confidential transaction must have at least one output");
        }

        // Limit the number of outputs to prevent DoS attacks
        const MAX_OUTPUTS: usize = 10000;
        if outputs.len() > MAX_OUTPUTS {
            return Err("Too many outputs in confidential transaction");
        }

        // Check for zero amounts which could cause cryptographic issues
        for (amount, _) in &outputs {
            if *amount == 0 {
                return Err("Zero amount outputs are not allowed in confidential transactions");
            }

            // Ensure the amount is within the valid range
            if *amount > u64::MAX / 2 {
                return Err("Amount is too large for confidential transaction");
            }
        }

        let mut conf_outputs = Vec::with_capacity(outputs.len());
        let mut blinding_factors = Vec::with_capacity(outputs.len());

        // Calculate the total value being committed (for verification and to avoid overflow)
        let total_output_value: u64 = outputs.iter().map(|(amount, _)| amount).sum();

        // Validate total output value is reasonable
        if total_output_value >= u64::MAX / 2 {
            return Err("Total output value is too large, risk of overflow");
        }

        for (amount, pub_key_script) in outputs {
            // Create a commitment to the amount using a secure blinding factor
            let (commitment, blinding) = crate::crypto::zkp::commit_pedersen(amount, rng);

            // The blinding factor is a critical secret - in a wallet implementation,
            // it must be securely stored to later prove ownership and spend the output
            blinding_factors.push(blinding.clone());

            // Create a range proof that the amount is positive without revealing it
            // This prevents both negative values and integer overflow attacks
            let range_proof = crate::crypto::zkp::create_range_proof(
                amount,
                &blinding,
                64, // 64-bit range proof (0 to 2^64-1)
                self.zkp_params.clone(),
                rng,
            );

            // Create the confidential output
            let conf_output = ConfidentialOutput::new(commitment, range_proof, pub_key_script);

            conf_outputs.push(conf_output);
        }

        // Create the confidential transaction
        let transaction = ConfidentialTransaction::new(version, inputs, conf_outputs, lock_time);

        // Return both the transaction and the blinding factors that must be stored securely
        // by the creator to later prove ownership and spend these outputs
        Ok((transaction, blinding_factors))
    }
}
