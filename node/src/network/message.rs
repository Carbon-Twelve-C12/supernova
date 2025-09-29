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

/// Maximum size of a network message in bytes
pub const MAX_MESSAGE_SIZE: usize = 32 * 1024 * 1024; // 32 MB

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

    /// Validate a message and check for duplicates
    fn validate_message(&mut self, message: &NetworkMessage) -> Result<bool, String> {
        // Serialize message to get hash for duplicate detection
        let message_bytes = match bincode::serialize(&message.message) {
            Ok(bytes) => bytes,
            Err(e) => return Err(format!("Failed to serialize message: {}", e)),
        };

        // Check size limit
        if message_bytes.len() > self.max_message_size {
            self.stats.oversized_messages += 1;
            return Err(format!("Message too large: {} bytes", message_bytes.len()));
        }

        // Check for duplicates
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

        // Basic validation based on message type
        match &message.message {
            ProtocolMessage::Block(block) => {
                if block.transactions.is_empty() {
                    return Err("Block has no transactions".to_string());
                }
            }
            ProtocolMessage::Transaction { transaction } => {
                // Transaction is just raw bytes, check if it's empty
                if transaction.is_empty() {
                    return Err("Empty transaction data".to_string());
                }
            }
            ProtocolMessage::GetBlocks(msg) => {
                // GetBlocks contains a GetBlocksMessage with locator hashes
                if msg.locator_hashes.is_empty() {
                    return Err("GetBlocks has no locator hashes".to_string());
                }
                if msg.locator_hashes.len() > 500 {
                    return Err("Too many locator hashes".to_string());
                }
            }
            ProtocolMessage::GetHeaders {
                start_height,
                end_height,
            } => {
                if end_height < start_height {
                    return Err("Invalid header range".to_string());
                }
                if end_height - start_height > 2000 {
                    return Err("Header range too large".to_string());
                }
            }
            // Add validation for other message types as needed
            _ => {}
        }

        // Message passed validation, add to seen cache
        seen.insert(hash, Instant::now());

        Ok(true)
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
}
