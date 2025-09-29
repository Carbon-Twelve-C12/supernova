use crate::types::transaction::{Transaction, TransactionInput, TransactionOutput};

/// Coinbase transaction builder
pub struct CoinbaseBuilder {
    /// Block height
    height: u64,
    /// Extra nonce
    extra_nonce: u64,
    /// Coinbase message
    message: Vec<u8>,
}

impl CoinbaseBuilder {
    /// Create a new coinbase builder
    pub fn new(height: u64) -> Self {
        Self {
            height,
            extra_nonce: 0,
            message: Vec::new(),
        }
    }
    
    /// Set the extra nonce
    pub fn with_extra_nonce(mut self, extra_nonce: u64) -> Self {
        self.extra_nonce = extra_nonce;
        self
    }
    
    /// Set the coinbase message
    pub fn with_message(mut self, message: Vec<u8>) -> Self {
        self.message = message;
        self
    }
    
    /// Build the coinbase transaction
    pub fn build(self, reward: u64, recipient_script: Vec<u8>) -> Transaction {
        // Create coinbase input
        let mut script_sig = Vec::new();
        
        // Add block height (BIP34)
        script_sig.push(0x03); // Push 3 bytes
        script_sig.extend_from_slice(&self.height.to_le_bytes()[..3]);
        
        // Add extra nonce
        script_sig.push(0x08); // Push 8 bytes
        script_sig.extend_from_slice(&self.extra_nonce.to_le_bytes());
        
        // Add message if any
        if !self.message.is_empty() {
            let msg_len = self.message.len().min(100);
            script_sig.push(msg_len as u8);
            script_sig.extend_from_slice(&self.message[..msg_len]);
        }
        
        // Create coinbase input using the proper constructor
        let coinbase_input = TransactionInput::new(
            [0u8; 32], // Null txid for coinbase
            0xFFFFFFFF, // Null output index
            script_sig,
            0xFFFFFFFF, // Sequence
        );
        
        // Create output using the proper constructor
        let output = TransactionOutput::new(reward, recipient_script);
        
        // Create transaction using the proper constructor
        Transaction::new(
            2, // version
            vec![coinbase_input],
            vec![output],
            0, // lock_time
        )
    }
}

/// Check if a transaction is a coinbase transaction
pub fn is_coinbase(tx: &Transaction) -> bool {
    let inputs = tx.inputs();
    inputs.len() == 1 
        && inputs[0].prev_tx_hash() == [0u8; 32]
        && inputs[0].prev_output_index() == 0xFFFFFFFF
}

/// Extract block height from coinbase transaction (BIP34)
pub fn extract_height(tx: &Transaction) -> Option<u64> {
    if !is_coinbase(tx) {
        return None;
    }
    
    let script_sig = tx.inputs()[0].script_sig();
    if script_sig.len() < 4 {
        return None;
    }
    
    // Check if first byte indicates height length
    let height_len = script_sig[0] as usize;
    if height_len > 8 || script_sig.len() < height_len + 1 {
        return None;
    }
    
    // Extract height bytes
    let mut height_bytes = [0u8; 8];
    height_bytes[..height_len].copy_from_slice(&script_sig[1..height_len + 1]);
    
    Some(u64::from_le_bytes(height_bytes))
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_coinbase_builder() {
        let coinbase = CoinbaseBuilder::new(123456)
            .with_extra_nonce(42)
            .with_message(b"Hello, Supernova!".to_vec())
            .build(5000000000, vec![0x76, 0xa9, 0x14]); // P2PKH script prefix
        
        assert!(is_coinbase(&coinbase));
        assert_eq!(coinbase.outputs()[0].value(), 5000000000);
    }
    
    #[test]
    fn test_height_extraction() {
        let coinbase = CoinbaseBuilder::new(123456)
            .build(5000000000, vec![]);
        
        let height = extract_height(&coinbase);
        assert_eq!(height, Some(123456));
    }
} 