mod worker;
mod coordinator;

pub use self::worker::MiningWorker;
pub use self::coordinator::Miner;

pub const NOVA_TOTAL_SUPPLY: u64 = 42_000_000;
pub const NOVA_BLOCK_REWARD: u64 = 50; // Initial block reward in NOVA