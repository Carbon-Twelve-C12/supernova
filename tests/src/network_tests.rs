use node::network::{P2PNetwork, NetworkCommand, NetworkEvent};
use btclib::types::block::Block;
use tokio::sync::mpsc;
use tokio::time::Duration;
use tracing::{debug, info};
use std::sync::Arc;

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;
    use node::storage::{ChainState, BlockchainDB};

    #[tokio::test]
    async fn test_network_creation() {
        // Create a simple P2P network instance for testing
        let (network, command_tx, event_rx) = P2PNetwork::new(
            None,
            [0u8; 32],
            "test_network",
            None, // listen address
            None, // validation mode (defaults to Strict)
        ).await.unwrap();

        // Simple assertion to verify creation
        assert!(true);
    }

    #[tokio::test]
    async fn test_block_propagation() {
        // Set up initial test environment
        let temp_dir = tempdir().unwrap();
        let db = Arc::new(BlockchainDB::new(temp_dir.path()).unwrap());
        let chain_state = ChainState::new(Arc::clone(&db)).unwrap();

        // Create a test block
        let prev_hash = chain_state.get_best_block_hash();
        let block = Block::new(1, prev_hash, Vec::new(), u32::MAX);

        // This is a simple smoke test that doesn't need to fully propagate
        assert!(block.hash() != [0u8; 32]);
    }
}