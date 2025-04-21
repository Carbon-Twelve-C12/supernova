use libp2p::{
    core::muxing::StreamMuxerBox,
    core::transport::Boxed,
    futures::StreamExt,
    identify, identity, kad,
    mdns::Mdns,
    multiaddr::Protocol as MultiAddrProtocol,
    noise,
    swarm::{Swarm, SwarmEvent},
    tcp, yamux, PeerId, Transport,
};
use std::error::Error;
use std::net::IpAddr;
use std::time::Duration;
use tokio::sync::mpsc;
use tracing::{info, warn, error};

use super::peer_manager::{PeerManager, PeerInfo};

pub struct P2PNetwork {
    swarm: Swarm<ComposedBehaviour>,
    command_receiver: mpsc::Receiver<NetworkCommand>,
    event_sender: mpsc::Sender<NetworkEvent>,
    peer_manager: PeerManager,
}

#[derive(libp2p::swarm::NetworkBehaviour)]
#[behaviour(out_event = "ComposedEvent")]
struct ComposedBehaviour {
    kad: kad::Behaviour<kad::store::MemoryStore>,
    identify: identify::Behaviour,
    mdns: Mdns,
}

enum ComposedEvent {
    Kad(kad::Event),
    Identify(identify::Event),
    Mdns(mdns::Event),
}

pub enum NetworkCommand {
    StartListening(String),
    Dial(String),
    AnnounceBlock(Vec<u8>),
    AnnounceTransaction(Vec<u8>),
    GetPeerInfo,
    RotatePeers,
}

pub enum NetworkEvent {
    NewPeer(PeerId),
    PeerLeft(PeerId),
    NewBlock(Vec<u8>),
    NewTransaction(Vec<u8>),
    PeerInfo(Vec<PeerInfo>),
    PeerRotationPlan(Vec<PeerId>, Vec<PeerId>),
}

impl P2PNetwork {
    pub async fn new() -> Result<(Self, mpsc::Sender<NetworkCommand>, mpsc::Receiver<NetworkEvent>), Box<dyn Error>> {
        let id_keys = identity::Keypair::generate_ed25519();
        let peer_id = PeerId::from(id_keys.public());
        info!("Local peer id: {}", peer_id);

        let transport = build_transport(id_keys.clone())?;
        let behaviour = build_behaviour(id_keys.clone()).await?;
        let swarm = Swarm::new(transport, behaviour, peer_id);

        let (command_sender, command_receiver) = mpsc::channel(32);
        let (event_sender, event_receiver) = mpsc::channel(32);
        let peer_manager = PeerManager::new();

        Ok((
            Self {
                swarm,
                command_receiver,
                event_sender,
                peer_manager,
            },
            command_sender,
            event_receiver,
        ))
    }

    pub async fn run(&mut self) {
        loop {
            tokio::select! {
                event = self.swarm.select_next_some() => self.handle_swarm_event(event).await,
                command = self.command_receiver.recv() => {
                    if let Some(cmd) = command {
                        self.handle_command(cmd).await;
                    } else {
                        break;
                    }
                }
            }
        }
    }

    async fn handle_swarm_event(&mut self, event: SwarmEvent<ComposedEvent>) {
        match event {
            SwarmEvent::Behaviour(ComposedEvent::Kad(kad::Event::OutboundQueryCompleted { result, .. })) => {
                match result {
                    kad::QueryResult::GetProviders(Ok(provider_peers)) => {
                        for peer in provider_peers.providers {
                            // Only dial if not violating diversity limits
                            self.try_dial_peer(peer).await;
                        }
                    }
                    _ => {}
                }
            }
            SwarmEvent::Behaviour(ComposedEvent::Mdns(mdns::Event::Discovered(list))) => {
                for (peer_id, addr) in list {
                    // Extract IP address and port from multiaddr
                    if let Some((ip, port)) = extract_ip_port(&addr) {
                        match self.peer_manager.try_add_connection(peer_id, ip, port) {
                            Ok(_) => {
                                self.event_sender.send(NetworkEvent::NewPeer(peer_id)).await.ok();
                                info!("New peer discovered: {}", peer_id);
                            }
                            Err(e) => {
                                warn!("Rejected peer connection from {}: {}", peer_id, e);
                                // If the peer was rejected for diversity reasons, we might
                                // disconnect it later in a controlled manner
                            }
                        }
                    }
                }
            }
            SwarmEvent::Behaviour(ComposedEvent::Mdns(mdns::Event::Expired(list))) => {
                for (peer_id, _) in list {
                    self.peer_manager.handle_disconnect(&peer_id, Some("MDNS expiration".to_string()));
                    self.event_sender.send(NetworkEvent::PeerLeft(peer_id)).await.ok();
                    info!("Peer connection expired: {}", peer_id);
                }
            }
            SwarmEvent::ConnectionEstablished { peer_id, endpoint, .. } => {
                if let Some(addr) = endpoint.get_remote_address() {
                    if let Some((ip, port)) = extract_ip_port(addr) {
                        match self.peer_manager.try_add_connection(peer_id, ip, port) {
                            Ok(_) => {
                                info!("Connection established with {}", peer_id);
                            }
                            Err(e) => {
                                warn!("Connection established but rejected by peer manager: {}", e);
                                // If diversity limits are violated, disconnect
                                if e.contains("diversity limits") {
                                    info!("Disconnecting peer due to diversity limits: {}", peer_id);
                                    self.swarm.disconnect_peer_id(peer_id);
                                }
                            }
                        }
                    }
                }
            }
            SwarmEvent::ConnectionClosed { peer_id, cause, .. } => {
                let reason = format!("Connection closed: {:?}", cause);
                self.peer_manager.handle_disconnect(&peer_id, Some(reason));
                info!("Connection closed with {}: {:?}", peer_id, cause);
            }
            _ => {}
        }
    }

    async fn handle_command(&mut self, command: NetworkCommand) {
        match command {
            NetworkCommand::StartListening(addr) => {
                if let Err(e) = self.swarm.listen_on(addr.parse().unwrap()) {
                    warn!("Failed to start listening: {}", e);
                }
            }
            NetworkCommand::Dial(addr) => {
                if let Err(e) = self.swarm.dial(addr.parse().unwrap()) {
                    warn!("Failed to dial address: {}", e);
                }
            }
            NetworkCommand::AnnounceBlock(data) => {
                // TODO: Implement block announcement
            }
            NetworkCommand::AnnounceTransaction(data) => {
                // TODO: Implement transaction announcement
            }
            NetworkCommand::GetPeerInfo => {
                // Collect info about all connected peers
                let peer_infos: Vec<PeerInfo> = self.peer_manager.get_connected_peer_infos();
                self.event_sender.send(NetworkEvent::PeerInfo(peer_infos)).await.ok();
            }
            NetworkCommand::RotatePeers => {
                // Request a peer rotation plan from the peer manager
                if let Some((to_disconnect, to_connect)) = self.peer_manager.create_rotation_plan() {
                    // Notify about the plan
                    self.event_sender.send(NetworkEvent::PeerRotationPlan(
                        to_disconnect.clone(), 
                        to_connect.clone()
                    )).await.ok();
                    
                    // Execute the plan
                    self.execute_rotation_plan(to_disconnect, to_connect).await;
                }
            }
        }
    }
    
    async fn try_dial_peer(&mut self, peer_id: PeerId) {
        // Check if we're already connected
        if self.swarm.is_connected(&peer_id) {
            return;
        }
        
        // Check if we have too many connections from this peer's subnet
        // For now, just dial - the connection will be evaluated when established
        if let Err(e) = self.swarm.dial(peer_id) {
            warn!("Failed to dial peer {}: {}", peer_id, e);
        }
    }
    
    async fn execute_rotation_plan(&mut self, to_disconnect: Vec<PeerId>, to_connect: Vec<PeerId>) {
        // Disconnect peers to improve diversity
        for peer_id in &to_disconnect {
            info!("Diversity rotation: disconnecting peer {}", peer_id);
            self.swarm.disconnect_peer_id(*peer_id);
            // The disconnection will be handled in the ConnectionClosed event
        }
        
        // Connect to new peers to improve diversity
        for peer_id in &to_connect {
            info!("Diversity rotation: connecting to peer {}", peer_id);
            if let Err(e) = self.swarm.dial(*peer_id) {
                warn!("Failed to dial peer for rotation {}: {}", peer_id, e);
            }
        }
    }
}

fn build_transport(
    id_keys: identity::Keypair,
) -> Result<Boxed<(PeerId, StreamMuxerBox)>, Box<dyn Error>> {
    let noise_keys = noise::Keypair::<noise::X25519Spec>::new()
        .into_authentic(&id_keys)
        .expect("Signing libp2p-noise static DH keypair failed.");

    Ok(tcp::TokioTcpConfig::new()
        .nodelay(true)
        .upgrade(libp2p::core::upgrade::Version::V1)
        .authenticate(noise::NoiseConfig::xx(noise_keys).into_authenticated())
        .multiplex(yamux::YamuxConfig::default())
        .boxed())
}

async fn build_behaviour(
    id_keys: identity::Keypair,
) -> Result<ComposedBehaviour, Box<dyn Error>> {
    let kad_store = kad::store::MemoryStore::new(id_keys.public().to_peer_id());
    let kad_config = kad::Config::default();
    let kad_behaviour = kad::Behaviour::new(
        id_keys.public().to_peer_id(),
        kad_store,
        kad_config,
    );

    let identify = identify::Behaviour::new(identify::Config::new(
        "supernova/1.0.0".into(),
        id_keys.public(),
    ));

    let mdns = Mdns::new(Default::default()).await?;

    Ok(ComposedBehaviour {
        kad: kad_behaviour,
        identify,
        mdns,
    })
}

// Helper function to extract IP and port from a multiaddr
fn extract_ip_port(addr: &libp2p::Multiaddr) -> Option<(IpAddr, u16)> {
    let mut iter = addr.iter();
    
    // Look for IP protocol in the multiaddr
    let ip = loop {
        match iter.next() {
            Some(MultiAddrProtocol::Ip4(ip)) => break IpAddr::V4(ip),
            Some(MultiAddrProtocol::Ip6(ip)) => break IpAddr::V6(ip),
            Some(_) => continue,
            None => return None,
        }
    };
    
    // Look for TCP or UDP protocol with port
    match iter.next() {
        Some(MultiAddrProtocol::Tcp(port)) => Some((ip, port)),
        Some(MultiAddrProtocol::Udp(port)) => Some((ip, port)),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_network_creation() {
        let (network, _command_sender, _event_receiver) = P2PNetwork::new().await.unwrap();
        assert!(network.swarm.connected_peers().count() == 0);
    }
}