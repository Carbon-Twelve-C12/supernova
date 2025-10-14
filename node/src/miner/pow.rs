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
            let mut block = block_clone.lock().unwrap();
            let mut nonce: u32 = 0;
            loop {
                block.header.nonce = nonce;
                let hash = block.hash();
                if hash < difficulty_target {
                    sender.send(block.clone()).unwrap();
                    break;
                }
                nonce = nonce.wrapping_add(1);
            }
        });

        receiver.recv().ok()
    }
}
