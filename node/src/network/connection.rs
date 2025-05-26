use libp2p::{
    core::{
        muxing::StreamMuxerBox,
        transport::Boxed,
        upgrade::{self, InboundUpgrade, OutboundUpgrade, UpgradeInfo, Negotiated},
        ConnectedPoint, PeerId, Multiaddr,
    },
    swarm::{DialError, NetworkBehaviour, ProtocolsHandler, KeepAlive, SubstreamProtocol},
    identity::Keypair,
};
use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use serde::{Serialize, Deserialize};
use tracing::{debug, info, warn, error};
use tokio::sync::mpsc;
use crate::network::peer::{PeerState, PeerManager};
use crate::network::peer_diversity::PeerDiversityManager;
use void::Void;

/// State of a connection
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionState {
    /// Connection is being established
    Connecting,
    /// Connection established but handshake not complete
    Connected,
    /// Connection established and handshake complete
    Ready,
    /// Connection is being closed
    Closing,
    /// Connection closed
    Closed,
}

/// Connection events emitted by the connection manager
#[derive(Debug, Clone)]
pub enum ConnectionEvent {
    /// New connection established
    Connected(PeerId, ConnectedPoint),
    /// Connection closed
    Disconnected(PeerId),
    /// Connection attempt failed
    Failed(PeerId, DialError),
    /// Connection state changed
    StateChanged(PeerId, ConnectionState),
    /// Outbound slots available
    OutboundSlotAvailable,
    /// Inbound slots available
    InboundSlotAvailable,
}

/// Manager for handling peer connections
pub struct ConnectionManager {
    /// Active connections by peer ID
    connections: HashMap<PeerId, Vec<u64>>,
    /// Connection states
    connection_states: HashMap<(PeerId, u64), ConnectionState>,
    /// Pending outbound connection attempts
    pending_dials: HashSet<PeerId>,
    /// Queue of peers to connect to when slots are available
    connection_queue: VecDeque<(PeerId, Multiaddr)>,
    /// Set of peers we want to keep connected
    persistent_peers: HashSet<PeerId>,
    /// Maximum number of inbound connections
    max_inbound_connections: usize,
    /// Maximum number of outbound connections
    max_outbound_connections: usize,
    /// Map of peers to their connection point
    peer_endpoints: HashMap<PeerId, ConnectedPoint>,
    /// Handle to the peer manager
    peer_manager: Arc<PeerManager>,
    /// Peer diversity manager for connection selection
    diversity_manager: Arc<PeerDiversityManager>,
    /// Event sender channel
    event_sender: Option<mpsc::Sender<ConnectionEvent>>,
    /// Count of inbound connections
    inbound_count: usize,
    /// Count of outbound connections
    outbound_count: usize,
    /// Last connection and cleanup time
    last_cleanup: Instant,
    /// Connection idle timeout
    idle_timeout: Duration,
    /// Feeler connections for discovery
    feeler_addresses: HashMap<PeerId, Instant>,
    /// Maximum feeler connections per cycle
    max_feeler_connections: usize,
    /// Next connection ID
    next_connection_id: u64,
}

impl ConnectionManager {
    /// Create a new connection manager
    pub fn new(
        peer_manager: Arc<PeerManager>,
        diversity_manager: Arc<PeerDiversityManager>,
        max_inbound: usize,
        max_outbound: usize,
    ) -> Self {
        Self {
            connections: HashMap::new(),
            connection_states: HashMap::new(),
            pending_dials: HashSet::new(),
            connection_queue: VecDeque::new(),
            persistent_peers: HashSet::new(),
            max_inbound_connections: max_inbound,
            max_outbound_connections: max_outbound,
            peer_endpoints: HashMap::new(),
            peer_manager,
            diversity_manager,
            event_sender: None,
            inbound_count: 0,
            outbound_count: 0,
            last_cleanup: Instant::now(),
            idle_timeout: Duration::from_secs(60 * 10), // 10 minutes
            feeler_addresses: HashMap::new(),
            max_feeler_connections: 2,
            next_connection_id: 0,
        }
    }
    
    /// Set the event sender channel
    pub fn set_event_sender(&mut self, sender: mpsc::Sender<ConnectionEvent>) {
        self.event_sender = Some(sender);
    }
    
    /// Add a persistent peer we want to keep connected
    pub fn add_persistent_peer(&mut self, peer_id: PeerId) {
        self.persistent_peers.insert(peer_id);
    }
    
    /// Remove a persistent peer
    pub fn remove_persistent_peer(&mut self, peer_id: &PeerId) {
        self.persistent_peers.remove(peer_id);
    }
    
    /// Check if we have available outbound connection slots
    pub fn has_outbound_slots(&self) -> bool {
        self.outbound_count + self.pending_dials.len() < self.max_outbound_connections
    }
    
    /// Check if we have available inbound connection slots
    pub fn has_inbound_slots(&self) -> bool {
        self.inbound_count < self.max_inbound_connections
    }
    
    /// Queue a peer for connection when slots are available
    pub fn queue_connection(&mut self, peer_id: PeerId, addr: Multiaddr) {
        // Don't queue banned peers
        if self.peer_manager.is_peer_banned(&peer_id) {
            return;
        }
        
        // Don't queue already connected or pending peers
        if self.is_connected(&peer_id) || self.pending_dials.contains(&peer_id) {
            return;
        }
        
        // Add to the queue
        if !self.connection_queue.iter().any(|(p, _)| p == &peer_id) {
            self.connection_queue.push_back((peer_id, addr));
        }
    }
    
    /// Handle new connection from a peer
    pub fn handle_connection_established(
        &mut self,
        peer_id: &PeerId, 
        connection_id: u64,
        endpoint: ConnectedPoint,
    ) {
        let is_inbound = match &endpoint {
            ConnectedPoint::Dialer { .. } => false,
            ConnectedPoint::Listener { .. } => true,
        };
        
        // Check if this is a pending dial that succeeded
        if !is_inbound {
            self.pending_dials.remove(peer_id);
        }
        
        // Update connection tracking
        self.connections
            .entry(peer_id.clone())
            .or_insert_with(Vec::new)
            .push(connection_id);
        
        // Set connection state
        self.connection_states.insert((peer_id.clone(), connection_id), ConnectionState::Connected);
        
        // Update count
        if is_inbound {
            self.inbound_count += 1;
        } else {
            self.outbound_count += 1;
        }
        
        // Store endpoint
        self.peer_endpoints.insert(peer_id.clone(), endpoint.clone());
        
        // Update peer state
        self.peer_manager.update_peer_state(peer_id, PeerState::Connected);
        
        // Emit connection event
        self.emit_event(ConnectionEvent::Connected(peer_id.clone(), endpoint));
        self.emit_event(ConnectionEvent::StateChanged(peer_id.clone(), ConnectionState::Connected));
        
        debug!("Connection established with peer {}: inbound={}", peer_id, is_inbound);
    }
    
    /// Handle connection close
    pub fn handle_connection_closed(
        &mut self,
        peer_id: &PeerId, 
        connection_id: u64,
    ) {
        // Get endpoint before removing connection
        let endpoint = self.peer_endpoints.remove(peer_id);
        let is_inbound = if let Some(ConnectedPoint::Listener { .. }) = endpoint {
            true
        } else {
            false
        };
        
        // Remove from connections
        if let Some(connections) = self.connections.get_mut(peer_id) {
            connections.retain(|&c| c != connection_id);
            
            // If no more connections, remove entirely
            if connections.is_empty() {
                self.connections.remove(peer_id);
                
                // Update peer state
                self.peer_manager.update_peer_state(peer_id, PeerState::Disconnected);
                
                // Emit disconnection event
                self.emit_event(ConnectionEvent::Disconnected(peer_id.clone()));
                
                // Update count
                if is_inbound {
                    self.inbound_count = self.inbound_count.saturating_sub(1);
                    // Notify about available inbound slot
                    self.emit_event(ConnectionEvent::InboundSlotAvailable);
                } else {
                    self.outbound_count = self.outbound_count.saturating_sub(1);
                    // Notify about available outbound slot
                    self.emit_event(ConnectionEvent::OutboundSlotAvailable);
                }
                
                debug!("All connections closed with peer {}", peer_id);
                
                // If this was a persistent peer, re-queue it for connection
                if self.persistent_peers.contains(peer_id) {
                    if let Some(addr) = self.get_peer_address(peer_id) {
                        debug!("Re-queueing persistent peer {} for connection", peer_id);
                        self.queue_connection(peer_id.clone(), addr);
                    }
                }
            }
        }
        
        // Remove connection state
        self.connection_states.remove(&(peer_id.clone(), connection_id));
    }
    
    /// Handle a failed connection attempt
    pub fn handle_dial_failure(&mut self, peer_id: &PeerId, error: &DialError) {
        // Remove from pending dials
        self.pending_dials.remove(peer_id);
        
        // Record the failure
        self.peer_manager.record_failed_attempt(peer_id);
        
        // Emit failure event
        self.emit_event(ConnectionEvent::Failed(peer_id.clone(), error.clone()));
        
        // Notify about available outbound slot
        self.emit_event(ConnectionEvent::OutboundSlotAvailable);
        
        debug!("Connection attempt failed for peer {}: {:?}", peer_id, error);
    }
    
    /// Update connection state
    pub fn update_connection_state(
        &mut self,
        peer_id: &PeerId, 
        connection_id: u64,
        state: ConnectionState,
    ) {
        // Update connection state
        self.connection_states.insert((peer_id.clone(), connection_id), state);
        
        // If all connections to this peer are ready, update peer state
        if state == ConnectionState::Ready {
            let all_ready = if let Some(connections) = self.connections.get(peer_id) {
                connections.iter().all(|conn_id| {
                    self.connection_states
                        .get(&(peer_id.clone(), *conn_id))
                        .map_or(false, |&s| s == ConnectionState::Ready)
                })
            } else {
                false
            };
            
            if all_ready {
                // Update peer state
                self.peer_manager.update_peer_state(peer_id, PeerState::Ready);
            }
        }
        
        // Emit state change event
        self.emit_event(ConnectionEvent::StateChanged(peer_id.clone(), state));
    }
    
    /// Process the connection queue
    pub fn process_connection_queue<F>(&mut self, dial_peer: F)
    where
        F: FnMut(PeerId, Multiaddr),
    {
        self.process_connection_queue_internal(dial_peer, false);
    }
    
    /// Process connection queue with optional feeler connections
    fn process_connection_queue_internal<F>(
        &mut self, 
        mut dial_peer: F, 
        include_feelers: bool,
    )
    where
        F: FnMut(PeerId, Multiaddr),
    {
        // Check for available slots
        if !self.has_outbound_slots() {
            return;
        }
        
        // First, try to connect to persistent peers
        for peer_id in &self.persistent_peers {
            if self.is_connected(peer_id) || self.pending_dials.contains(peer_id) {
                continue;
            }
            
            if let Some(addr) = self.get_peer_address(peer_id) {
                debug!("Dialing persistent peer {}", peer_id);
                dial_peer(peer_id.clone(), addr);
                self.pending_dials.insert(peer_id.clone());
                
                // Break if no more slots
                if !self.has_outbound_slots() {
                    return;
                }
            }
        }
        
        // Then process regular connection queue
        while self.has_outbound_slots() && !self.connection_queue.is_empty() {
            if let Some((peer_id, addr)) = self.connection_queue.pop_front() {
                // Skip if already connected or dialing
                if self.is_connected(&peer_id) || self.pending_dials.contains(&peer_id) {
                    continue;
                }
                
                // Skip if banned
                if self.peer_manager.is_peer_banned(&peer_id) {
                    continue;
                }
                
                debug!("Dialing queued peer {}", peer_id);
                dial_peer(peer_id.clone(), addr);
                self.pending_dials.insert(peer_id);
                
                // Break if no more slots
                if !self.has_outbound_slots() {
                    break;
                }
            }
        }
        
        // Finally, try feeler connections if enabled
        if include_feelers && self.has_outbound_slots() {
            self.process_feeler_connections(dial_peer);
        }
    }
    
    /// Process feeler connections for discovery
    fn process_feeler_connections<F>(&mut self, mut dial_peer: F)
    where
        F: FnMut(PeerId, Multiaddr),
    {
        // Clean up old feeler attempts
        let now = Instant::now();
        self.feeler_addresses
            .retain(|_, timestamp| now.duration_since(*timestamp) < Duration::from_secs(600));
        
        // Don't make too many feeler connections
        if self.feeler_addresses.len() >= self.max_feeler_connections {
            return;
        }
        
        // Request addresses from the peer manager
        let candidate_peers = self.peer_manager.get_peers_by_state(PeerState::Disconnected);
        
        for peer in candidate_peers {
            // Skip if already connected, dialing, or recently tried
            if self.is_connected(&peer.peer_id) || 
               self.pending_dials.contains(&peer.peer_id) ||
               self.feeler_addresses.contains_key(&peer.peer_id) {
                continue;
            }
            
            // Skip if no addresses
            if peer.addresses.is_empty() {
                continue;
            }
            
            // Select a random address
            let addr = peer.addresses[0].clone();
            
            debug!("Dialing feeler connection to {}", peer.peer_id);
            dial_peer(peer.peer_id.clone(), addr);
            self.pending_dials.insert(peer.peer_id.clone());
            self.feeler_addresses.insert(peer.peer_id, now);
            
            // Only try a limited number per cycle
            if self.feeler_addresses.len() >= self.max_feeler_connections || 
               !self.has_outbound_slots() {
                break;
            }
        }
    }
    
    /// Check if a peer is connected
    pub fn is_connected(&self, peer_id: &PeerId) -> bool {
        self.connections.contains_key(peer_id) && !self.connections[peer_id].is_empty()
    }
    
    /// Get the list of currently connected peers
    pub fn connected_peers(&self) -> Vec<PeerId> {
        self.connections
            .keys()
            .filter(|p| !self.connections[p].is_empty())
            .cloned()
            .collect()
    }
    
    /// Get the connection state for a peer
    pub fn get_connection_state(&self, peer_id: &PeerId) -> Option<ConnectionState> {
        if let Some(connections) = self.connections.get(peer_id) {
            if connections.is_empty() {
                return None;
            }
            
            // Return the most "advanced" state of any connection
            let mut best_state = ConnectionState::Connecting;
            
            for &conn_id in connections {
                if let Some(&state) = self.connection_states.get(&(peer_id.clone(), conn_id)) {
                    if state == ConnectionState::Ready {
                        return Some(ConnectionState::Ready);
                    } else if state == ConnectionState::Connected && best_state != ConnectionState::Ready {
                        best_state = ConnectionState::Connected;
                    }
                }
            }
            
            Some(best_state)
        } else {
            None
        }
    }
    
    /// Get a peer's address for connecting
    fn get_peer_address(&self, peer_id: &PeerId) -> Option<Multiaddr> {
        if let Some(peer_info) = self.peer_manager.get_peer(peer_id) {
            if !peer_info.addresses.is_empty() {
                return Some(peer_info.addresses[0].clone());
            }
        }
        None
    }
    
    /// Perform periodic maintenance
    pub fn perform_maintenance<F>(&mut self, dial_peer: F)
    where
        F: FnMut(PeerId, Multiaddr),
    {
        let now = Instant::now();
        
        // Only run every 30 seconds
        if now.duration_since(self.last_cleanup) < Duration::from_secs(30) {
            return;
        }
        
        self.last_cleanup = now;
        
        // Process connection queue (including feelers)
        self.process_connection_queue_internal(dial_peer, true);
    }
    
    /// Emit a connection event if sender is available
    fn emit_event(&self, event: ConnectionEvent) {
        if let Some(sender) = &self.event_sender {
            // Try to send but don't block if channel is full
            let _ = sender.try_send(event);
        }
    }
    
    /// Get connection counts
    pub fn connection_counts(&self) -> (usize, usize) {
        (self.inbound_count, self.outbound_count)
    }
    
    /// Get the total number of connections
    pub fn total_connections(&self) -> usize {
        self.inbound_count + self.outbound_count
    }
    
    /// Get the connection queue length
    pub fn connection_queue_length(&self) -> usize {
        self.connection_queue.len()
    }
}

// Simple dummy handler for libp2p 0.41 compatibility
#[derive(Debug)]
pub struct DummyHandler;

impl ProtocolsHandler for DummyHandler {
    type InEvent = Void;
    type OutEvent = Void;
    type Error = Void;
    type InboundProtocol = libp2p::core::upgrade::DeniedUpgrade;
    type OutboundProtocol = libp2p::core::upgrade::DeniedUpgrade;
    type OutboundOpenInfo = Void;
    type InboundOpenInfo = ();

    fn listen_protocol(&self) -> SubstreamProtocol<Self::InboundProtocol, Self::InboundOpenInfo> {
        SubstreamProtocol::new(libp2p::core::upgrade::DeniedUpgrade, ())
    }

    fn inject_fully_negotiated_inbound(
        &mut self,
        _: <Self::InboundProtocol as libp2p::core::InboundUpgrade<Negotiated<StreamMuxerBox>>>::Output,
        _: Self::InboundOpenInfo,
    ) {
        // Handle fully negotiated inbound connection
    }

    fn inject_fully_negotiated_outbound(
        &mut self,
        _: <Self::OutboundProtocol as libp2p::core::OutboundUpgrade<Negotiated<StreamMuxerBox>>>::Output,
        _: Self::OutboundOpenInfo,
    ) {
        // Handle fully negotiated outbound connection
    }

    fn inject_event(&mut self, _: Self::InEvent) {}

    fn inject_address_change(&mut self, _: &Multiaddr) {}

    fn inject_dial_upgrade_error(&mut self, _: Self::OutboundOpenInfo, _: libp2p::swarm::ProtocolsHandlerUpgrErr<Self::Error>) {}

    fn inject_listen_upgrade_error(&mut self, _: Self::InboundOpenInfo, _: libp2p::swarm::ProtocolsHandlerUpgrErr<Self::Error>) {}

    fn connection_keep_alive(&self) -> KeepAlive {
        KeepAlive::No
    }

    fn poll(
        &mut self,
        _: &mut std::task::Context<'_>,
    ) -> std::task::Poll<libp2p::swarm::ProtocolsHandlerEvent<Self::OutboundProtocol, Self::OutboundOpenInfo, Self::OutEvent, Self::Error>> {
        std::task::Poll::Pending
    }
}

// Implementation for libp2p NetworkBehaviour trait
impl NetworkBehaviour for ConnectionManager {
    type ProtocolsHandler = DummyHandler;
    type OutEvent = ConnectionEvent;

    fn new_handler(&mut self) -> Self::ProtocolsHandler {
        DummyHandler
    }

    fn inject_event(
        &mut self,
        _peer_id: PeerId,
        _connection: u64, // Using u64 instead of ConnectionId
        _event: <Self::ProtocolsHandler as ProtocolsHandler>::OutEvent,
    ) {
        // No events from DummyHandler
    }

    fn poll(
        &mut self,
        _cx: &mut std::task::Context<'_>,
        _params: &mut impl libp2p::swarm::PollParameters,
    ) -> std::task::Poll<libp2p::swarm::NetworkBehaviourAction<Self::OutEvent, Self::ProtocolsHandler, <Self::ProtocolsHandler as ProtocolsHandler>::InEvent>> {
        std::task::Poll::Pending
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::network::peer_diversity::ConnectionStrategy;
    use std::sync::Arc;
    
    // Helper to create a test manager
    fn create_test_manager() -> ConnectionManager {
        let peer_manager = Arc::new(PeerManager::new());
        let diversity_manager = Arc::new(PeerDiversityManager::with_config(
            0.5, 
            ConnectionStrategy::BalancedDiversity,
            10
        ));
        
        ConnectionManager::new(
            peer_manager,
            diversity_manager,
            5, // max inbound
            3, // max outbound
        )
    }
    
    #[test]
    fn test_connection_tracking() {
        let mut manager = create_test_manager();
        let peer_id = PeerId::random();
        let connection_id = 1;
        let endpoint = ConnectedPoint::Dialer {
            address: "/ip4/127.0.0.1/tcp/8000".parse().unwrap(),
            role_override: libp2p::core::Endpoint::Dialer,
        };
        
        // Test connection establishment
        manager.handle_connection_established(&peer_id, connection_id, endpoint);
        assert!(manager.is_connected(&peer_id));
        assert_eq!(manager.outbound_count, 1);
        assert_eq!(manager.inbound_count, 0);
        
        // Test connection state update
        manager.update_connection_state(&peer_id, connection_id, ConnectionState::Ready);
        assert_eq!(manager.get_connection_state(&peer_id), Some(ConnectionState::Ready));
        
        // Test connection close
        manager.handle_connection_closed(&peer_id, connection_id);
        assert!(!manager.is_connected(&peer_id));
        assert_eq!(manager.outbound_count, 0);
        assert_eq!(manager.inbound_count, 0);
    }
    
    #[test]
    fn test_connection_slots() {
        let mut manager = create_test_manager();
        
        // Should have slots initially
        assert!(manager.has_outbound_slots());
        assert!(manager.has_inbound_slots());
        
        // Fill outbound slots
        for i in 0..3 {
            let peer_id = PeerId::random();
            let connection_id = i as u64;
            let endpoint = ConnectedPoint::Dialer {
                address: "/ip4/127.0.0.1/tcp/8000".parse().unwrap(),
                role_override: libp2p::core::Endpoint::Dialer,
            };
            
            manager.handle_connection_established(&peer_id, connection_id, endpoint);
        }
        
        // Should have no outbound slots now
        assert!(!manager.has_outbound_slots());
        assert!(manager.has_inbound_slots());
        
        // Fill inbound slots
        for i in 0..5 {
            let peer_id = PeerId::random();
            let connection_id = (i + 10) as u64;
            let endpoint = ConnectedPoint::Listener {
                local_addr: "/ip4/127.0.0.1/tcp/8000".parse().unwrap(),
                send_back_addr: "/ip4/192.168.1.1/tcp/9000".parse().unwrap(),
            };
            
            manager.handle_connection_established(&peer_id, connection_id, endpoint);
        }
        
        // Should have no slots now
        assert!(!manager.has_outbound_slots());
        assert!(!manager.has_inbound_slots());
        
        // Close one connection of each type
        let first_outbound = manager.connections.iter().find(|(_, conns)| {
            if let Some(conn_id) = conns.first() {
                let key = ((*conn_id).clone(), (*conn_id));
                manager.connection_states.get(&key).map_or(false, |&state| {
                    if let Some(endpoint) = manager.peer_endpoints.get(conn_id) {
                        match endpoint {
                            ConnectedPoint::Dialer { .. } => true,
                            _ => false,
                        }
                    } else {
                        false
                    }
                })
            } else {
                false
            }
        });
        
        if let Some((peer_id, conns)) = first_outbound {
            let conn_id = conns[0];
            manager.handle_connection_closed(&peer_id, conn_id);
        }
        
        let first_inbound = manager.connections.iter().find(|(_, conns)| {
            if let Some(conn_id) = conns.first() {
                let key = ((*conn_id).clone(), (*conn_id));
                manager.connection_states.get(&key).map_or(false, |&state| {
                    if let Some(endpoint) = manager.peer_endpoints.get(conn_id) {
                        match endpoint {
                            ConnectedPoint::Listener { .. } => true,
                            _ => false,
                        }
                    } else {
                        false
                    }
                })
            } else {
                false
            }
        });
        
        if let Some((peer_id, conns)) = first_inbound {
            let conn_id = conns[0];
            manager.handle_connection_closed(&peer_id, conn_id);
        }
        
        // Should have slots again
        assert!(manager.has_outbound_slots());
        assert!(manager.has_inbound_slots());
    }
} 