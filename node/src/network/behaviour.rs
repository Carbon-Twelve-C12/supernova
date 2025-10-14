use libp2p::{
    gossipsub::{Behaviour as Gossipsub, Event as GossipsubEvent},
    identify::{Behaviour as Identify, Event as IdentifyEvent},
    kad::{store::MemoryStore, Behaviour as Kademlia, Event as KademliaEvent},
    mdns::{tokio::Behaviour as Mdns, Event as MdnsEvent},
    swarm::NetworkBehaviour,
    PeerId,
};

/// Combined network behaviour for Supernova
#[derive(NetworkBehaviour)]
#[behaviour(to_swarm = "SupernovaBehaviourEvent")]
pub struct SupernovaBehaviour {
    pub gossipsub: Gossipsub,
    pub kademlia: Kademlia<MemoryStore>,
    pub mdns: Mdns,
    pub identify: Identify,
}

/// Events produced by the Supernova behaviour
#[derive(Debug)]
pub enum SupernovaBehaviourEvent {
    Gossipsub(GossipsubEvent),
    Kademlia(KademliaEvent),
    Mdns(MdnsEvent),
    Identify(IdentifyEvent),
    // Note: keep_alive has no events
}

impl From<GossipsubEvent> for SupernovaBehaviourEvent {
    fn from(event: GossipsubEvent) -> Self {
        SupernovaBehaviourEvent::Gossipsub(event)
    }
}

impl From<KademliaEvent> for SupernovaBehaviourEvent {
    fn from(event: KademliaEvent) -> Self {
        SupernovaBehaviourEvent::Kademlia(event)
    }
}

impl From<MdnsEvent> for SupernovaBehaviourEvent {
    fn from(event: MdnsEvent) -> Self {
        SupernovaBehaviourEvent::Mdns(event)
    }
}

impl From<IdentifyEvent> for SupernovaBehaviourEvent {
    fn from(event: IdentifyEvent) -> Self {
        SupernovaBehaviourEvent::Identify(event)
    }
}

impl SupernovaBehaviour {
    pub fn new(
        local_peer_id: PeerId,
        gossipsub: Gossipsub,
        kademlia: Kademlia<MemoryStore>,
        mdns: Mdns,
        identify: Identify,
    ) -> Self {
        Self {
            gossipsub,
            kademlia,
            mdns,
            identify,
        }
    }
}
