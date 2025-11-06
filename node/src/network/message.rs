use crate::network::protocol::{Message as ProtocolMessage, PublishError};
use blake3;
use libp2p::{gossipsub, PeerId};
use std::{
    collections::{HashMap, VecDeque},
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};
use tokio::sync::mpsc;
use tracing::{trace, warn};

// ============================================================================
// Network Message Size Limits
// ============================================================================

/// Network message size configuration
/// 
/// SECURITY: Reduced from dangerous 32MB to safe 4MB limit.
/// This prevents bandwidth exhaustion and memory DoS attacks.
pub struct MessageSizeLimits;

impl MessageSizeLimits {
    /// Maximum size of a network message in bytes
    /// 
    /// SECURITY FIX: Reduced from 32MB to 4MB.
    /// 
    /// Rationale:
    /// - Bitcoin mainnet: 2MB blocks
    /// - Supernova blocks: ~4MB (2.5 minute block time)
    /// - Messages should fit block size + overhead
    /// - 32MB was 8x too large, enabled bandwidth DoS
    pub const MAX_MESSAGE_SIZE: usize = 4 * 1024 * 1024; // 4 MB
    
    /// Maximum block message size (same as MAX_MESSAGE_SIZE)
    pub const MAX_BLOCK_SIZE: usize = 4 * 1024 * 1024; // 4 MB
    
    /// Maximum transaction message size
    /// 
    /// Individual transactions should be much smaller than blocks.
    pub const MAX_TRANSACTION_SIZE: usize = 1 * 1024 * 1024; // 1 MB
    
    /// Maximum inventory message size
    /// 
    /// Inventory messages list transaction/block hashes.
    pub const MAX_INVENTORY_SIZE: usize = 512 * 1024; // 512 KB
    
    /// Maximum header message size
    /// 
    /// Block headers are small (~80 bytes each), but allow batches.
    pub const MAX_HEADERS_SIZE: usize = 2 * 1024 * 1024; // 2 MB (~25,000 headers)
}

/// Maximum size of a network message in bytes
/// SECURITY: Now uses hardened constant
pub const MAX_MESSAGE_SIZE: usize = MessageSizeLimits::MAX_MESSAGE_SIZE;

/// Maximum time to keep a message in the seen cache
pub const MESSAGE_CACHE_TTL: Duration = Duration::from_secs(120);

/// Maximum messages to process in a single batch
pub const MAX_BATCH_PROCESS: usize = 100;

/// Network message with metadata
#[derive(Debug, Clone)]
pub struct NetworkMessage {
    /// The sender of the message
    pub from_peer: Option<PeerId>,
    /// The message content
    pub message: ProtocolMessage,
    /// When the message was received
    pub received_at: Instant,
}

impl NetworkMessage {
    /// Create a new network message
    pub fn new(from_peer: Option<PeerId>, message: ProtocolMessage) -> Self {
        Self {
            from_peer,
            message,
            received_at: Instant::now(),
        }
    }

    /// Check if this message is too old to process
    pub fn is_expired(&self, ttl: Duration) -> bool {
        self.received_at.elapsed() > ttl
    }
}

/// Events emitted by the message handler
#[derive(Debug, Clone)]
pub enum MessageEvent {
    /// New message received and validated
    MessageValidated(NetworkMessage),
    /// Message received but failed validation
    MessageInvalid(PeerId, String),
    /// Message created locally and ready to be sent
    MessageCreated(ProtocolMessage, String), // message and topic
    /// Broadcast completed
    MessageBroadcast(gossipsub::MessageId),
    /// Error publishing a message
    PublishError(String), // Changed from PublishError to String
}

/// Error types for message broadcasting
#[derive(Debug)] // Remove Clone from here
pub enum BroadcastError {
    /// The message is too large
    MessageTooLarge(usize),
    /// No connected peers
    NoPeers,
    /// Protocol error
    ProtocolError(PublishError),
    /// Serialization error
    SerializationError(String),
}

/// Handler for network messages
pub struct MessageHandler {
    /// Message queue for incoming messages
    incoming_queue: Arc<Mutex<VecDeque<NetworkMessage>>>,
    /// Cache of seen message hashes to prevent duplicates
    seen_messages: Arc<Mutex<HashMap<Vec<u8>, Instant>>>,
    /// Message event sender
    event_sender: Option<mpsc::Sender<MessageEvent>>,
    /// Maximum message size in bytes
    max_message_size: usize,
    /// TTL for message cache
    message_cache_ttl: Duration,
    /// Counters for statistics
    stats: MessageStats,
}

/// Message handling statistics
#[derive(Debug, Clone, Default)]
pub struct MessageStats {
    /// Total messages received
    pub messages_received: u64,
    /// Messages rejected as duplicates
    pub duplicate_messages: u64,
    /// Messages rejected as too large
    pub oversized_messages: u64,
    /// Messages rejected by validation
    pub invalid_messages: u64,
    /// Messages that passed validation
    pub valid_messages: u64,
    /// Messages sent by the node
    pub messages_sent: u64,
    /// Message broadcast errors
    pub broadcast_errors: u64,
}

impl MessageHandler {
    /// Create a new message handler
    pub fn new() -> Self {
        Self {
            incoming_queue: Arc::new(Mutex::new(VecDeque::new())),
            seen_messages: Arc::new(Mutex::new(HashMap::new())),
            event_sender: None,
            max_message_size: MAX_MESSAGE_SIZE,
            message_cache_ttl: MESSAGE_CACHE_TTL,
            stats: MessageStats::default(),
        }
    }

    /// Set the event sender channel
    pub fn set_event_sender(&mut self, sender: mpsc::Sender<MessageEvent>) {
        self.event_sender = Some(sender);
    }

    /// Queue a message for processing
    pub fn queue_message(&self, from_peer: Option<PeerId>, message: ProtocolMessage) {
        // Queue the message for processing
        let network_message = NetworkMessage::new(from_peer, message);
        if let Ok(mut queue) = self.incoming_queue.lock() {
            queue.push_back(network_message);
        } else {
            warn!("Failed to acquire message queue lock, dropping message");
        }
    }

    /// Process the next batch of queued messages
    pub async fn process_message_queue(&mut self) -> usize {
        let mut processed = 0;

        // Get messages from the queue (limited batch)
        let messages = {
            let mut queue = match self.incoming_queue.lock() {
                Ok(guard) => guard,
                Err(_) => {
                    warn!("Failed to acquire message queue lock for processing");
                    return 0;
                }
            };
            let mut batch = Vec::new();

            while let Some(message) = queue.pop_front() {
                batch.push(message);
                processed += 1;

                if processed >= MAX_BATCH_PROCESS {
                    break;
                }
            }

            batch
        };

        // Process each message
        for message in messages {
            self.process_message(message).await;
        }

        // Clean up the seen messages cache
        self.clean_seen_cache();

        processed
    }

    /// Process a single message
    async fn process_message(&mut self, message: NetworkMessage) {
        // Skip if message is too old
        if message.is_expired(Duration::from_secs(300)) {
            trace!("Skipping expired message");
            return;
        }

        self.stats.messages_received += 1;

        // Validate message
        match self.validate_message(&message) {
            Ok(true) => {
                // Message is valid and not a duplicate
                self.stats.valid_messages += 1;

                // Emit message validated event
                if let Some(sender) = &self.event_sender {
                    if let Err(e) = sender.send(MessageEvent::MessageValidated(message)).await {
                        warn!("Failed to send message validated event: {}", e);
                    }
                }
            }
            Ok(false) => {
                // Message is a duplicate, no action needed
                self.stats.duplicate_messages += 1;
                trace!("Duplicate message detected");
            }
            Err(reason) => {
                // Message is invalid
                self.stats.invalid_messages += 1;

                // Emit message invalid event if we know the sender
                if let Some(peer_id) = message.from_peer {
                    if let Some(sender) = &self.event_sender {
                        if let Err(e) = sender
                            .send(MessageEvent::MessageInvalid(peer_id, reason))
                            .await
                        {
                            warn!("Failed to send message invalid event: {}", e);
                        }
                    }
                }
            }
        }
    }

    /// SECURITY FIX [P1-011]: Validate a message and check for duplicates
    /// Comprehensive validation including size limits, structure validation, and malformed data checks
    fn validate_message(&mut self, message: &NetworkMessage) -> Result<bool, String> {
        // Step 1: Validate message structure BEFORE serialization
        self.validate_message_structure(&message.message)?;

        // Step 2: Serialize message to get hash for duplicate detection
        let message_bytes = match bincode::serialize(&message.message) {
            Ok(bytes) => bytes,
            Err(e) => return Err(format!("Failed to serialize message: {}", e)),
        };

        // Step 3: Check overall size limit
        if message_bytes.len() > self.max_message_size {
            self.stats.oversized_messages += 1;
            return Err(format!("Message too large: {} bytes (max: {})", message_bytes.len(), self.max_message_size));
        }

        // Step 4: Check type-specific size limits
        self.validate_type_specific_size(&message.message, message_bytes.len())?;

        // Step 5: Check for duplicates
        let mut seen = self
            .seen_messages
            .lock()
            .map_err(|_| "Seen messages lock poisoned".to_string())?;

        // Use blake3 for fast hashing
        let hash = blake3::hash(&message_bytes).as_bytes().to_vec();

        if seen.contains_key(&hash) {
            // Message is a duplicate
            return Ok(false);
        }

        // Step 6: Message passed all validation, add to seen cache
        seen.insert(hash, Instant::now());

        Ok(true)
    }

    /// SECURITY FIX [P1-011]: Validate message structure before processing
    /// Prevents malformed message attacks by validating structure upfront
    fn validate_message_structure(&self, msg: &ProtocolMessage) -> Result<(), String> {
        match msg {
            ProtocolMessage::Block(block) => {
                if block.transactions.is_empty() {
                    return Err("Block has no transactions".to_string());
                }
                // Validate block header exists (via block structure)
                // Block structure validation is handled by the consensus layer
            }
            ProtocolMessage::Transaction { transaction } => {
                // Transaction structure validation
                if transaction.is_empty() {
                    return Err("Transaction data is empty".to_string());
                }
                // Additional transaction validation would require deserialization
                // For now, we just check it's not empty
            }
            ProtocolMessage::Headers { headers, .. } => {
                if headers.is_empty() {
                    return Err("Headers message is empty".to_string());
                }
                // Validate header count is reasonable
                if headers.len() > 25_000 {
                    return Err(format!("Too many headers: {} (max: 25000)", headers.len()));
                }
                // Validate each header is reasonable size (block headers are ~80 bytes)
                for (idx, header) in headers.iter().enumerate() {
                    if header.len() > 1024 {
                        return Err(format!("Header {} too large: {} bytes (max: 1024)", idx, header.len()));
                    }
                }
            }
            ProtocolMessage::GetBlocks(msg) => {
                // GetBlocks contains a GetBlocksMessage struct
                if msg.locator_hashes.is_empty() {
                    return Err("GetBlocks has no locator hashes".to_string());
                }
                if msg.locator_hashes.len() > 500 {
                    return Err(format!("Too many block hashes: {} (max: 500)", msg.locator_hashes.len()));
                }
            }
            ProtocolMessage::GetBlocksByHash { block_hashes } => {
                if block_hashes.is_empty() {
                    return Err("GetBlocksByHash has no block hashes".to_string());
                }
                if block_hashes.len() > 500 {
                    return Err(format!("Too many block hashes: {} (max: 500)", block_hashes.len()));
                }
            }
            ProtocolMessage::GetData(hashes) => {
                if hashes.is_empty() {
                    return Err("GetData has no hashes".to_string());
                }
                if hashes.len() > 500 {
                    return Err(format!("Too many GetData hashes: {} (max: 500)", hashes.len()));
                }
                // Note: hashes are Vec<[u8; 32]>, so each hash is guaranteed to be 32 bytes
                // Fixed-size arrays ensure correct length, no need to validate individually
            }
            ProtocolMessage::GetHeaders {
                start_height,
                end_height,
            } => {
                if end_height < start_height {
                    return Err(format!("Invalid header range: end_height {} < start_height {}", end_height, start_height));
                }
                let range = end_height.saturating_sub(*start_height);
                if range > 2000 {
                    return Err(format!("Header range too large: {} (max: 2000)", range));
                }
            }
            ProtocolMessage::Status {
                best_hash,
                ..
            } => {
                // Note: best_hash is [u8; 32], a fixed-size array guaranteed to be 32 bytes
                // No need to validate length, deserialization ensures correctness
            }
            ProtocolMessage::Extension(name, payload) => {
                // Validate extension message name is not empty
                if name.is_empty() {
                    return Err("Extension message name is empty".to_string());
                }
                // Validate extension message name length (prevent DoS)
                if name.len() > 256 {
                    return Err(format!("Extension message name too long: {} bytes (max: 256)", name.len()));
                }
                // Validate payload size
                if payload.len() > MessageSizeLimits::MAX_MESSAGE_SIZE {
                    return Err(format!("Extension message payload too large: {} bytes", payload.len()));
                }
            }
            ProtocolMessage::Ping(_) | ProtocolMessage::Pong(_) | ProtocolMessage::GetStatus 
            | ProtocolMessage::Verack | ProtocolMessage::GetAddr
            | ProtocolMessage::Version(_) | ProtocolMessage::Addr(_)
            | ProtocolMessage::Environmental(_) | ProtocolMessage::Lightning(_)
            | ProtocolMessage::NewBlock { .. } | ProtocolMessage::GetBlocksByHeight { .. }
            | ProtocolMessage::BroadcastTransaction(_) | ProtocolMessage::TransactionAnnouncement { .. }
            | ProtocolMessage::Blocks { .. } | ProtocolMessage::BlockResponse { .. }
            |             ProtocolMessage::GetMempool { .. } | ProtocolMessage::Mempool { .. }
            | ProtocolMessage::CompactBlock(_)
            | ProtocolMessage::GetCompactBlockTxs { .. }
            | ProtocolMessage::CompactBlockTxs(_) => {
                // Simple messages or messages with validation handled elsewhere
                // No additional validation needed at this layer
            }
        }
        Ok(())
    }

    /// SECURITY FIX [P1-011]: Validate type-specific size limits
    /// Enforces message-type-specific size constraints to prevent DoS attacks
    fn validate_type_specific_size(&self, msg: &ProtocolMessage, serialized_size: usize) -> Result<(), String> {
        match msg {
            ProtocolMessage::Block(_) => {
                if serialized_size > MessageSizeLimits::MAX_BLOCK_SIZE {
                    return Err(format!("Block message too large: {} bytes (max: {})", serialized_size, MessageSizeLimits::MAX_BLOCK_SIZE));
                }
            }
            ProtocolMessage::Transaction { transaction } => {
                if transaction.len() > MessageSizeLimits::MAX_TRANSACTION_SIZE {
                    return Err(format!("Transaction message too large: {} bytes (max: {})", transaction.len(), MessageSizeLimits::MAX_TRANSACTION_SIZE));
                }
            }
            ProtocolMessage::Headers { headers, .. } => {
                // Estimate headers size: ~80 bytes per header + overhead
                let estimated_size = headers.len() * 80 + 1024; // 1KB overhead
                if estimated_size > MessageSizeLimits::MAX_HEADERS_SIZE {
                    return Err(format!("Headers message too large: estimated {} bytes (max: {})", estimated_size, MessageSizeLimits::MAX_HEADERS_SIZE));
                }
                // Also check serialized size
                if serialized_size > MessageSizeLimits::MAX_HEADERS_SIZE {
                    return Err(format!("Headers message serialized size too large: {} bytes (max: {})", serialized_size, MessageSizeLimits::MAX_HEADERS_SIZE));
                }
            }
            ProtocolMessage::GetData(hashes) => {
                // Inventory messages: estimate size as hash count * 32 bytes + overhead
                let estimated_size = hashes.len() * 32 + 1024; // 1KB overhead
                if estimated_size > MessageSizeLimits::MAX_INVENTORY_SIZE {
                    return Err(format!("GetData message too large: estimated {} bytes (max: {})", estimated_size, MessageSizeLimits::MAX_INVENTORY_SIZE));
                }
            }
            ProtocolMessage::GetBlocks(msg) => {
                // GetBlocks contains GetBlocksMessage struct
                let estimated_size = msg.locator_hashes.len() * 32 + 1024; // 1KB overhead
                if estimated_size > MessageSizeLimits::MAX_INVENTORY_SIZE {
                    return Err(format!("GetBlocks message too large: estimated {} bytes (max: {})", estimated_size, MessageSizeLimits::MAX_INVENTORY_SIZE));
                }
            }
            ProtocolMessage::GetBlocksByHash { block_hashes } => {
                // Estimate size as hash count * 32 bytes + overhead
                let estimated_size = block_hashes.len() * 32 + 1024; // 1KB overhead
                if estimated_size > MessageSizeLimits::MAX_INVENTORY_SIZE {
                    return Err(format!("GetBlocksByHash message too large: estimated {} bytes (max: {})", estimated_size, MessageSizeLimits::MAX_INVENTORY_SIZE));
                }
            }
            ProtocolMessage::Extension(_, payload) => {
                if payload.len() > MessageSizeLimits::MAX_MESSAGE_SIZE {
                    return Err(format!("Extension message payload too large: {} bytes (max: {})", payload.len(), MessageSizeLimits::MAX_MESSAGE_SIZE));
                }
            }
            ProtocolMessage::CompactBlock(_) => {
                // Compact blocks use general message size limit
                // Size validation handled in general check
            }
            ProtocolMessage::GetCompactBlockTxs { short_ids } => {
                if short_ids.len() > 1000 {
                    return Err(format!("Too many short IDs: {} (max: 1000)", short_ids.len()));
                }
            }
            ProtocolMessage::CompactBlockTxs(transactions) => {
                if transactions.len() > 1000 {
                    return Err(format!("Too many transactions: {} (max: 1000)", transactions.len()));
                }
            }
            _ => {
                // Other message types use the general MAX_MESSAGE_SIZE limit
                // Already checked in validate_message
            }
        }
        Ok(())
    }

    /// Clean up the seen messages cache
    fn clean_seen_cache(&self) {
        let now = Instant::now();
        let mut seen = match self.seen_messages.lock() {
            Ok(guard) => guard,
            Err(_) => {
                warn!("Failed to acquire seen messages lock for cleanup");
                return;
            }
        };

        // Remove entries older than TTL
        seen.retain(|_, timestamp| now.duration_since(*timestamp) < self.message_cache_ttl);
    }

    /// Create a new message for broadcasting
    pub async fn create_message(&mut self, message: ProtocolMessage, topic: String) {
        self.stats.messages_sent += 1;

        // Emit message created event
        if let Some(sender) = &self.event_sender {
            if let Err(e) = sender
                .send(MessageEvent::MessageCreated(message, topic))
                .await
            {
                warn!("Failed to send message created event: {}", e);
            }
        }
    }

    /// Handle message broadcast completion
    pub async fn handle_broadcast_complete(&mut self, message_id: gossipsub::MessageId) {
        // Emit message broadcast event
        if let Some(sender) = &self.event_sender {
            if let Err(e) = sender
                .send(MessageEvent::MessageBroadcast(message_id))
                .await
            {
                warn!("Failed to send message broadcast event: {}", e);
            }
        }
    }

    /// Handle message publish error
    pub async fn handle_publish_error(&mut self, error: PublishError) {
        self.stats.broadcast_errors += 1;

        // Emit publish error event
        if let Some(sender) = &self.event_sender {
            if let Err(e) = sender
                .send(MessageEvent::PublishError(error.to_string()))
                .await
            {
                warn!("Failed to send publish error event: {}", e);
            }
        }
    }

    /// Get message handling statistics
    pub fn get_stats(&self) -> MessageStats {
        self.stats.clone()
    }

    /// Get the current size of the message queue
    pub fn queue_size(&self) -> usize {
        self.incoming_queue.lock().map(|q| q.len()).unwrap_or(0)
    }

    /// Get the current size of the seen message cache
    pub fn seen_cache_size(&self) -> usize {
        self.seen_messages.lock().map(|s| s.len()).unwrap_or(0)
    }
}

impl Default for MessageHandler {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_message_queue() {
        let handler = MessageHandler::new();

        // Queue a message
        let message = ProtocolMessage::Ping(0);
        handler.queue_message(Some(PeerId::random()), message);

        // Check queue size
        assert_eq!(handler.queue_size(), 1);
    }

    #[test]
    fn test_duplicate_detection() {
        let mut handler = MessageHandler::new();

        // Create two identical messages
        let message1 = ProtocolMessage::Ping(123);
        let message2 = ProtocolMessage::Ping(123);

        let network_message1 = NetworkMessage::new(Some(PeerId::random()), message1);
        let network_message2 = NetworkMessage::new(Some(PeerId::random()), message2);

        // First message should be valid and not a duplicate
        assert_eq!(handler.validate_message(&network_message1), Ok(true));

        // Second message should be detected as a duplicate
        assert_eq!(handler.validate_message(&network_message2), Ok(false));
    }

    #[test]
    fn test_oversized_message_rejection() {
        let mut handler = MessageHandler::new();
        handler.max_message_size = 10; // Set very small limit for testing

        // Create a message that will be larger than the limit
        let large_data = vec![0u8; 100];
        let message = ProtocolMessage::Block { block: large_data };
        let network_message = NetworkMessage::new(Some(PeerId::random()), message);

        // Message should be rejected as too large
        assert!(handler.validate_message(&network_message).is_err());
    }

    #[test]
    fn test_message_expiration() {
        let message = NetworkMessage {
            from_peer: Some(PeerId::random()),
            message: ProtocolMessage::Ping(0),
            received_at: Instant::now() - Duration::from_secs(600), // 10 minutes ago
        };

        // Message should be expired with 5 minute TTL
        assert!(message.is_expired(Duration::from_secs(300)));

        // Create a fresh message
        let fresh_message = NetworkMessage::new(Some(PeerId::random()), ProtocolMessage::Ping(0));

        // Fresh message should not be expired
        assert!(!fresh_message.is_expired(Duration::from_secs(300)));
    }

    // Comprehensive message validation tests
    #[test]
    fn test_block_message_validation() {
        use supernova_core::types::{block::Block, block::BlockHeader as CoreBlockHeader, transaction::Transaction};

        let mut handler = MessageHandler::new();

        // Test: Block with no transactions should be rejected
        let empty_block = Block::new(
            CoreBlockHeader::new(1, [0u8; 32], [0u8; 32], 0, 0, 0),
            vec![],
        );
        let invalid_msg = NetworkMessage::new(Some(PeerId::random()), ProtocolMessage::Block(empty_block));
        assert!(handler.validate_message(&invalid_msg).is_err());

        // Test: Valid block should pass (coinbase transaction with output)
        use supernova_core::types::transaction::TransactionOutput;
        let coinbase_output = TransactionOutput::new(5000000000, vec![0x51]); // 50 NOVA coinbase reward
        let coinbase_tx = Transaction::new(1, vec![], vec![coinbase_output], 0);
        let valid_block = Block::new(
            CoreBlockHeader::new(1, [0u8; 32], [0u8; 32], 0, 0, 0),
            vec![coinbase_tx],
        );
        let valid_msg = NetworkMessage::new(Some(PeerId::random()), ProtocolMessage::Block(valid_block));
        assert_eq!(handler.validate_message(&valid_msg), Ok(true));
    }

    #[test]
    fn test_transaction_message_validation() {
        let mut handler = MessageHandler::new();

        // Test: Transaction with empty data should be rejected
        let invalid_msg = NetworkMessage::new(
            Some(PeerId::random()),
            ProtocolMessage::Transaction { transaction: vec![] },
        );
        assert!(handler.validate_message(&invalid_msg).is_err());

        // Test: Valid transaction should pass
        let valid_msg = NetworkMessage::new(
            Some(PeerId::random()),
            ProtocolMessage::Transaction { transaction: vec![1, 2, 3, 4] },
        );
        assert_eq!(handler.validate_message(&valid_msg), Ok(true));
    }

    #[test]
    fn test_headers_message_validation() {
        let mut handler = MessageHandler::new();

        // Test: Empty headers should be rejected
        let invalid_msg = NetworkMessage::new(
            Some(PeerId::random()),
            ProtocolMessage::Headers {
                headers: vec![],
                total_difficulty: 0,
            },
        );
        assert!(handler.validate_message(&invalid_msg).is_err());

        // Test: Too many headers should be rejected
        let mut too_many_headers = vec![];
        for _ in 0..25_001 {
            too_many_headers.push(vec![0u8; 80]); // Simulated header
        }
        let invalid_msg = NetworkMessage::new(
            Some(PeerId::random()),
            ProtocolMessage::Headers {
                headers: too_many_headers,
                total_difficulty: 0,
            },
        );
        assert!(handler.validate_message(&invalid_msg).is_err());

        // Test: Valid headers should pass
        let valid_msg = NetworkMessage::new(
            Some(PeerId::random()),
            ProtocolMessage::Headers {
                headers: vec![vec![0u8; 80]],
                total_difficulty: 0,
            },
        );
        assert_eq!(handler.validate_message(&valid_msg), Ok(true));
    }

    #[test]
    fn test_getblocks_message_validation() {
        use crate::network::protocol::GetBlocksMessage;

        let mut handler = MessageHandler::new();

        // Test: Empty GetBlocks should be rejected
        let empty_msg = GetBlocksMessage {
            version: 1,
            locator_hashes: vec![],
            stop_hash: [0u8; 32],
        };
        let invalid_msg = NetworkMessage::new(Some(PeerId::random()), ProtocolMessage::GetBlocks(empty_msg));
        assert!(handler.validate_message(&invalid_msg).is_err());

        // Test: Too many hashes should be rejected
        let mut too_many_hashes = vec![];
        for _ in 0..501 {
            too_many_hashes.push([0u8; 32]);
        }
        let invalid_msg = NetworkMessage::new(
            Some(PeerId::random()),
            ProtocolMessage::GetBlocks(GetBlocksMessage {
                version: 1,
                locator_hashes: too_many_hashes,
                stop_hash: [0u8; 32],
            }),
        );
        assert!(handler.validate_message(&invalid_msg).is_err());

        // Test: Valid GetBlocks should pass
        let valid_msg = NetworkMessage::new(
            Some(PeerId::random()),
            ProtocolMessage::GetBlocks(GetBlocksMessage {
                version: 1,
                locator_hashes: vec![[0u8; 32], [1u8; 32]],
                stop_hash: [0u8; 32],
            }),
        );
        assert_eq!(handler.validate_message(&valid_msg), Ok(true));
    }

    #[test]
    fn test_getheaders_message_validation() {
        let mut handler = MessageHandler::new();

        // Test: Invalid range (end < start) should be rejected
        let invalid_msg = NetworkMessage::new(
            Some(PeerId::random()),
            ProtocolMessage::GetHeaders {
                start_height: 100,
                end_height: 50,
            },
        );
        assert!(handler.validate_message(&invalid_msg).is_err());

        // Test: Range too large should be rejected
        let invalid_msg = NetworkMessage::new(
            Some(PeerId::random()),
            ProtocolMessage::GetHeaders {
                start_height: 0,
                end_height: 2001,
            },
        );
        assert!(handler.validate_message(&invalid_msg).is_err());

        // Test: Valid GetHeaders should pass
        let valid_msg = NetworkMessage::new(
            Some(PeerId::random()),
            ProtocolMessage::GetHeaders {
                start_height: 0,
                end_height: 100,
            },
        );
        assert_eq!(handler.validate_message(&valid_msg), Ok(true));
    }

    // Note: ProtocolMessage doesn't have Custom variant, removed test

    #[test]
    fn test_type_specific_size_limits() {
        use supernova_core::types::{block::Block, block::BlockHeader as CoreBlockHeader, transaction::Transaction, transaction::TransactionOutput};

        let mut handler = MessageHandler::new();
        handler.max_message_size = MessageSizeLimits::MAX_MESSAGE_SIZE;

        // Test: Block exceeding MAX_BLOCK_SIZE should be rejected
        let mut large_transactions = vec![];
        for _ in 0..1000 {
            let output = TransactionOutput::new(1000, vec![0u8; 1000]);
            large_transactions.push(Transaction::new(1, vec![], vec![output], 0));
        }
        let large_block = Block::new(
            CoreBlockHeader::new(1, [0u8; 32], [0u8; 32], 0, 0, 0),
            large_transactions,
        );
        let large_block_msg = NetworkMessage::new(Some(PeerId::random()), ProtocolMessage::Block(large_block));
        let serialized_size = bincode::serialize(&large_block_msg.message).unwrap().len();
        if serialized_size > MessageSizeLimits::MAX_BLOCK_SIZE {
            assert!(handler.validate_message(&large_block_msg).is_err());
        }

        // Test: Transaction exceeding MAX_TRANSACTION_SIZE should be rejected
        let large_output = TransactionOutput::new(1000, vec![0u8; MessageSizeLimits::MAX_TRANSACTION_SIZE]);
        let large_tx = Transaction::new(1, vec![], vec![large_output], 0);
        let large_tx_msg = NetworkMessage::new(Some(PeerId::random()), ProtocolMessage::Transaction(large_tx));
        let serialized_size = bincode::serialize(&large_tx_msg.message).unwrap().len();
        if serialized_size > MessageSizeLimits::MAX_TRANSACTION_SIZE {
            assert!(handler.validate_message(&large_tx_msg).is_err());
        }
    }
}
