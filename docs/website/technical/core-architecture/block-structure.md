# Block Structure

## Overview

The Supernova blockchain uses a block structure that extends traditional blockchain designs with additional fields for quantum resistance, environmental data, and enhanced security. This document explains the structure of Supernova blocks, their components, and how they work together to form the blockchain.

## Block Components

A Supernova block consists of two primary components:

1. **Block Header**: Contains metadata and consensus-critical information
2. **Block Body**: Contains the actual transaction data

## Block Header

The block header contains critical metadata about the block and is used for the proof-of-work calculation. Each header is 144 bytes (without optional fields) and contains the following fields:

| Field | Size (bytes) | Description |
|-------|--------------|-------------|
| Version | 4 | Block version number |
| Previous Block Hash | 32 | SHA-256 hash of the previous block header |
| Merkle Root | 32 | Root of the Merkle tree of transactions |
| Timestamp | 8 | Unix timestamp of block creation (64-bit) |
| Difficulty Target | 4 | Compact representation of the current difficulty |
| Nonce | 8 | Value modified during mining to satisfy PoW (64-bit) |
| Environmental Data Root | 32 | Merkle root of environmental impact data |
| Quantum Signature | Variable | Optional quantum-resistant signature (when enabled) |
| Quantum Public Key | Variable | Optional quantum-resistant public key (when enabled) |

### Header Format

```rust
struct BlockHeader {
    // Core fields (96 bytes)
    version: u32,                   // 4 bytes
    previous_block_hash: [u8; 32],  // 32 bytes
    merkle_root: [u8; 32],          // 32 bytes
    timestamp: u64,                 // 8 bytes
    difficulty_target: u32,         // 4 bytes
    nonce: u64,                     // 8 bytes
    
    // Extended fields (32 bytes)
    environmental_data_root: [u8; 32], // 32 bytes
    
    // Optional quantum-resistant fields (variable size)
    quantum_signature: Option<Vec<u8>>,
    quantum_public_key: Option<Vec<u8>>,
}
```

### Field Details

#### Version
A 4-byte integer that indicates the block version. The version number is used to signal support for new features and protocol upgrades. Currently, the following versions are defined:

- **Version 1**: Original format
- **Version 2**: Added environmental data support
- **Version 3**: Added quantum signature support

#### Previous Block Hash
A 32-byte field containing the SHA-256 hash of the previous block header. This creates the chain of blocks and ensures immutability.

#### Merkle Root
A 32-byte field containing the root of the Merkle tree constructed from all transactions in the block. This allows for efficient verification that a transaction is included in a block.

#### Timestamp
An 8-byte unsigned integer representing the Unix timestamp when the block was created. The timestamp must be:
- Greater than the median timestamp of the previous 11 blocks
- Less than the network-adjusted time + 2 hours

#### Difficulty Target
A 4-byte field that encodes the proof-of-work difficulty target in a compact format. The actual target threshold is derived from this field.

#### Nonce
An 8-byte field that miners modify to find a block hash that satisfies the difficulty target. Supernova uses a 64-bit nonce (compared to Bitcoin's 32-bit) to provide more search space for mining.

#### Environmental Data Root
A 32-byte field containing the Merkle root of the environmental impact data. This includes:
- Energy consumption metrics
- Carbon emissions data
- Renewable energy certificates
- Carbon offset proofs

#### Quantum Signature (Optional)
A variable-length field containing a quantum-resistant signature. This signature is only included in blocks with version 3 or higher. It uses a lattice-based signature scheme that is resistant to quantum computing attacks.

#### Quantum Public Key (Optional)
A variable-length field containing the quantum-resistant public key used to verify the quantum signature. This field is only included in blocks with version 3 or higher.

## Block Body

The block body contains the actual data of the block, primarily the list of transactions. It has the following structure:

### Transaction Count
A variable-length integer (VarInt) specifying the number of transactions in the block.

### Transactions
An array of transactions included in the block. Each transaction follows the Supernova transaction structure (detailed in the Transaction Structure documentation).

### Environmental Data Section
An optional section containing environmental impact data, including:
- Energy consumption metrics
- Carbon emissions estimates
- Renewable energy source information
- Carbon offset certificates

## Block Size Limits

Supernova implements a flexible block size limit with the following parameters:

- **Base Block Size Limit**: 2 MB
- **Maximum Block Size Limit**: 8 MB
- **Adaptive Sizing**: Block size can increase based on network demand up to the maximum

The actual block size limit is calculated using a moving average of recent block sizes and transaction demand. This allows the network to scale during peak usage while preventing blockchain bloat.

## Block Serialization

Blocks are serialized in a binary format for transmission and storage. The serialization format follows these steps:

1. **Magic Bytes**: Network identifier (4 bytes)
2. **Block Size**: Total size of the block in bytes (4 bytes)
3. **Block Header**: Serialized block header
4. **Transaction Count**: Number of transactions (VarInt)
5. **Transactions**: Serialized transactions
6. **Environmental Data Size**: Size of environmental data (VarInt)
7. **Environmental Data**: Serialized environmental data (if present)

## Block Identification

Blocks in Supernova are identified by their block hash, which is calculated by applying a double SHA-256 hash to the serialized block header:

```
block_hash = SHA256(SHA256(serialized_block_header))
```

This hash serves as a unique identifier for the block and is used in the `previous_block_hash` field of the next block.

## Merkle Tree Construction

Supernova uses a Merkle tree to efficiently verify the inclusion of transactions in a block. The Merkle tree is constructed as follows:

1. Calculate the transaction ID (double SHA-256 hash) for each transaction
2. Pair adjacent transaction IDs and hash them together
3. If there is an odd number of transaction IDs, duplicate the last one
4. Continue pairing and hashing until a single hash remains (the Merkle root)

### Example Merkle Tree

For transactions with IDs A, B, C, and D:

```
    Root = Hash(Hash(A+B) + Hash(C+D))
             /                  \
       Hash(A+B)              Hash(C+D)
        /    \                /    \
       A      B              C      D
```

## Environmental Data Structure

The environmental data section contains information about the energy consumption and carbon footprint of the block. It includes:

```rust
struct EnvironmentalData {
    // Energy metrics
    energy_consumption_kwh: f64,
    carbon_emissions_kg: f64,
    
    // Energy source breakdown (percentage)
    renewable_energy_percentage: u8,
    
    // Certification data
    energy_certificates: Vec<Certificate>,
    carbon_offsets: Vec<Offset>,
    
    // Signature from authorized energy auditor (optional)
    auditor_signature: Option<Signature>,
}
```

## Block Validation Rules

For a block to be considered valid, it must pass the following validation checks:

1. **Block Header Format**: The header must follow the correct format
2. **Proof-of-Work**: The block hash must be below the difficulty target
3. **Previous Block**: The previous block hash must reference a valid block in the chain
4. **Timestamp**: Must be within the allowed range
5. **Block Size**: Must not exceed the current block size limit
6. **First Transaction**: Must be a valid coinbase transaction
7. **Transactions**: All transactions must be valid and properly formatted
8. **Merkle Root**: Must match the calculated Merkle root from the transactions
9. **Environmental Data**: If present, must be properly formatted and valid
10. **Quantum Signature**: If present, must be valid for the block header

## Genesis Block

The Supernova genesis block is the first block in the blockchain and has the following characteristics:

- **Block Height**: 0
- **Previous Block Hash**: All zeros
- **Timestamp**: 1689436800 (July 15, 2023, 12:00:00 UTC)
- **Difficulty Target**: Initial difficulty target for the network
- **Nonce**: 2083236893
- **Coinbase Transaction**: Contains the genesis message and initial coin allocation
- **Coinbase Message**: "Supernova: Sustainable, Secure, Quantum-Resistant Blockchain for a Brighter Future"

## Conclusion

The Supernova block structure extends traditional blockchain designs with innovations for environmental sustainability, quantum resistance, and enhanced security. By incorporating these features directly into the block structure, Supernova creates a foundation for a more sustainable and future-proof blockchain ecosystem. 