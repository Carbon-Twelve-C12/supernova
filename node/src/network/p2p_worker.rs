use libp2p::{
    Swarm, PeerId, Multiaddr,
    swarm::SwarmEvent,
    gossipsub::TopicHash,
};
use tokio::sync::mpsc;
use futures::StreamExt;
use tracing::{info, debug, warn, error};
use std::time::Duration;
use crate::network::{
    behaviour::SupernovaBehaviour,
    p2p::{SwarmCommand, SwarmEventWrapper},
};

/// P2P worker that runs in a dedicated thread to handle the non-Send Swarm
pub struct P2PWorker {
    swarm: Swarm<SupernovaBehaviour>,
    command_rx: mpsc::Receiver<SwarmCommand>,
    event_tx: mpsc::Sender<SwarmEventWrapper>,
}

impl P2PWorker {
    /// Create a new P2P worker
    pub fn new(
        swarm: Swarm<SupernovaBehaviour>,
        command_rx: mpsc::Receiver<SwarmCommand>,
        event_tx: mpsc::Sender<SwarmEventWrapper>,
    ) -> Self {
        Self {
            swarm,
            command_rx,
            event_tx,
        }
    }

    /// Run the worker event loop
    pub async fn run(mut self) {
        info!("P2P worker started");

        loop {
            tokio::select! {
                // Handle commands
                Some(cmd) = self.command_rx.recv() => {
                    match cmd {
                        SwarmCommand::Dial(addr) => {
                            match self.swarm.dial(addr.clone()) {
                                Ok(_) => debug!("Dialing {}", addr),
                                Err(e) => warn!("Failed to dial {}: {}", addr, e),
                            }
                        }
                        SwarmCommand::Publish(topic, data) => {
                            match self.swarm.behaviour_mut().gossipsub.publish(topic.clone(), data) {
                                Ok(_) => debug!("Published message to topic"),
                                Err(e) => warn!("Failed to publish message: {:?}", e),
                            }
                        }
                        SwarmCommand::Stop => {
                            info!("P2P worker stopping");
                            break;
                        }
                    }
                }

                // Handle swarm events
                event = self.swarm.next() => {
                    if let Some(event) = event {
                        if let Ok(wrapped) = SwarmEventWrapper::from_event(event) {
                            if let Err(e) = self.event_tx.send(wrapped).await {
                                error!("Failed to send swarm event: {}", e);
                            }
                        }
                    }
                }
            }
        }

        info!("P2P worker stopped");
    }
}