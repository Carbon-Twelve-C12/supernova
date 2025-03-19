mod protocol;
mod p2p;
pub mod sync;

pub use p2p::{P2PNetwork, NetworkCommand, NetworkEvent};
pub use protocol::{Message, Protocol, Checkpoint};

pub use libp2p::{
    PeerId,
    gossipsub::MessageId,
    identity::Keypair,
};