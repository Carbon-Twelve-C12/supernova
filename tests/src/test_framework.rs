use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use tempfile::TempDir;
use tokio::sync::{mpsc, oneshot};
use tokio::time::timeout;
use tracing::{debug, info, warn, error};

use btclib::types::block::Block;
use btclib::types::transaction::Transaction;
use node::network::{P2PNetwork, NetworkCommand, NetworkEvent};
use node::storage::{BlockchainDB, ChainState};
use node::mempool::{TransactionPool, MempoolConfig};
use node::metrics::registry::{MetricsRegistry, MetricsConfig};
use node::config::NodeConfig;

/// Test node instance with all components
pub struct TestNode {
    /// Node ID for test reference
    pub id: usize,
    /// Database for this node
    pub db: Arc<BlockchainDB>,
    /// Chain state
    pub chain_state: Arc<ChainState>,
    /// Mempool
    pub mempool: Arc<TransactionPool>,
    /// Network handler
    pub network: P2PNetwork,
    /// Network command sender
    pub network_tx: mpsc::Sender<NetworkCommand>,
    /// Network event receiver
    pub network_rx: mpsc::Receiver<NetworkEvent>,
    /// Data directory (temporary)
    pub data_dir: TempDir,
    /// Metrics registry
    pub metrics: Arc<MetricsRegistry>,
    /// Node is running flag
    pub running: bool,
}

/// Test network with multiple nodes
pub struct TestNetwork {
    /// Nodes in the test network
    pub nodes: Vec<TestNode>,
    /// Connections between nodes
    pub connections: Vec<(usize, usize)>,
}

impl TestNode {
    /// Create a new test node
    pub async fn new(id: usize) -> Result<Self, Box<dyn std::error::Error>> {
        // Create temporary directory
        let data_dir = tempfile::tempdir()?;
        let db_path = data_dir.path().join("chaindata");
        
        // Initialize database
        let db = Arc::new(BlockchainDB::new(&db_path)?);
        
        // Initialize chain state
        let chain_state = Arc::new(ChainState::new(Arc::clone(&db))?);
        
        // Initialize mempool
        let mempool_config = MempoolConfig::default();
        let mempool = Arc::new(TransactionPool::new(mempool_config));
        
        // Initialize metrics (disabled for testing)
        let metrics = Arc::new(MetricsRegistry::disabled());
        
        // Initialize network
        let (network, network_tx, network_rx) = P2PNetwork::new(
            None, // No custom keypair
            [0u8; 32], // Genesis hash
            &format!("test-{}", id), // Network ID
        ).await?;
        
        Ok(Self {
            id,
            db,
            chain_state,
            mempool,
            network,
            network_tx,
            network_rx,
            data_dir,
            metrics,
            running: false,
        })
    }
    
    /// Start the node
    pub async fn start(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        if self.running {
            return Ok(());
        }
        
        // In a real implementation, this would start all node components
        self.running = true;
        
        Ok(())
    }
    
    /// Stop the node
    pub async fn stop(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        if !self.running {
            return Ok(());
        }
        
        // In a real implementation, this would stop all node components
        self.running = false;
        
        Ok(())
    }
    
    /// Add a block to the node
    pub async fn add_block(&self, block: Block) -> Result<bool, Box<dyn std::error::Error>> {
        // Process the block through chain state
        let added = self.chain_state.process_block(block).await?;
        
        Ok(added)
    }
    
    /// Add a transaction to the node's mempool
    pub async fn add_transaction(&self, tx: Transaction) -> Result<bool, Box<dyn std::error::Error>> {
        // Add transaction to mempool
        self.mempool.add_transaction(tx, 1)?;
        
        Ok(true)
    }
    
    /// Mine a new block with transactions from the mempool
    pub async fn mine_block(&self) -> Result<Block, Box<dyn std::error::Error>> {
        // Get current chain state
        let height = self.chain_state.get_height();
        let prev_hash = self.chain_state.get_best_block_hash();
        
        // Get transactions from mempool
        let txs = self.mempool.get_sorted_transactions();
        
        // Create a new block
        let block = Block::new(
            1, // Version
            prev_hash,
            txs,
            u32::MAX / 2, // Target difficulty
        );
        
        // Add the block to our chain
        self.add_block(block.clone()).await?;
        
        Ok(block)
    }
    
    /// Get current blockchain height
    pub fn get_height(&self) -> u64 {
        self.chain_state.get_height()
    }
    
    /// Get best block hash
    pub fn get_best_block_hash(&self) -> [u8; 32] {
        self.chain_state.get_best_block_hash()
    }
}

impl TestNetwork {
    /// Create a new test network with specified number of nodes
    pub async fn new(node_count: usize) -> Result<Self, Box<dyn std::error::Error>> {
        let mut nodes = Vec::with_capacity(node_count);
        
        // Create nodes
        for i in 0..node_count {
            let node = TestNode::new(i).await?;
            nodes.push(node);
        }
        
        Ok(Self {
            nodes,
            connections: Vec::new(),
        })
    }
    
    /// Connect nodes in the network
    pub async fn connect_nodes(&mut self, from: usize, to: usize) -> Result<(), Box<dyn std::error::Error>> {
        if from >= self.nodes.len() || to >= self.nodes.len() {
            return Err("Invalid node index".into());
        }
        
        if from == to {
            return Err("Cannot connect node to itself".into());
        }
        
        // Check if connection already exists
        if self.connections.contains(&(from, to)) || self.connections.contains(&(to, from)) {
            return Ok(());
        }
        
        // In a real implementation, this would establish a P2P connection
        // For tests, we just record the connection
        self.connections.push((from, to));
        
        Ok(())
    }
    
    /// Create a fully connected network
    pub async fn connect_all(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        for i in 0..self.nodes.len() {
            for j in (i+1)..self.nodes.len() {
                self.connect_nodes(i, j).await?;
            }
        }
        
        Ok(())
    }
    
    /// Start all nodes
    pub async fn start_all(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        for node in &mut self.nodes {
            node.start().await?;
        }
        
        Ok(())
    }
    
    /// Stop all nodes
    pub async fn stop_all(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        for node in &mut self.nodes {
            node.stop().await?;
        }
        
        Ok(())
    }
    
    /// Wait for all nodes to reach the same height
    pub async fn wait_for_sync(&self, timeout_seconds: u64) -> Result<bool, Box<dyn std::error::Error>> {
        let timeout_duration = Duration::from_secs(timeout_seconds);
        
        let result = timeout(timeout_duration, async {
            loop {
                // Get heights of all nodes
                let heights: Vec<u64> = self.nodes.iter().map(|n| n.get_height()).collect();
                
                // Check if all heights are the same
                if heights.windows(2).all(|w| w[0] == w[1]) {
                    return true;
                }
                
                // Wait a bit before checking again
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
        }).await;
        
        match result {
            Ok(synced) => Ok(synced),
            Err(_) => Ok(false), // Timeout
        }
    }
    
    /// Mine a block on a specific node
    pub async fn mine_block_on_node(&self, node_index: usize) -> Result<Block, Box<dyn std::error::Error>> {
        if node_index >= self.nodes.len() {
            return Err("Invalid node index".into());
        }
        
        self.nodes[node_index].mine_block().await
    }
}

/// Create a network partition
pub async fn simulate_network_partition(group_a: &[TestNode], group_b: &[TestNode]) {
    // In a real implementation, this would disconnect nodes between groups
    // For tests, we just log the action
    info!("Simulating network partition between {} and {} nodes", 
        group_a.len(), group_b.len());
}

/// Heal a network partition
pub async fn heal_network_partition(group_a: &[TestNode], group_b: &[TestNode]) {
    // In a real implementation, this would reconnect nodes between groups
    // For tests, we just log the action
    info!("Healing network partition between {} and {} nodes", 
        group_a.len(), group_b.len());
}

/// Mine blocks on nodes in a partition
pub async fn mine_blocks_on_partition(nodes: &[TestNode], count: usize) -> Result<Vec<Block>, Box<dyn std::error::Error>> {
    if nodes.is_empty() {
        return Err("No nodes in partition".into());
    }
    
    let mut blocks = Vec::with_capacity(count);
    
    for _ in 0..count {
        // Mine on first node in partition
        let block = nodes[0].mine_block().await?;
        blocks.push(block.clone());
        
        // Propagate to other nodes in partition
        for node in &nodes[1..] {
            node.add_block(block.clone()).await?;
        }
    }
    
    Ok(blocks)
}

/// Get best block hash from a node
pub fn get_best_block_hash(node: &TestNode) -> [u8; 32] {
    node.get_best_block_hash()
}

/// Create a test network with specified number of nodes
pub async fn create_test_network(node_count: usize) -> Result<(Vec<TestNode>, TestNetwork), Box<dyn std::error::Error>> {
    let mut network = TestNetwork::new(node_count).await?;
    
    // Connect all nodes
    network.connect_all().await?;
    
    // Start all nodes
    network.start_all().await?;
    
    // Return nodes separately for easy access
    let nodes = network.nodes.clone();
    
    Ok((nodes, network))
}

#[cfg(test)]
mod tests {
    use super::*;
    use btclib::types::transaction::{Transaction, TransactionInput, TransactionOutput};
    
    #[tokio::test]
    async fn test_create_node() {
        let node = TestNode::new(0).await.unwrap();
        assert_eq!(node.id, 0);
        assert_eq!(node.get_height(), 0);
    }
    
    #[tokio::test]
    async fn test_create_network() {
        let network = TestNetwork::new(3).await.unwrap();
        assert_eq!(network.nodes.len(), 3);
        assert_eq!(network.connections.len(), 0);
    }
    
    #[tokio::test]
    async fn test_connect_nodes() {
        let mut network = TestNetwork::new(3).await.unwrap();
        
        network.connect_nodes(0, 1).await.unwrap();
        network.connect_nodes(1, 2).await.unwrap();
        
        assert_eq!(network.connections.len(), 2);
        assert!(network.connections.contains(&(0, 1)));
        assert!(network.connections.contains(&(1, 2)));
    }
    
    #[tokio::test]
    async fn test_mine_block() {
        let node = TestNode::new(0).await.unwrap();
        
        // Create and mine a block
        let block = node.mine_block().await.unwrap();
        
        // Check that height increased
        assert_eq!(node.get_height(), 1);
        assert_eq!(node.get_best_block_hash(), block.hash());
    }
    
    #[tokio::test]
    async fn test_add_transaction() {
        let node = TestNode::new(0).await.unwrap();
        
        // Create a test transaction
        let tx = Transaction::new(
            1,
            vec![TransactionInput::new([0u8; 32], 0, vec![], 0xffffffff)],
            vec![TransactionOutput::new(1_000_000, vec![1, 2, 3, 4, 5])],
            0,
        );
        
        // Add to mempool
        let result = node.add_transaction(tx.clone()).await.unwrap();
        assert!(result);
        
        // Mine a block with the transaction
        let block = node.mine_block().await.unwrap();
        
        // Verify block has our transaction
        assert!(block.transactions().iter().any(|t| t.hash() == tx.hash()));
    }
} 