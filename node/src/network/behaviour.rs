use libp2p::{
    gossipsub::{Event as GossipsubEvent, Behaviour as Gossipsub},
    identify::{Event as IdentifyEvent, Behaviour as Identify},
    kad::{Event as KademliaEvent, store::MemoryStore, Behaviour as Kademlia},
    mdns::{Event as MdnsEvent, tokio::Behaviour as Mdns},
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