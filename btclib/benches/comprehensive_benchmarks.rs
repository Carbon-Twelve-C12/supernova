// Comprehensive Benchmarks for Supernova
// Meeting the Satoshi Standard: Every critical path must be benchmarked

use btclib::{
    blockchain::{Block, Transaction},
    consensus::{ProofOfWork, QuantumProofOfWork},
    crypto::{
        falcon_real::FalconKeyPair,
        hash::{blake3_hash, sha256, sha3_256},
        kem::{DilithiumKEM, KyberKEM},
        zkp::{ZKProof, ZKVerifier},
    },
    environmental::{emissions::EmissionCalculator, tracker::EnvironmentalTracker},
    lightning::{
        channel::Channel,
        payment::{Payment, PaymentPreimage},
        router::Router,
    },
    mempool::Mempool,
    mining::{miner::Miner, reward::calculate_mining_reward},
    network::{message::NetworkMessage, p2p::P2PNetwork},
    script::{Script, ScriptEngine},
    storage::{persistence::BlockchainDB, utxo_set::UtxoSet},
    types::{
        block::BlockHeader,
        transaction::{TxInput, TxOutput},
    },
    validation::{block_validator::BlockValidator, transaction_validator::TransactionValidator},
    wallet::{hdwallet::HDWallet, quantum_wallet::QuantumWallet},
};
use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use std::time::Duration;

/// Benchmark cryptographic operations
fn bench_crypto(c: &mut Criterion) {
    let mut group = c.benchmark_group("crypto");

    // SHA-256 benchmarks
    group.bench_function("sha256_32bytes", |b| {
        let data = vec![0u8; 32];
        b.iter(|| sha256(&data))
    });

    group.bench_function("sha256_1kb", |b| {
        let data = vec![0u8; 1024];
        b.iter(|| sha256(&data))
    });

    group.bench_function("sha256_1mb", |b| {
        let data = vec![0u8; 1024 * 1024];
        b.iter(|| sha256(&data))
    });

    // SHA3-256 benchmarks
    group.bench_function("sha3_256_32bytes", |b| {
        let data = vec![0u8; 32];
        b.iter(|| sha3_256(&data))
    });

    // Blake3 benchmarks
    group.bench_function("blake3_32bytes", |b| {
        let data = vec![0u8; 32];
        b.iter(|| blake3_hash(&data))
    });

    // Falcon signature benchmarks
    group.bench_function("falcon_keygen", |b| b.iter(|| FalconKeyPair::generate()));

    let keypair = FalconKeyPair::generate().unwrap();
    let message = b"Benchmark message for Falcon signatures";

    group.bench_function("falcon_sign", |b| {
        b.iter(|| keypair.sign(black_box(message)))
    });

    let signature = keypair.sign(message).unwrap();
    group.bench_function("falcon_verify", |b| {
        b.iter(|| keypair.verify(black_box(message), black_box(&signature)))
    });

    // Kyber KEM benchmarks
    group.bench_function("kyber_keygen", |b| b.iter(|| KyberKEM::generate_keypair()));

    let (kyber_pk, kyber_sk) = KyberKEM::generate_keypair().unwrap();
    group.bench_function("kyber_encapsulate", |b| {
        b.iter(|| KyberKEM::encapsulate(&kyber_pk))
    });

    let (ciphertext, _) = KyberKEM::encapsulate(&kyber_pk).unwrap();
    group.bench_function("kyber_decapsulate", |b| {
        b.iter(|| KyberKEM::decapsulate(&ciphertext, &kyber_sk))
    });

    group.finish();
}

/// Benchmark transaction operations
fn bench_transactions(c: &mut Criterion) {
    let mut group = c.benchmark_group("transactions");

    // Create test transaction
    let tx = Transaction {
        version: 1,
        inputs: vec![TxInput {
            previous_output: Default::default(),
            script_sig: vec![0u8; 72], // Typical signature size
            sequence: 0xffffffff,
        }],
        outputs: vec![TxOutput {
            value: 50000000,
            script_pubkey: vec![0u8; 25], // P2PKH script
        }],
        lock_time: 0,
    };

    group.bench_function("tx_serialize", |b| b.iter(|| tx.serialize()));

    let serialized = tx.serialize();
    group.bench_function("tx_deserialize", |b| {
        b.iter(|| Transaction::deserialize(&serialized))
    });

    group.bench_function("tx_hash", |b| b.iter(|| tx.hash()));

    // Transaction validation
    let validator = TransactionValidator::new();
    group.bench_function("tx_validate_basic", |b| {
        b.iter(|| validator.validate_basic(&tx))
    });

    // Multi-input transaction
    let multi_tx = Transaction {
        version: 1,
        inputs: (0..10)
            .map(|_| TxInput {
                previous_output: Default::default(),
                script_sig: vec![0u8; 72],
                sequence: 0xffffffff,
            })
            .collect(),
        outputs: (0..10)
            .map(|i| TxOutput {
                value: 5000000 * i,
                script_pubkey: vec![0u8; 25],
            })
            .collect(),
        lock_time: 0,
    };

    group.bench_function("tx_validate_multi_10x10", |b| {
        b.iter(|| validator.validate_basic(&multi_tx))
    });

    group.finish();
}

/// Benchmark block operations
fn bench_blocks(c: &mut Criterion) {
    let mut group = c.benchmark_group("blocks");

    // Create test block
    let header = BlockHeader {
        version: 1,
        previous_block_hash: [0u8; 32],
        merkle_root: [0u8; 32],
        timestamp: 1234567890,
        bits: 0x1d00ffff,
        nonce: 0,
        quantum_proof: None,
    };

    let block = Block {
        header: header.clone(),
        transactions: vec![Transaction::default(); 100], // 100 transactions
    };

    group.bench_function("block_serialize", |b| b.iter(|| block.serialize()));

    group.bench_function("block_hash", |b| b.iter(|| block.hash()));

    group.bench_function("block_merkle_root", |b| {
        b.iter(|| block.calculate_merkle_root())
    });

    // Block validation
    let validator = BlockValidator::new();
    group.bench_function("block_validate_header", |b| {
        b.iter(|| validator.validate_header(&header))
    });

    // Large block
    let large_block = Block {
        header: header.clone(),
        transactions: vec![Transaction::default(); 1000], // 1000 transactions
    };

    group.bench_function("block_validate_1000tx", |b| {
        b.iter(|| validator.validate_basic(&large_block))
    });

    group.finish();
}

/// Benchmark mining operations
fn bench_mining(c: &mut Criterion) {
    let mut group = c.benchmark_group("mining");

    // Proof of Work
    let pow = ProofOfWork::new(0x1d00ffff); // Difficulty target
    let header = BlockHeader::default();

    group.bench_function("pow_validate", |b| b.iter(|| pow.validate(&header)));

    // Mining reward calculation
    let env_profile = Default::default();
    group.bench_function("mining_reward_calc", |b| {
        b.iter(|| calculate_mining_reward(black_box(700000), &env_profile))
    });

    // Quantum PoW
    let qpow = QuantumProofOfWork::new();
    group.bench_function("quantum_pow_generate", |b| {
        b.iter(|| qpow.generate_proof(&header))
    });

    group.finish();
}

/// Benchmark UTXO operations
fn bench_utxo(c: &mut Criterion) {
    let mut group = c.benchmark_group("utxo");

    let utxo_set = UtxoSet::new_in_memory(10000);

    // Add UTXO
    let entry = Default::default();
    group.bench_function("utxo_add", |b| {
        b.iter(|| utxo_set.add(black_box(entry.clone())))
    });

    // Get UTXO
    let outpoint = Default::default();
    group.bench_function("utxo_get", |b| {
        b.iter(|| utxo_set.get(black_box(&outpoint)))
    });

    // Remove UTXO
    group.bench_function("utxo_remove", |b| {
        b.iter(|| utxo_set.remove(black_box(&outpoint)))
    });

    // UTXO commitment update
    group.bench_function("utxo_commitment_update", |b| {
        b.iter(|| utxo_set.update_commitment(black_box(700000)))
    });

    group.finish();
}

/// Benchmark Lightning Network operations
fn bench_lightning(c: &mut Criterion) {
    let mut group = c.benchmark_group("lightning");

    // Payment preimage generation
    group.bench_function("payment_preimage_gen", |b| {
        b.iter(|| PaymentPreimage::new_random())
    });

    // Payment hash calculation
    let preimage = PaymentPreimage::new_random();
    group.bench_function("payment_hash_calc", |b| b.iter(|| preimage.payment_hash()));

    // Channel state update
    let channel = Channel::new_test_channel();
    group.bench_function("channel_state_update", |b| {
        b.iter(|| channel.update_state())
    });

    // Route finding
    let router = Router::new();
    group.bench_function("route_find_3hop", |b| {
        b.iter(|| router.find_route("destination", 1000000, &[]))
    });

    group.finish();
}

/// Benchmark script execution
fn bench_script(c: &mut Criterion) {
    let mut group = c.benchmark_group("script");

    // P2PKH script
    let p2pkh_script = Script::new_p2pkh(&[0u8; 20]);
    let engine = ScriptEngine::new();

    group.bench_function("script_p2pkh_verify", |b| {
        b.iter(|| engine.execute(&p2pkh_script))
    });

    // Multi-sig script
    let multisig_script = Script::new_multisig(2, &vec![[0u8; 33]; 3]);
    group.bench_function("script_multisig_2of3", |b| {
        b.iter(|| engine.execute(&multisig_script))
    });

    // Complex script with multiple operations
    let complex_script = Script::new_complex();
    group.bench_function("script_complex_ops", |b| {
        b.iter(|| engine.execute(&complex_script))
    });

    group.finish();
}

/// Benchmark network operations
fn bench_network(c: &mut Criterion) {
    let mut group = c.benchmark_group("network");

    // Message serialization
    let msg = NetworkMessage::new_block_announcement(Block::default());
    group.bench_function("msg_serialize", |b| b.iter(|| msg.serialize()));

    let serialized = msg.serialize();
    group.bench_function("msg_deserialize", |b| {
        b.iter(|| NetworkMessage::deserialize(&serialized))
    });

    // Message validation
    group.bench_function("msg_validate", |b| b.iter(|| msg.validate()));

    group.finish();
}

/// Benchmark environmental tracking
fn bench_environmental(c: &mut Criterion) {
    let mut group = c.benchmark_group("environmental");

    let tracker = EnvironmentalTracker::new();

    // Emission calculation
    group.bench_function("emission_calc_1mw", |b| {
        b.iter(|| tracker.calculate_emissions(black_box(1000.0), black_box("grid")))
    });

    // Carbon credit validation
    group.bench_function("carbon_credit_validate", |b| {
        b.iter(|| tracker.validate_carbon_credit("TEST-CREDIT-001"))
    });

    // Environmental score calculation
    group.bench_function("env_score_calc", |b| {
        b.iter(|| tracker.calculate_environmental_score())
    });

    group.finish();
}

/// Benchmark wallet operations
fn bench_wallet(c: &mut Criterion) {
    let mut group = c.benchmark_group("wallet");

    // HD wallet key derivation
    let hdwallet = HDWallet::new_random().unwrap();
    group.bench_function("hd_derive_key", |b| {
        b.iter(|| hdwallet.derive_key(black_box(0), black_box(0)))
    });

    // Quantum wallet operations
    let qwallet = QuantumWallet::new().unwrap();
    group.bench_function("quantum_wallet_sign", |b| {
        let msg = b"Test transaction";
        b.iter(|| qwallet.sign_transaction(black_box(msg)))
    });

    // Address generation
    group.bench_function("address_gen_p2pkh", |b| {
        b.iter(|| hdwallet.generate_address(black_box(0)))
    });

    group.finish();
}

/// Benchmark mempool operations
fn bench_mempool(c: &mut Criterion) {
    let mut group = c.benchmark_group("mempool");

    let mempool = Mempool::new(10000);
    let tx = Transaction::default();

    // Add transaction
    group.bench_function("mempool_add", |b| {
        b.iter(|| mempool.add_transaction(black_box(tx.clone())))
    });

    // Get transaction
    let txid = tx.hash();
    group.bench_function("mempool_get", |b| {
        b.iter(|| mempool.get_transaction(black_box(&txid)))
    });

    // Select transactions for block
    group.bench_function("mempool_select_1mb", |b| {
        b.iter(|| mempool.select_transactions(black_box(1_000_000)))
    });

    group.finish();
}

// Configure criterion
criterion_group! {
    name = benches;
    config = Criterion::default()
        .sample_size(100)
        .measurement_time(Duration::from_secs(10))
        .warm_up_time(Duration::from_secs(3));
    targets =
        bench_crypto,
        bench_transactions,
        bench_blocks,
        bench_mining,
        bench_utxo,
        bench_lightning,
        bench_script,
        bench_network,
        bench_environmental,
        bench_wallet,
        bench_mempool
}

criterion_main!(benches);
