use libp2p::{
    core::muxing::StreamMuxerBox,
    core::transport::Boxed,
    futures::StreamExt,
    identify, identity, kad,
    mdns::Mdns,
    noise,
    swarm::{Swarm, SwarmEvent},
    tcp, yamux, PeerId, Transport,
};
use std::error::Error;
use std::time::Duration;
use tokio::sync::mpsc;
use tracing::{info, warn};

pub struct P2PNetwork {
    swarm: Swarm<ComposedBehaviour>,
    command_receiver: mpsc::Receiver<NetworkCommand>,
    event_sender: mpsc::Sender<NetworkEvent>,
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
}

pub enum NetworkEvent {
    NewPeer(PeerId),
    PeerLeft(PeerId),
    NewBlock(Vec<u8>),
    NewTransaction(Vec<u8>),
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

        Ok((
            Self {
                swarm,
                command_receiver,
                event_sender,
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
                            if let Err(e) = self.swarm.dial(peer) {
                                warn!("Failed to dial provider: {}", e);
                            }
                        }
                    }
                    _ => {}
                }
            }
            SwarmEvent::Behaviour(ComposedEvent::Mdns(mdns::Event::Discovered(list))) => {
                for (peer_id, _) in list {
                    self.event_sender.send(NetworkEvent::NewPeer(peer_id)).await.ok();
                }
            }
            SwarmEvent::Behaviour(ComposedEvent::Mdns(mdns::Event::Expired(list))) => {
                for (peer_id, _) in list {
                    self.event_sender.send(NetworkEvent::PeerLeft(peer_id)).await.ok();
                }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_network_creation() {
        let (network, _command_sender, _event_receiver) = P2PNetwork::new().await.unwrap();
        assert!(network.swarm.connected_peers().count() == 0);
    }
}