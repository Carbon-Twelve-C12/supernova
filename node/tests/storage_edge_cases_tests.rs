//! Storage Edge Cases Tests  
//!
//! TEST SUITE (P2-012): Storage module edge case testing

use node::storage::{AtomicUtxoSet, OutPoint, UnspentOutput, UtxoTransaction};
use tempfile::tempdir;

#[test]
fn test_atomic_utxo_empty_set() {
    let temp_dir = tempdir().unwrap();
    let utxo_set = AtomicUtxoSet::new(temp_dir.path().join("test.db")).unwrap();
    
    // Empty transaction
    let empty_tx = UtxoTransaction {
        inputs: vec![],
        outputs: vec![],
    };
    
    let result = utxo_set.apply_transaction(empty_tx);
    assert!(result.is_ok());
    
    println!("✓ Empty transaction on UTXO set");
}

#[test]
fn test_atomic_utxo_add_remove() {
    let temp_dir = tempdir().unwrap();
    let utxo_set = AtomicUtxoSet::new(temp_dir.path().join("test.db")).unwrap();
    
    let outpoint = OutPoint::new([1; 32], 0);
    let output = UnspentOutput {
        txid: [1; 32],
        vout: 0,
        value: 1000,
        script_pubkey: vec![0; 25],
        height: 1,
        is_coinbase: false,
    };
    
    // Add UTXO
    let add_tx = UtxoTransaction {
        inputs: vec![],
        outputs: vec![(outpoint, output)],
    };
    utxo_set.apply_transaction(add_tx).unwrap();
    
    // Remove UTXO
    let remove_tx = UtxoTransaction {
        inputs: vec![outpoint],
        outputs: vec![],
    };
    let result = utxo_set.apply_transaction(remove_tx);
    assert!(result.is_ok());
    
    println!("✓ Atomic add and remove UTXO");
}

#[test]
fn test_atomic_utxo_double_spend_prevention() {
    let temp_dir = tempdir().unwrap();
    let utxo_set = AtomicUtxoSet::new(temp_dir.path().join("test.db")).unwrap();
    
    let outpoint = OutPoint::new([1; 32], 0);
    let output = UnspentOutput {
        txid: [1; 32],
        vout: 0,
        value: 1000,
        script_pubkey: vec![0; 25],
        height: 1,
        is_coinbase: false,
    };
    
    // Add UTXO
    let add_tx = UtxoTransaction {
        inputs: vec![],
        outputs: vec![(outpoint, output)],
    };
    utxo_set.apply_transaction(add_tx).unwrap();
    
    // Spend once
    let spend1 = UtxoTransaction {
        inputs: vec![outpoint],
        outputs: vec![],
    };
    utxo_set.apply_transaction(spend1).unwrap();
    
    // Try double spend
    let spend2 = UtxoTransaction {
        inputs: vec![outpoint],
        outputs: vec![],
    };
    let result = utxo_set.apply_transaction(spend2);
    assert!(result.is_err(), "Double spend should be rejected");
    
    println!("✓ Double spend detected and prevented");
}

#[test]
fn test_atomic_utxo_zero_value() {
    let temp_dir = tempdir().unwrap();
    let utxo_set = AtomicUtxoSet::new(temp_dir.path().join("test.db")).unwrap();
    
    let outpoint = OutPoint::new([2; 32], 0);
    let output = UnspentOutput {
        txid: [2; 32],
        vout: 0,
        value: 0,
        script_pubkey: vec![0; 25],
        height: 1,
        is_coinbase: false,
    };
    
    let tx = UtxoTransaction {
        inputs: vec![],
        outputs: vec![(outpoint, output)],
    };
    let result = utxo_set.apply_transaction(tx);
    assert!(result.is_ok());
    
    println!("✓ Zero-value UTXO accepted");
}

#[test]
fn test_atomic_utxo_max_value() {
    let temp_dir = tempdir().unwrap();
    let utxo_set = AtomicUtxoSet::new(temp_dir.path().join("test.db")).unwrap();
    
    let outpoint = OutPoint::new([3; 32], 0);
    let output = UnspentOutput {
        txid: [3; 32],
        vout: 0,
        value: u64::MAX,
        script_pubkey: vec![0; 25],
        height: 1,
        is_coinbase: false,
    };
    
    let tx = UtxoTransaction {
        inputs: vec![],
        outputs: vec![(outpoint, output)],
    };
    let result = utxo_set.apply_transaction(tx);
    assert!(result.is_ok());
    
    println!("✓ Maximum u64 value UTXO accepted");
}

#[test]
fn test_atomic_utxo_large_script() {
    let temp_dir = tempdir().unwrap();
    let utxo_set = AtomicUtxoSet::new(temp_dir.path().join("test.db")).unwrap();
    
    let outpoint = OutPoint::new([4; 32], 0);
    let output = UnspentOutput {
        txid: [4; 32],
        vout: 0,
        value: 1000,
        script_pubkey: vec![0; 10000], // 10KB
        height: 1,
        is_coinbase: false,
    };
    
    let tx = UtxoTransaction {
        inputs: vec![],
        outputs: vec![(outpoint, output)],
    };
    let result = utxo_set.apply_transaction(tx);
    assert!(result.is_ok());
    
    println!("✓ Large 10KB script UTXO accepted");
}

#[test]
fn test_atomic_utxo_many_outputs() {
    let temp_dir = tempdir().unwrap();
    let utxo_set = AtomicUtxoSet::new(temp_dir.path().join("test.db")).unwrap();
    
    let mut outputs = vec![];
    for i in 0..50u32 {
        let outpoint = OutPoint::new([5; 32], i);
        let output = UnspentOutput {
            txid: [5; 32],
            vout: i,
            value: 1000,
            script_pubkey: vec![0; 25],
            height: 1,
            is_coinbase: false,
        };
        outputs.push((outpoint, output));
    }
    
    let tx = UtxoTransaction {
        inputs: vec![],
        outputs,
    };
    let result = utxo_set.apply_transaction(tx);
    assert!(result.is_ok());
    
    println!("✓ 50 UTXOs in single transaction");
}

