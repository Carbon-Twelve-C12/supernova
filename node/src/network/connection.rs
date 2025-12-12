use crate::network::peer::{PeerManager, PeerState};
use crate::network::peer_diversity::PeerDiversityManager;
use libp2p::{
    core::{ConnectedPoint, Multiaddr},
    swarm::DialError,
    PeerId,
};
use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::mpsc;
use tracing::{debug, warn};

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
    Failed(PeerId, String),
    /// Connection state changed
    StateChanged(PeerId, ConnectionState),
    /// Outbound slots available
    OutboundSlotAvailable,
    /// Inbound slots available
    InboundSlotAvailable,
}

/// Connection-related errors
#[derive(Debug)]
pub enum ConnectionError {
    /// Peer not found
    PeerNotFound(PeerId),
    /// Connection failed
    ConnectionFailed(String),
    /// Already connected to peer
    AlreadyConnected(PeerId),
    /// Connection limit reached
    ConnectionLimitReached(usize),
    /// Invalid address
    InvalidAddress(String),
    /// Dial error
    DialError(DialError),
    /// Connection timeout
    ConnectionTimeout(String),
    /// Handshake timeout
    HandshakeTimeout(PeerId),
    /// Idle timeout
    IdleTimeout(PeerId),
}

/// Connection timeout configuration
#[derive(Debug, Clone)]
pub struct ConnectionTimeoutConfig {
    /// Timeout for establishing a connection
    pub connection_timeout: Duration,
    /// Timeout for completing handshake after connection established
    pub handshake_timeout: Duration,
    /// Timeout for idle connections (no activity)
    pub idle_timeout: Duration,
    /// Timeout for pending dial attempts
    pub dial_timeout: Duration,
    /// Timeout for connection state transitions
    pub state_transition_timeout: Duration,
}

impl Default for ConnectionTimeoutConfig {
    fn default() -> Self {
        Self {
            connection_timeout: Duration::from_secs(20),
            handshake_timeout: Duration::from_secs(10),
            idle_timeout: Duration::from_secs(600), // 10 minutes
            dial_timeout: Duration::from_secs(30),
            state_transition_timeout: Duration::from_secs(5),
        }
    }
}

/// Connection metadata for timeout tracking
#[derive(Debug, Clone)]
struct ConnectionMetadata {
    /// When connection was established
    established_at: Instant,
    /// When connection became ready (handshake complete)
    ready_at: Option<Instant>,
    /// Last activity timestamp
    last_activity: Instant,
    /// When connection attempt started
    dial_started_at: Option<Instant>,
    /// When connection state was last changed
    state_changed_at: Instant,
}

/// Manager for handling peer connections
pub struct ConnectionManager {
    /// Active connections by peer ID
    connections: HashMap<PeerId, Vec<u64>>,
    /// Connection states
    connection_states: HashMap<(PeerId, u64), ConnectionState>,
    /// Connection metadata for timeout tracking
    connection_metadata: HashMap<(PeerId, u64), ConnectionMetadata>,
    /// Pending outbound connection attempts with start times
    pending_dials: HashMap<PeerId, Instant>,
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
    /// Connection timeout configuration
    timeout_config: ConnectionTimeoutConfig,
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
            connection_metadata: HashMap::new(),
            pending_dials: HashMap::new(),
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
            timeout_config: ConnectionTimeoutConfig::default(),
            feeler_addresses: HashMap::new(),
            max_feeler_connections: 2,
            next_connection_id: 0,
        }
    }

    /// Create a new connection manager with custom timeout configuration
    pub fn with_timeout_config(
        peer_manager: Arc<PeerManager>,
        diversity_manager: Arc<PeerDiversityManager>,
        max_inbound: usize,
        max_outbound: usize,
        timeout_config: ConnectionTimeoutConfig,
    ) -> Self {
        Self {
            connections: HashMap::new(),
            connection_states: HashMap::new(),
            connection_metadata: HashMap::new(),
            pending_dials: HashMap::new(),
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
            timeout_config,
            feeler_addresses: HashMap::new(),
            max_feeler_connections: 2,
            next_connection_id: 0,
        }
    }

    /// Get the timeout configuration
    pub fn timeout_config(&self) -> &ConnectionTimeoutConfig {
        &self.timeout_config
    }

    /// Set the timeout configuration
    pub fn set_timeout_config(&mut self, config: ConnectionTimeoutConfig) {
        self.timeout_config = config;
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

    /// Get a peer's address for connecting
    fn get_peer_address(&self, peer_id: &PeerId) -> Option<Multiaddr> {
        if let Some(peer_info) = self.peer_manager.get_peer(peer_id) {
            if !peer_info.addresses.is_empty() {
                return Some(peer_info.addresses[0].clone());
            }
        }
        None
    }

    /// Check for and handle connection timeouts
    pub fn check_timeouts(&mut self) -> Vec<(PeerId, u64, ConnectionError)> {
        let now = Instant::now();
        let mut timed_out = Vec::new();

        // Check pending dials
        let timed_out_dials: Vec<PeerId> = self
            .pending_dials
            .iter()
            .filter(|(_, dial_start)| now.duration_since(**dial_start) > self.timeout_config.dial_timeout)
            .map(|(peer_id, _)| *peer_id)
            .collect();

        for peer_id in timed_out_dials {
            warn!("Dial timeout for peer {}", peer_id);
            self.pending_dials.remove(&peer_id);
            timed_out.push((
                peer_id,
                0,
                ConnectionError::ConnectionTimeout(format!(
                    "Dial timeout after {}s",
                    self.timeout_config.dial_timeout.as_secs()
                )),
            ));
        }

        // Check connection handshake timeouts
        let timed_out_handshakes: Vec<(PeerId, u64)> = self
            .connection_metadata
            .iter()
            .filter(|((peer_id, conn_id), metadata)| {
                matches!(
                    self.connection_states.get(&(*peer_id, *conn_id)),
                    Some(&ConnectionState::Connected)
                ) && now.duration_since(metadata.established_at) > self.timeout_config.handshake_timeout
            })
            .map(|((peer_id, conn_id), _)| (*peer_id, *conn_id))
            .collect();

        for (peer_id, conn_id) in timed_out_handshakes {
            warn!("Handshake timeout for peer {} connection {}", peer_id, conn_id);
            timed_out.push((
                peer_id,
                conn_id,
                ConnectionError::HandshakeTimeout(peer_id),
            ));
        }

        // Check idle timeouts
        let timed_out_idle: Vec<(PeerId, u64)> = self
            .connection_metadata
            .iter()
            .filter(|((peer_id, conn_id), metadata)| {
                matches!(
                    self.connection_states.get(&(*peer_id, *conn_id)),
                    Some(&ConnectionState::Ready)
                ) && now.duration_since(metadata.last_activity) > self.timeout_config.idle_timeout
                    && !self.persistent_peers.contains(peer_id)
            })
            .map(|((peer_id, conn_id), _)| (*peer_id, *conn_id))
            .collect();

        for (peer_id, conn_id) in timed_out_idle {
            warn!("Idle timeout for peer {} connection {}", peer_id, conn_id);
            timed_out.push((
                peer_id,
                conn_id,
                ConnectionError::IdleTimeout(peer_id),
            ));
        }

        timed_out
    }

    /// Emit a connection event if sender is available
    fn emit_event(&self, event: ConnectionEvent) {
        if let Some(sender) = &self.event_sender {
            // Try to send but don't block if channel is full
            let _ = sender.try_send(event);
        }
    }

    /// Check if we have available inbound connection slots
    pub fn has_inbound_slots(&self) -> bool {
        self.inbound_count < self.max_inbound_connections
    }

    /// Check if we have available outbound connection slots
    pub fn has_outbound_slots(&self) -> bool {
        self.outbound_count + self.pending_dials.len() < self.max_outbound_connections
    }

    /// Queue a peer for connection when slots are available
    pub fn queue_connection(&mut self, peer_id: PeerId, addr: Multiaddr) {
        // Don't queue banned peers
        if self.peer_manager.is_peer_banned(&peer_id) {
            return;
        }

        // Don't queue already connected or pending peers
        if self.is_connected(&peer_id) || self.pending_dials.contains_key(&peer_id) {
            return;
        }

        // Add to the queue
        if !self.connection_queue.iter().any(|(p, _)| p == &peer_id) {
            self.connection_queue.push_back((peer_id, addr));
        }
    }

    /// Start a connection attempt (track dial start time)
    pub fn start_dial(&mut self, peer_id: PeerId) {
        self.pending_dials.insert(peer_id, Instant::now());
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

        let now = Instant::now();

        // Check if this is a pending dial that succeeded
        if !is_inbound {
            if let Some(dial_start) = self.pending_dials.remove(peer_id) {
                // Check if dial took too long
                if now.duration_since(dial_start) > self.timeout_config.dial_timeout {
                    warn!(
                        "Dial to peer {} took {}s (timeout: {}s)",
                        peer_id,
                        now.duration_since(dial_start).as_secs(),
                        self.timeout_config.dial_timeout.as_secs()
                    );
                }
            }
        }

        // Update connection tracking
        self.connections
            .entry(*peer_id)
            .or_default()
            .push(connection_id);

        // Set connection state
        self.connection_states
            .insert((*peer_id, connection_id), ConnectionState::Connected);

        // Create connection metadata
        self.connection_metadata.insert(
            (*peer_id, connection_id),
            ConnectionMetadata {
                established_at: now,
                ready_at: None,
                last_activity: now,
                dial_started_at: if is_inbound { None } else { Some(now) },
                state_changed_at: now,
            },
        );

        // Update count
        if is_inbound {
            self.inbound_count += 1;
        } else {
            self.outbound_count += 1;
        }

        // Store endpoint
        self.peer_endpoints.insert(*peer_id, endpoint.clone());

        // Update peer state
        self.peer_manager
            .update_peer_state(peer_id, PeerState::Connected);

        // Emit connection event
        self.emit_event(ConnectionEvent::Connected(*peer_id, endpoint));
        self.emit_event(ConnectionEvent::StateChanged(
            *peer_id,
            ConnectionState::Connected,
        ));

        debug!(
            "Connection established with peer {}: inbound={}",
            peer_id, is_inbound
        );
    }

    /// Handle connection close
    pub fn handle_connection_closed(&mut self, peer_id: &PeerId, connection_id: u64) {
        // Get endpoint before removing connection
        let endpoint = self.peer_endpoints.remove(peer_id);
        let is_inbound = if let Some(ConnectedPoint::Listener { .. }) = endpoint {
            true
        } else {
            false
        };

        // Remove connection metadata
        self.connection_metadata.remove(&(*peer_id, connection_id));

        // Remove from connections
        if let Some(connections) = self.connections.get_mut(peer_id) {
            connections.retain(|&c| c != connection_id);

            // If no more connections, remove entirely
            if connections.is_empty() {
                self.connections.remove(peer_id);

                // Update peer state
                self.peer_manager
                    .update_peer_state(peer_id, PeerState::Disconnected);

                // Emit disconnection event
                self.emit_event(ConnectionEvent::Disconnected(*peer_id));

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
                        self.queue_connection(*peer_id, addr);
                    }
                }
            }
        }

        // Remove connection state
        self.connection_states.remove(&(*peer_id, connection_id));
    }

    /// Handle a failed connection attempt
    pub fn handle_dial_failure(&mut self, peer_id: &PeerId, error: &DialError) {
        // Remove from pending dials
        self.pending_dials.remove(peer_id);

        // Record the failure
        self.peer_manager.record_failed_attempt(peer_id);

        // Emit failure event
        self.emit_event(ConnectionEvent::Failed(*peer_id, error.to_string()));

        // Notify about available outbound slot
        self.emit_event(ConnectionEvent::OutboundSlotAvailable);

        debug!(
            "Connection attempt failed for peer {}: {:?}",
            peer_id, error
        );
    }

    /// Update connection activity timestamp
    pub fn update_connection_activity(&mut self, peer_id: &PeerId, connection_id: u64) {
        if let Some(metadata) = self.connection_metadata.get_mut(&(*peer_id, connection_id)) {
            metadata.last_activity = Instant::now();
        }
    }

    /// Update connection state
    pub fn update_connection_state(
        &mut self,
        peer_id: &PeerId,
        connection_id: u64,
        state: ConnectionState,
    ) {
        let now = Instant::now();

        // Update connection state
        self.connection_states
            .insert((*peer_id, connection_id), state);

        // Update metadata
        if let Some(metadata) = self.connection_metadata.get_mut(&(*peer_id, connection_id)) {
            metadata.state_changed_at = now;
            if state == ConnectionState::Ready && metadata.ready_at.is_none() {
                metadata.ready_at = Some(now);
                // Check handshake timeout
                let handshake_duration = now.duration_since(metadata.established_at);
                if handshake_duration > self.timeout_config.handshake_timeout {
                    warn!(
                        "Handshake for peer {} took {}s (timeout: {}s)",
                        peer_id,
                        handshake_duration.as_secs(),
                        self.timeout_config.handshake_timeout.as_secs()
                    );
                }
            }
        }

        // If all connections to this peer are ready, update peer state
        if state == ConnectionState::Ready {
            let all_ready = if let Some(connections) = self.connections.get(peer_id) {
                connections.iter().all(|conn_id| {
                    self.connection_states
                        .get(&(*peer_id, *conn_id))
                        .map_or(false, |&s| s == ConnectionState::Ready)
                })
            } else {
                false
            };

            if all_ready {
                // Update peer state
                self.peer_manager
                    .update_peer_state(peer_id, PeerState::Ready);
            }
        }

        // Emit state change event
        self.emit_event(ConnectionEvent::StateChanged(*peer_id, state));
    }

    /// Process the connection queue
    pub fn process_connection_queue<F>(&mut self, mut dial_peer: F)
    where
        F: FnMut(PeerId, Multiaddr),
    {
        // Check for timed-out dial attempts
        let now = Instant::now();
        let mut timed_out_dials = Vec::new();
        for (peer_id, dial_start) in &self.pending_dials {
            if now.duration_since(*dial_start) > self.timeout_config.dial_timeout {
                timed_out_dials.push(*peer_id);
            }
        }

        // Remove timed-out dials
        for peer_id in &timed_out_dials {
            warn!("Dial timeout for peer {}", peer_id);
            self.pending_dials.remove(peer_id);
            self.peer_manager.record_failed_attempt(peer_id);
            self.emit_event(ConnectionEvent::Failed(
                *peer_id,
                format!("Dial timeout after {}s", self.timeout_config.dial_timeout.as_secs()),
            ));
            self.emit_event(ConnectionEvent::OutboundSlotAvailable);
        }

        self.process_connection_queue_internal(dial_peer, false);
    }

    /// Process connection queue with optional feeler connections
    fn process_connection_queue_internal<F>(&mut self, mut dial_peer: F, include_feelers: bool)
    where
        F: FnMut(PeerId, Multiaddr),
    {
        // Check for available slots
        if !self.has_outbound_slots() {
            return;
        }

        // First, try to connect to persistent peers
        let persistent_peer_ids: Vec<PeerId> = self.persistent_peers.iter().cloned().collect();
        for peer_id in persistent_peer_ids {
            if self.is_connected(&peer_id) || self.pending_dials.contains_key(&peer_id) {
                continue;
            }

            if let Some(addr) = self.get_peer_address(&peer_id) {
                debug!("Dialing persistent peer {}", peer_id);
                dial_peer(peer_id, addr);
                self.start_dial(peer_id);

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
                if self.is_connected(&peer_id) || self.pending_dials.contains_key(&peer_id) {
                    continue;
                }

                // Skip if banned
                if self.peer_manager.is_peer_banned(&peer_id) {
                    continue;
                }

                debug!("Dialing queued peer {}", peer_id);
                dial_peer(peer_id, addr);
                self.start_dial(peer_id);

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
        let candidate_peers = self
            .peer_manager
            .get_peers_by_state(PeerState::Disconnected);

        for peer in candidate_peers {
            // Skip if already connected, dialing, or recently tried
            if self.is_connected(&peer.peer_id)
                || self.pending_dials.contains_key(&peer.peer_id)
                || self.feeler_addresses.contains_key(&peer.peer_id)
            {
                continue;
            }

            // Skip if no addresses
            if peer.addresses.is_empty() {
                continue;
            }

            // Select a random address
            let addr = peer.addresses[0].clone();

            debug!("Dialing feeler connection to {}", peer.peer_id);
            dial_peer(peer.peer_id, addr);
            self.start_dial(peer.peer_id);
            self.feeler_addresses.insert(peer.peer_id, now);

            // Only try a limited number per cycle
            if self.feeler_addresses.len() >= self.max_feeler_connections
                || !self.has_outbound_slots()
            {
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
                if let Some(&state) = self.connection_states.get(&(*peer_id, conn_id)) {
                    if state == ConnectionState::Ready {
                        return Some(ConnectionState::Ready);
                    } else if state == ConnectionState::Connected
                        && best_state != ConnectionState::Ready
                    {
                        best_state = ConnectionState::Connected;
                    }
                }
            }

            Some(best_state)
        } else {
            None
        }
    }

    /// Perform periodic maintenance
    pub fn perform_maintenance<F>(&mut self, mut dial_peer: F)
    where
        F: FnMut(PeerId, Multiaddr),
    {
        let now = Instant::now();

        // Only run every 30 seconds
        if now.duration_since(self.last_cleanup) < Duration::from_secs(30) {
            return;
        }

        self.last_cleanup = now;

        // Check for timeouts
        let timed_out = self.check_timeouts();
        for (peer_id, conn_id, error) in timed_out {
            match error {
                ConnectionError::HandshakeTimeout(_) | ConnectionError::IdleTimeout(_) => {
                    // Close the connection
                    self.handle_connection_closed(&peer_id, conn_id);
                    self.emit_event(ConnectionEvent::Failed(peer_id, format!("{:?}", error)));
                }
                ConnectionError::ConnectionTimeout(_) => {
                    // Already handled in check_timeouts
                    self.peer_manager.record_failed_attempt(&peer_id);
                    self.emit_event(ConnectionEvent::Failed(peer_id, format!("{:?}", error)));
                    self.emit_event(ConnectionEvent::OutboundSlotAvailable);
                }
                _ => {}
            }
        }

        // Process connection queue (including feelers)
        self.process_connection_queue_internal(dial_peer, true);
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
            10,
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
        assert_eq!(
            manager.get_connection_state(&peer_id),
            Some(ConnectionState::Ready)
        );

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
        let first_outbound: Option<(PeerId, u64)> = manager.connections.iter().find_map(|(peer_id, conns)| {
            let conn_id = *conns.first()?;
            let key = (peer_id.clone(), conn_id);
            let is_dialer = manager.connection_states.get(&key).is_some()
                && matches!(manager.peer_endpoints.get(peer_id), Some(ConnectedPoint::Dialer { .. }));
            if is_dialer {
                Some((peer_id.clone(), conn_id))
            } else {
                None
            }
        });

        if let Some((peer_id, conn_id)) = first_outbound {
            manager.handle_connection_closed(&peer_id, conn_id);
        }

        let first_inbound: Option<(PeerId, u64)> = manager.connections.iter().find_map(|(peer_id, conns)| {
            let conn_id = *conns.first()?;
            let key = (peer_id.clone(), conn_id);
            let is_listener = manager.connection_states.get(&key).is_some()
                && matches!(manager.peer_endpoints.get(peer_id), Some(ConnectedPoint::Listener { .. }));
            if is_listener {
                Some((peer_id.clone(), conn_id))
            } else {
                None
            }
        });

        if let Some((peer_id, conn_id)) = first_inbound {
            manager.handle_connection_closed(&peer_id, conn_id);
        }

        // Should have slots again
        assert!(manager.has_outbound_slots());
        assert!(manager.has_inbound_slots());
    }
}
