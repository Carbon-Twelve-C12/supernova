# SuperNova Blockchain Consensus Rules

This document outlines the formal specification of the SuperNova blockchain consensus rules. These rules define the properties that must be satisfied for blocks and transactions to be considered valid according to the consensus protocol.

## Table of Contents

1. [Block Validation Rules](#block-validation-rules)
2. [Transaction Validation Rules](#transaction-validation-rules)
3. [Chain Selection Rules](#chain-selection-rules)
4. [Formal Properties](#formal-properties)
5. [Verification Framework](#verification-framework)

## Block Validation Rules

### Block Structure

1. **Block Header**: Every block must contain a valid header with the following fields:
   - `version`: Protocol version (currently 1)
   - `previous_block_hash`: 32-byte hash of the previous block header
   - `merkle_root`: 32-byte hash representing the merkle root of the transactions
   - `timestamp`: Unix timestamp (seconds)
   - `difficulty_target`: Encoded difficulty target
   - `nonce`: 32-bit arbitrary value used for mining

2. **Block Size**: The total size of a block must not exceed `MAX_BLOCK_SIZE` (currently 4MB).

3. **Transactions**: A block must contain at least one transaction (the coinbase transaction).

4. **Coinbase Transaction**: The first transaction in a block must be a coinbase transaction.

5. **Block Weight**: The total weight of a block must not exceed `MAX_BLOCK_WEIGHT` (currently 8MB equivalent).

### Block Header Validation

1. **Version**: The block version must be valid for the current network state.

2. **Previous Block Hash**: Must reference a block that exists in the blockchain history.

3. **Merkle Root**: Must match the computed merkle root of the transactions included in the block.

4. **Timestamp**:
   - Must be greater than the median time of the previous 11 blocks
   - Must not be more than 2 hours in the future from the node's local time

5. **Difficulty Target**: Must match the expected difficulty based on the difficulty adjustment algorithm.

6. **Proof of Work**: The block hash must be less than or equal to the difficulty target.

### Block Consensus Rules

1. **Genesis Block**: The first block must match the predefined genesis block for the network.

2. **Difficulty Adjustment**: The difficulty target is adjusted every 2016 blocks to maintain an average block time of 10 minutes, with adjustments capped at 4x changes up or down.

3. **Subsidy Schedule**: The block subsidy (newly created NOVA) follows the predefined schedule, starting at 50 NOVA and halving every 420,000 blocks.

4. **Coinbase Maturity**: Newly created NOVA in coinbase transactions cannot be spent for 100 blocks.

5. **Maximum Supply**: The total supply must never exceed 42,000,000 NOVA.

## Transaction Validation Rules

### Transaction Structure

1. **Format**: Transactions must adhere to the SuperNova transaction format.

2. **Size**: A transaction must not exceed `MAX_TRANSACTION_SIZE` (currently 1MB).

3. **Weight**: A transaction's weight must not exceed `MAX_TRANSACTION_WEIGHT` (currently 400,000 weight units).

### Transaction Inputs

1. **UTXO References**: Each input must reference a valid unspent transaction output (UTXO).

2. **Signature Verification**: Each input must contain a valid signature that authorizes the spending of the referenced UTXO.

3. **No Double Spending**: No UTXO can be spent more than once.

4. **Maturity**: Inputs can only spend outputs that have reached maturity (at least 100 blocks for coinbase outputs).

### Transaction Outputs

1. **Value Range**: Each output value must be between `MIN_AMOUNT` (1 attonova) and `MAX_AMOUNT` (42,000,000 NOVA).

2. **Script Validity**: Output scripts must be standard and valid according to the scripting system.

### Coinbase Transactions

1. **No Inputs**: A coinbase transaction must not have regular inputs.

2. **Coinbase Input**: Must contain a single coinbase input with arbitrary data.

3. **Output Value**: The sum of outputs must not exceed the block subsidy plus total transaction fees.

4. **Extra Coinbase Data**: May include additional data in the coinbase field, up to 100 bytes.

### Quantum-Resistant Transactions

1. **Signature Verification**: Quantum-resistant signatures must be verified using the appropriate quantum-resistant signature scheme.

2. **Security Level**: Quantum-resistant transactions must use a security level appropriate for the transaction value and network settings.

### Confidential Transactions

1. **Range Proofs**: All confidential outputs must include valid range proofs.

2. **Value Balance**: The transaction must maintain value balance between inputs and outputs, accounting for fees.

## Chain Selection Rules

1. **Longest Chain**: The valid chain with the most accumulated proof of work is considered the main chain.

2. **First Seen**: In case of equal proof of work, the chain that was received first is preferred.

3. **Chain Reorganization**: A chain reorganization is performed when a new chain with more accumulated proof of work is discovered.

4. **Maximum Reorganization Depth**: For security reasons, reorganizations deeper than 100 blocks may require manual confirmation.

## Formal Properties

The SuperNova consensus must satisfy the following formal properties:

### Safety Properties

1. **Transaction Validity**: All transactions in the blockchain must adhere to the transaction validation rules.

2. **No Double Spending**: No UTXO can be spent more than once in the blockchain.

3. **Supply Conservation**: The total supply of NOVA must never exceed 42,000,000.

4. **Block Validity**: All blocks in the blockchain must adhere to the block validation rules.

### Liveness Properties

1. **Transaction Inclusion**: Any valid transaction with sufficient fee will eventually be included in the blockchain.

2. **Chain Growth**: The blockchain will continue to grow as long as mining power is applied.

3. **Confirmation Convergence**: All nodes will eventually converge on the same chain of confirmations.

### Invariants

1. **Chain Consistency**: At any point, all nodes with the same view of blocks will compute the same main chain.

2. **UTXO Set Consistency**: The UTXO set computed from the blockchain is consistent across all nodes.

3. **Digital Signature Unforgeability**: It is computationally infeasible to forge a valid transaction signature.

## Verification Framework

The SuperNova project includes a formal verification framework that allows for automated checking of these consensus rules. The framework supports:

1. **Invariant Checking**: Verifies that invariant properties hold at all times.

2. **Safety Property Verification**: Ensures that safety properties are never violated.

3. **Liveness Testing**: Tests that liveness properties are eventually satisfied.

4. **Model Checking**: Explores the state space to verify properties across all possible states.

5. **Property-Based Testing**: Generates random inputs to test that properties hold under various conditions.

### JSON Specification Format

Formal properties can be specified in a structured JSON format:

```json
{
  "properties": [
    {
      "id": "no-double-spend",
      "description": "No UTXO can be spent more than once",
      "verification_type": "Invariant",
      "predicate": {
        "type": "ForAll",
        "variable": "utxo",
        "in": "UTXO_SET",
        "formula": {
          "type": "AtMostOnce",
          "element": "utxo",
          "in": "spent_outputs"
        }
      },
      "is_critical": true
    },
    {
      "id": "supply-limit",
      "description": "The total supply of NOVA must never exceed 42,000,000",
      "verification_type": "Invariant",
      "predicate": {
        "type": "LessThanOrEqual",
        "left": {
          "type": "Function",
          "name": "total_supply"
        },
        "right": 4200000000000000
      },
      "is_critical": true
    }
  ]
}
```

### Using the Verification Framework

The ConsensusVerificationFramework can be used to verify these properties programmatically:

```rust
use btclib::consensus_verification::{
    ConsensusVerificationFramework, 
    ConsensusProperty,
    VerificationType,
    ChainState
};

// Create a framework instance
let chain_state = Arc::new(ChainState { /* ... */ });
let mut framework = ConsensusVerificationFramework::new(SecurityLevel::Maximum, chain_state);

// Import formal specifications
framework.import_specifications(specification_json)?;

// Verify a block
let verification_report = framework.verify_block(&block)?;
if !verification_report.success {
    // Handle verification failure
    let failures = verification_report.get_critical_failures();
    for failure in failures {
        println!("Critical failure: {}", failure.description);
    }
}

// Generate formal proofs
let proofs = framework.generate_proofs()?;
```

## Conclusion

This formal specification provides a precise and unambiguous definition of the SuperNova consensus rules. By adhering to these rules, nodes can independently verify the blockchain and reach consensus on its state. The formal verification framework allows for automated checking of these rules, increasing confidence in the correctness of the implementation. 