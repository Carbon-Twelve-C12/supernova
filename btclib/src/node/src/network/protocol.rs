use libp2p::gossipsub::{
    self, IdentTopic as Topic, MessageAuthenticity, ValidationMode,
};
use serde::{Deserialize, Serialize};
use std::error::Error;

const BLOCKS_TOPIC: &str = "supernova/blocks/1.0.0";
const TXS_TOPIC: &str = "supernova/transactions/1.0.0";

#[derive(Debug, Serialize, Deserialize)]
pub enum Message {
    NewBlock(Vec<u8>),
    NewTransaction(Vec<u8>),
    GetBlocks { start: u64, end: u64 },
    BlockResponse(Vec<Vec<u8>>),
}

pub struct Protocol {
    gossipsub: gossipsub::Behaviour,
}

impl Protocol {
    pub fn new() -> Result<Self, Box<dyn Error>> {
        let gossipsub_config = gossipsub::ConfigBuilder::default()
            .heartbeat_interval(Duration::from_secs(1))
            .validation_mode(ValidationMode::Strict)
            .build()
            .map_err(|e| format!("Failed to build gossipsub config: {}", e))?;

        let gossipsub = gossipsub::Behaviour::new(
            MessageAuthenticity::Signed(id_keys),
            gossipsub_config,
        )?;

        Ok(Self { gossipsub })
    }

    pub fn subscribe_to_topics(&mut self) {
        let blocks_topic = Topic::new(BLOCKS_TOPIC);
        let txs_topic = Topic::new(TXS_TOPIC);
        
        self.gossipsub.subscribe(&blocks_topic)?;
        self.gossipsub.subscribe(&txs_topic)?;
    }

    pub fn publish_block(&mut self, block_data: Vec<u8>) -> Result<MessageId, PublishError> {
        let message = Message::NewBlock(block_data);
        let encoded = bincode::serialize(&message)?;
        self.gossipsub.publish(Topic::new(BLOCKS_TOPIC), encoded)
    }

    pub fn publish_transaction(&mut self, tx_data: Vec<u8>) -> Result<MessageId, PublishError> {
        let message = Message::NewTransaction(tx_data);
        let encoded = bincode::serialize(&message)?;
        self.gossipsub.publish(Topic::new(TXS_TOPIC), encoded)
    }
}