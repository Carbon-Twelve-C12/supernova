//! Proof-of-Work
//!
//! This module contains the logic for performing the proof-of-work computation
//! to find a valid block hash.

use supernova_core::types::Block;
use std::sync::{mpsc, Arc, Mutex};
use std::thread;

pub struct ProofOfWork;

impl ProofOfWork {
    pub fn mine(block: Block, difficulty_target: [u8; 32]) -> Option<Block> {
        let block = Arc::new(Mutex::new(block));
        let (sender, receiver) = mpsc::channel();

        // Simple single-threaded mining for now
        let block_clone = Arc::clone(&block);
        thread::spawn(move || {
            // If the mutex is poisoned another worker has already panicked; bail
            // out so the receiver below observes a closed channel instead of us
            // propagating a panic into the miner thread pool.
            let mut block = match block_clone.lock() {
                Ok(guard) => guard,
                Err(_) => return,
            };
            let mut nonce: u32 = 0;
            loop {
                block.header.nonce = nonce;
                let hash = block.hash();
                if hash < difficulty_target {
                    // Receiver may have been dropped (e.g. mining cancelled);
                    // a send failure is not a programming error here.
                    let _ = sender.send(block.clone());
                    break;
                }
                nonce = nonce.wrapping_add(1);
            }
        });

        receiver.recv().ok()
    }
}
