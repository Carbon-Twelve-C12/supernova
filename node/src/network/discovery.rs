use libp2p::{
    core::{
        muxing::StreamMuxerBox,
        upgrade::{self, InboundUpgrade, OutboundUpgrade, UpgradeInfo, Negotiated, DeniedUpgrade},
        ConnectedPoint, PeerId, Multiaddr,
    },
    kad::{self, Kademlia, KademliaConfig, KademliaEvent, QueryId, QueryResult, Record, BootstrapError, store::MemoryStore},
    swarm::{NetworkBehaviour, ProtocolsHandler, KeepAlive, SubstreamProtocol, DialError},
    mdns::{Mdns, MdnsEvent, MdnsConfig},
    identity::Keypair,
};
use std::{
    collections::{HashMap, HashSet},
    error::Error,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};
use tokio::sync::mpsc;
use tracing::{debug, info, warn, error};
use serde::{Serialize, Deserialize};
use void::Void;

/// Events emitted by the discovery system
#[derive(Debug, Clone)]
pub enum DiscoveryEvent {
    /// New peer discovered
    PeerDiscovered(PeerId, Vec<Multiaddr>),
    /// Peer lost or expired
    PeerExpired(PeerId),
    /// Peer verified and validated
    PeerVerified(PeerId),
    /// Bootstrap complete
    BootstrapComplete,
    /// Error occurred during discovery
    Error(String),
}

/// Peer discovery using Kademlia DHT and mDNS
pub struct PeerDiscovery {
    // Kademlia DHT for peer discovery over WAN
    kademlia: Option<Kademlia<Record>>,
    // mDNS for local network discovery
    mdns: Option<Mdns>,
    // Map of ongoing queries
    active_queries: HashMap<QueryId, QueryType>,
    // Known peers with their discovered addresses
    known_peers: Arc<Mutex<HashMap<PeerId, Vec<Multiaddr>>>>,
    // Verified peers that passed connection test
    verified_peers: HashSet<PeerId>,
    // Bootstrap nodes
    bootstrap_nodes: Vec<(PeerId, Multiaddr)>,
    // Is bootstrap complete
    bootstrap_complete: bool,
    // Event sender channel
    event_sender: mpsc::Sender<DiscoveryEvent>,
    // Last bootstrap attempt timestamp
    last_bootstrap: Option<Instant>,
    // Bootstrap interval
    bootstrap_interval: Duration,
    // Local peer ID
    local_peer_id: PeerId,
}

#[derive(Debug, Clone)]
enum QueryType {
    Bootstrap,
    FindPeer(PeerId),
    GetProviders(String),
}

// Simple dummy handler for libp2p 0.41 compatibility
#[derive(Debug)]
pub struct DummyHandler;

impl ProtocolsHandler for DummyHandler {
    type InEvent = Void;
    type OutEvent = Void;
    type Error = Void;
    type InboundProtocol = DeniedUpgrade;
    type OutboundProtocol = DeniedUpgrade;
    type OutboundOpenInfo = Void;
    type InboundOpenInfo = ();

    fn listen_protocol(&self) -> SubstreamProtocol<Self::InboundProtocol, Self::InboundOpenInfo> {
        SubstreamProtocol::new(DeniedUpgrade, ())
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

impl PeerDiscovery {
    /// Create a new peer discovery system
    pub async fn new(
        keypair: &Keypair,
        bootstrap_nodes: Vec<(PeerId, Multiaddr)>,
        enable_mdns: bool,
    ) -> Result<(Self, mpsc::Receiver<DiscoveryEvent>), Box<dyn Error>> {
        let local_peer_id = PeerId::from(keypair.public());
        
        // Set up Kademlia DHT for peer discovery
        let mut kad_config = KademliaConfig::default();
        kad_config.set_protocol_name(b"/supernova/kad/1.0.0");
        kad_config.set_query_timeout(Duration::from_secs(60));
        kad_config.set_record_ttl(Some(Duration::from_secs(3600 * 24))); // 24 hours
        
        let store = MemoryStore::new(local_peer_id.clone());
        let kademlia = Kademlia::new(local_peer_id.clone(), store);
        
        // Set up mDNS for local network discovery if enabled
        let mdns = if enable_mdns {
            let mdns_config = MdnsConfig::default();
            match Mdns::new(mdns_config).await {
                Ok(mdns) => Some(mdns),
                Err(e) => {
                    warn!("Failed to initialize mDNS, continuing without local discovery: {}", e);
                    None
                }
            }
        } else {
            None
        };
        
        // Create event channel
        let (event_tx, event_rx) = mpsc::channel(128);
        
        Ok((
            Self {
                kademlia: Some(kademlia),
                mdns,
                active_queries: HashMap::new(),
                known_peers: Arc::new(Mutex::new(HashMap::new())),
                verified_peers: HashSet::new(),
                bootstrap_nodes,
                bootstrap_complete: false,
                event_sender: event_tx,
                last_bootstrap: None,
                bootstrap_interval: Duration::from_secs(3600), // Re-bootstrap every hour
                local_peer_id,
            },
            event_rx
        ))
    }
    
    /// Start the bootstrap process
    pub fn bootstrap(&mut self) -> Result<(), Box<dyn Error>> {
        // Don't bootstrap too frequently
        let now = Instant::now();
        if let Some(last) = self.last_bootstrap {
            if now.duration_since(last) < self.bootstrap_interval {
                return Ok(());
            }
        }
        
        info!("Starting Kademlia bootstrap process");
        self.last_bootstrap = Some(now);
        
        // Add bootstrap nodes to Kademlia
        if let Some(kademlia) = &mut self.kademlia {
            for (peer_id, addr) in &self.bootstrap_nodes {
                kademlia.add_address(peer_id, addr.clone());
            }
            
            // Start bootstrap process
            match kademlia.bootstrap() {
                Ok(query_id) => {
                    self.active_queries.insert(query_id, QueryType::Bootstrap);
                    info!("Bootstrap started with query ID: {:?}", query_id);
                    Ok(())
                }
                Err(e) => {
                    error!("Failed to bootstrap Kademlia: {}", e);
                    Err(Box::new(std::io::Error::new(std::io::ErrorKind::Other, 
                                                   format!("Bootstrap error: {}", e))))
                }
            }
        } else {
            Err(Box::new(std::io::Error::new(std::io::ErrorKind::Other, 
                                           "Kademlia not initialized")))
        }
    }
    
    /// Handle a Kademlia event
    pub async fn handle_kademlia_event(&mut self, event: KademliaEvent) -> Result<(), Box<dyn Error>> {
        match event {
            KademliaEvent::OutboundQueryCompleted { id, result, .. } => {
                let query_type = self.active_queries.remove(&id);
                
                match (query_type, result) {
                    (Some(QueryType::Bootstrap), QueryResult::Bootstrap(Ok(_))) => {
                        info!("Kademlia bootstrap completed successfully");
                        self.bootstrap_complete = true;
                        if let Err(e) = self.event_sender.send(DiscoveryEvent::BootstrapComplete).await {
                            warn!("Failed to send bootstrap complete event: {}", e);
                        }
                    }
                    (Some(QueryType::Bootstrap), QueryResult::Bootstrap(Err(e))) => {
                        warn!("Kademlia bootstrap failed: {:?}", e);
                        // Retry bootstrap later
                        self.last_bootstrap = Some(Instant::now() - self.bootstrap_interval + Duration::from_secs(300));
                    }
                    (Some(QueryType::FindPeer(peer_id)), QueryResult::GetClosestPeers(Ok(result))) => {
                        debug!("Get closest peers query completed for {}: found {} peers", peer_id, result.peers.len());
                        // The GetClosestPeersOk contains the k closest peers, not addresses for a specific peer
                        // We'll need to handle this differently
                        // For now, just log the result
                    }
                    (Some(QueryType::FindPeer(peer_id)), QueryResult::GetClosestPeers(Err(e))) => {
                        debug!("Get closest peers query failed for {}: {:?}", peer_id, e);
                    }
                    _ => {
                        // Handle other query types if needed
                    }
                }
            }
            KademliaEvent::RoutingUpdated { peer, .. } => {
                debug!("Kademlia routing updated for peer: {}", peer);
                
                // When a peer is added to the routing table, we get its addresses
                // So we can notify about the discovered peer
                if let Err(e) = self.event_sender.send(DiscoveryEvent::PeerDiscovered(peer, vec![])).await {
                    warn!("Failed to send peer discovered event: {}", e);
                }
            }
            _ => {
                // Ignore other events
            }
        }
        
        Ok(())
    }
    
    /// Handle an mDNS event
    pub async fn handle_mdns_event(&mut self, event: MdnsEvent) -> Result<(), Box<dyn Error>> {
        match event {
            MdnsEvent::Discovered(discovered) => {
                for (peer_id, addr) in discovered {
                    if peer_id == self.local_peer_id {
                        continue; // Skip self
                    }
                    
                    debug!("mDNS discovered peer: {} at {}", peer_id, addr);
                    
                    // Add to known peers
                    {
                        let mut known_peers = self.known_peers.lock().unwrap();
                        known_peers
                            .entry(peer_id)
                            .or_insert_with(Vec::new)
                            .push(addr.clone());
                    }
                    
                    // Add to Kademlia routing table if available
                    if let Some(kademlia) = &mut self.kademlia {
                        kademlia.add_address(&peer_id, addr.clone());
                    }
                    
                    // Notify about discovered peer
                    let mut addresses = Vec::new();
                    addresses.push(addr);
                    if let Err(e) = self.event_sender.send(DiscoveryEvent::PeerDiscovered(peer_id, addresses)).await {
                        warn!("Failed to send peer discovered event: {}", e);
                    }
                }
            }
            MdnsEvent::Expired(expired) => {
                for (peer_id, addr) in expired {
                    debug!("mDNS expired peer: {} at {}", peer_id, addr);
                    
                    // Remove address from known peers
                    {
                        let mut known_peers = self.known_peers.lock().unwrap();
                        if let Some(addresses) = known_peers.get_mut(&peer_id) {
                            addresses.retain(|a| a != &addr);
                            if addresses.is_empty() {
                                known_peers.remove(&peer_id);
                                
                                // Notify that peer expired
                                if let Err(e) = self.event_sender.send(DiscoveryEvent::PeerExpired(peer_id)).await {
                                    warn!("Failed to send peer expired event: {}", e);
                                }
                            }
                        }
                    }
                }
            }
        }
        
        Ok(())
    }
    
    /// Add a verified peer
    pub fn add_verified_peer(&mut self, peer_id: PeerId) {
        if self.verified_peers.insert(peer_id) {
            debug!("Added verified peer: {}", peer_id);
        }
    }
    
    /// Remove a peer from verified list
    pub fn remove_verified_peer(&mut self, peer_id: &PeerId) {
        if self.verified_peers.remove(peer_id) {
            debug!("Removed verified peer: {}", peer_id);
        }
    }
    
    /// Get known peers with their addresses
    pub fn get_known_peers(&self) -> HashMap<PeerId, Vec<Multiaddr>> {
        let known_peers = self.known_peers.lock().unwrap();
        known_peers.clone()
    }
    
    /// Get verified peers
    pub fn get_verified_peers(&self) -> HashSet<PeerId> {
        self.verified_peers.clone()
    }
    
    /// Add a bootstrap node
    pub fn add_bootstrap_node(&mut self, peer_id: PeerId, addr: Multiaddr) {
        // Add to bootstrap nodes
        self.bootstrap_nodes.push((peer_id, addr.clone()));
        
        // Add to Kademlia routing table if available
        if let Some(kademlia) = &mut self.kademlia {
            kademlia.add_address(&peer_id, addr);
        }
    }
}

// Implementation for libp2p NetworkBehaviour trait
impl NetworkBehaviour for PeerDiscovery {
    type ProtocolsHandler = DummyHandler;
    type OutEvent = DiscoveryEvent;

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