use tokio::sync::mpsc;
use std::collections::HashMap;
use std::error::Error;

pub struct ChainSync {
    height: u64,
    pending_blocks: HashMap<u64, Vec<u8>>,
    sync_state: SyncState,
    command_sender: mpsc::Sender<NetworkCommand>,
}

enum SyncState {
    Idle,
    Syncing { target_height: u64 },
}

impl ChainSync {
    pub fn new(command_sender: mpsc::Sender<NetworkCommand>) -> Self {
        Self {
            height: 0,
            pending_blocks: HashMap::new(),
            sync_state: SyncState::Idle,
            command_sender,
        }
    }

    pub async fn start_sync(&mut self, target_height: u64) -> Result<(), Box<dyn Error>> {
        if target_height <= self.height {
            return Ok(());
        }

        self.sync_state = SyncState::Syncing { target_height };
        
        let message = Message::GetBlocks {
            start: self.height + 1,
            end: target_height,
        };

        self.command_sender
            .send(NetworkCommand::BroadcastMessage(message))
            .await?;

        Ok(())
    }

    pub fn process_block(&mut self, height: u64, block: Vec<u8>) -> Result<(), Box<dyn Error>> {
        self.pending_blocks.insert(height, block);
        self.try_process_pending_blocks()?;
        Ok(())
    }

    fn try_process_pending_blocks(&mut self) -> Result<(), Box<dyn Error>> {
        while let Some(block) = self.pending_blocks.remove(&(self.height + 1)) {
            // Validate and process block
            self.height += 1;
        }
        Ok(())
    }
}