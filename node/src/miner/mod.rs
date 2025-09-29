//! Miner Module
//!
//! This module is responsible for block creation, proof-of-work computation,
//! and managing the mining process.

pub mod block_producer;
pub mod pow;

pub use block_producer::BlockProducer;
pub use pow::ProofOfWork;
