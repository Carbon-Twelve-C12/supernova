//! WebSocket notification system for atomic swap events
//!
//! This module provides real-time notifications for swap state changes
//! and blockchain events to connected clients.

use crate::atomic_swap::monitor::SwapEvent;
use tokio::sync::mpsc;
use tokio::sync::RwLock;
use std::collections::HashMap;
use std::sync::Arc;
use serde::{Serialize, Deserialize};
use uuid::Uuid;

/// WebSocket message types
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum WsMessage {
    /// Subscribe to swap events
    Subscribe {
        swap_id: Option<String>, // None = subscribe to all
    },
    
    /// Unsubscribe from events
    Unsubscribe {
        swap_id: Option<String>,
    },
    
    /// Swap event notification
    SwapEvent {
        event: SwapEvent,
    },
    
    /// Heartbeat/ping
    Ping,
    
    /// Heartbeat/pong
    Pong,
    
    /// Error message
    Error {
        code: i32,
        message: String,
    },
}

/// WebSocket client connection
pub struct WsClient {
    /// Client ID
    pub id: Uuid,
    
    /// Send channel for messages
    pub sender: mpsc::UnboundedSender<WsMessage>,
    
    /// Subscribed swap IDs (None = all swaps)
    pub subscriptions: Vec<Option<[u8; 32]>>,
}

/// WebSocket notification manager
pub struct WsNotificationManager {
    /// Connected clients
    clients: Arc<RwLock<HashMap<Uuid, WsClient>>>,
    
    /// Event receiver from monitor
    event_rx: mpsc::UnboundedReceiver<SwapEvent>,
}

impl WsNotificationManager {
    /// Create a new WebSocket notification manager
    pub fn new(event_rx: mpsc::UnboundedReceiver<SwapEvent>) -> Self {
        Self {
            clients: Arc::new(RwLock::new(HashMap::new())),
            event_rx,
        }
    }
    
    /// Add a new client connection
    pub async fn add_client(&self, client_id: Uuid, sender: mpsc::UnboundedSender<WsMessage>) {
        let mut clients = self.clients.write().await;
        clients.insert(client_id, WsClient {
            id: client_id,
            sender,
            subscriptions: vec![None], // Subscribe to all by default
        });
        
        log::info!("WebSocket client {} connected", client_id);
    }
    
    /// Remove a client connection
    pub async fn remove_client(&self, client_id: &Uuid) {
        let mut clients = self.clients.write().await;
        clients.remove(client_id);
        
        log::info!("WebSocket client {} disconnected", client_id);
    }
    
    /// Handle client subscription
    pub async fn handle_subscription(&self, client_id: &Uuid, swap_id: Option<[u8; 32]>) {
        let mut clients = self.clients.write().await;
        if let Some(client) = clients.get_mut(client_id) {
            if !client.subscriptions.contains(&swap_id) {
                client.subscriptions.push(swap_id);
            }
        }
    }
    
    /// Handle client unsubscription
    pub async fn handle_unsubscription(&self, client_id: &Uuid, swap_id: Option<[u8; 32]>) {
        let mut clients = self.clients.write().await;
        if let Some(client) = clients.get_mut(client_id) {
            client.subscriptions.retain(|&sub| sub != swap_id);
        }
    }
    
    /// Start the notification service
    pub async fn start(&mut self) {
        log::info!("Starting WebSocket notification service");
        
        while let Some(event) = self.event_rx.recv().await {
            self.broadcast_event(event).await;
        }
    }
    
    /// Broadcast an event to subscribed clients
    async fn broadcast_event(&self, event: SwapEvent) {
        let swap_id = match &event {
            SwapEvent::SwapInitiated { swap_id, .. } |
            SwapEvent::HTLCFunded { swap_id, .. } |
            SwapEvent::SecretRevealed { swap_id, .. } |
            SwapEvent::SwapCompleted { swap_id, .. } |
            SwapEvent::SwapRefunded { swap_id, .. } => Some(*swap_id),
        };
        
        let clients = self.clients.read().await;
        let message = WsMessage::SwapEvent { event };
        
        for client in clients.values() {
            // Check if client is subscribed to this event
            let is_subscribed = client.subscriptions.iter().any(|sub| {
                sub.is_none() || // Subscribed to all
                (swap_id.is_some() && sub.as_ref() == swap_id.as_ref())
            });
            
            if is_subscribed {
                if let Err(e) = client.sender.send(message.clone()) {
                    log::warn!("Failed to send message to client {}: {}", client.id, e);
                }
            }
        }
    }
}

/// WebSocket connection handler
pub async fn handle_ws_connection(
    client_id: Uuid,
    mut receiver: mpsc::UnboundedReceiver<WsMessage>,
    sender: mpsc::UnboundedSender<WsMessage>,
    notification_manager: Arc<WsNotificationManager>,
) {
    // Add client to manager
    notification_manager.add_client(client_id, sender.clone()).await;
    
    // Handle incoming messages
    while let Some(msg) = receiver.recv().await {
        match msg {
            WsMessage::Subscribe { swap_id } => {
                let swap_id_bytes = swap_id.and_then(|s| {
                    hex::decode(s).ok().and_then(|bytes| {
                        let mut arr = [0u8; 32];
                        if bytes.len() == 32 {
                            arr.copy_from_slice(&bytes);
                            Some(arr)
                        } else {
                            None
                        }
                    })
                });
                
                notification_manager.handle_subscription(&client_id, swap_id_bytes).await;
            }
            
            WsMessage::Unsubscribe { swap_id } => {
                let swap_id_bytes = swap_id.and_then(|s| {
                    hex::decode(s).ok().and_then(|bytes| {
                        let mut arr = [0u8; 32];
                        if bytes.len() == 32 {
                            arr.copy_from_slice(&bytes);
                            Some(arr)
                        } else {
                            None
                        }
                    })
                });
                
                notification_manager.handle_unsubscription(&client_id, swap_id_bytes).await;
            }
            
            WsMessage::Ping => {
                if sender.send(WsMessage::Pong).is_err() {
                    break;
                }
            }
            
            _ => {
                // Ignore other message types from client
            }
        }
    }
    
    // Remove client on disconnect
    notification_manager.remove_client(&client_id).await;
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_ws_notification_manager() {
        let (event_tx, event_rx) = mpsc::unbounded_channel();
        let manager = Arc::new(tokio::sync::Mutex::new(WsNotificationManager::new(event_rx)));
        
        // Create test client
        let client_id = Uuid::new_v4();
        let (tx, mut rx) = mpsc::unbounded_channel();
        
        manager.lock().await.add_client(client_id, tx).await;
        
        // Send test event
        let test_event = SwapEvent::SwapInitiated {
            swap_id: [1u8; 32],
            initiator: "alice".to_string(),
            participant: "bob".to_string(),
            amounts: crate::atomic_swap::monitor::SwapAmounts {
                bitcoin_sats: 100000,
                nova_units: 1000000,
            },
        };
        
        event_tx.send(test_event.clone()).unwrap();
        
        // Start manager in background
        let handle = tokio::spawn(async move {
            let mut manager_guard = manager.lock().await;
            manager_guard.start().await;
        });
        
        // Check if client received the event
        if let Some(WsMessage::SwapEvent { event }) = rx.recv().await {
            match event {
                SwapEvent::SwapInitiated { swap_id, .. } => {
                    assert_eq!(swap_id, [1u8; 32]);
                }
                _ => panic!("Unexpected event type"),
            }
        } else {
            panic!("No message received");
        }
        
        // Cleanup
        drop(event_tx);
        let _ = handle.await;
    }
} 