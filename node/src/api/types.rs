//! API data types for requests and responses
//!
//! This module defines the data structures used in the supernova API for
//! serializing and deserializing requests and responses.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use utoipa::ToSchema;
use actix_web::{HttpResponse, Responder};

// Import and re-export environmental types
pub mod environmental;
pub use environmental::*;

/// Standard API response wrapper
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct ApiResponse<T> {
    /// Whether the request was successful
    pub success: bool,
    /// Response data (only present if success is true)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<T>,
    /// Error message (only present if success is false)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

impl<T> ApiResponse<T> {
    /// Create a successful response
    pub fn success(data: T) -> Self {
        Self {
            success: true,
            data: Some(data),
            error: None,
        }
    }
    
    /// Create an error response
    pub fn error(message: impl Into<String>) -> Self {
        Self {
            success: false,
            data: None,
            error: Some(message.into()),
        }
    }
}

/// API error response
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct ApiErrorResponse {
    /// Error message
    pub error: String,
    /// Error code
    pub code: u32,
    /// Additional details
    pub details: Option<serde_json::Value>,
}

//
// Blockchain data types
//

/// Blockchain information response
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct BlockchainInfo {
    /// Current block height
    pub height: u64,
    /// Best block hash
    pub best_block_hash: String,
    /// Current difficulty
    pub difficulty: f64,
    /// Total work
    pub total_work: String,
    /// Network name
    pub network: String,
    /// Software version
    pub version: String,
}

/// Block information response
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct BlockInfo {
    /// Block hash
    pub hash: String,
    /// Block height
    pub height: u64,
    /// Number of confirmations
    pub confirmations: u64,
    /// Block size in bytes
    pub size: u64,
    /// Block weight
    pub weight: u64,
    /// Block version
    pub version: u32,
    /// Merkle root
    pub merkle_root: String,
    /// Block timestamp
    pub time: u64,
    /// Block nonce
    pub nonce: u32,
    /// Block difficulty
    pub difficulty: f64,
    /// Previous block hash
    pub previous_block_hash: String,
    /// Next block hash (if exists)
    pub next_block_hash: Option<String>,
    /// Number of transactions
    pub transaction_count: u32,
    /// Transaction IDs
    pub transactions: Vec<String>,
}

/// Block structure
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct Block {
    /// Block header
    pub header: BlockHeader,
    /// Transactions in the block
    pub transactions: Vec<Transaction>,
}

/// Block header structure
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct BlockHeader {
    /// Block version
    pub version: u32,
    /// Previous block hash
    pub prev_hash: String,
    /// Merkle root
    pub merkle_root: String,
    /// Block timestamp
    pub timestamp: u64,
    /// Difficulty target
    pub bits: u32,
    /// Nonce
    pub nonce: u32,
}

/// Transaction information response
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct TransactionInfo {
    /// Transaction ID
    pub txid: String,
    /// Transaction hash
    pub hash: String,
    /// Transaction version
    pub version: u32,
    /// Transaction size in bytes
    pub size: u64,
    /// Virtual size
    pub vsize: u64,
    /// Transaction weight
    pub weight: u64,
    /// Locktime
    pub locktime: u32,
    /// Transaction inputs
    pub inputs: Vec<serde_json::Value>,
    /// Transaction outputs
    pub outputs: Vec<serde_json::Value>,
    /// Block hash containing this transaction (if confirmed)
    pub block_hash: Option<String>,
    /// Block height containing this transaction (if confirmed)
    pub block_height: Option<u64>,
    /// Number of confirmations
    pub confirmations: u64,
    /// Block time (if confirmed)
    pub time: Option<u64>,
    /// Block time (if confirmed)
    pub block_time: Option<u64>,
}

/// Blockchain statistics
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct BlockchainStats {
    /// Current block height
    pub height: u64,
    /// Total number of transactions
    pub total_transactions: u64,
    /// Total number of blocks
    pub total_blocks: u64,
    /// Current difficulty
    pub difficulty: f64,
    /// Network hashrate estimate
    pub hashrate: u64,
    /// Current mempool size
    pub mempool_size: usize,
    /// Current mempool size in bytes
    pub mempool_bytes: usize,
    /// UTXO set size
    pub utxo_set_size: u64,
    /// Chain size in bytes
    pub chain_size_bytes: u64,
}

/// Mempool information response
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct MempoolInfo {
    /// Number of transactions in mempool
    pub transaction_count: usize,
    /// Total size of mempool in bytes
    pub total_size: usize,
    /// Total fees in the mempool
    pub total_fee: u64,
    /// Minimum fee rate
    pub min_fee_rate: u64,
    /// Maximum fee rate
    pub max_fee_rate: u64,
    /// Average fee rate
    pub avg_fee_rate: u64,
}

/// Mempool transaction information
#[derive(Debug, Serialize, Deserialize, ToSchema, Clone)]
pub struct MempoolTransaction {
    /// Transaction ID
    pub txid: String,
    /// Transaction size in bytes
    pub size: usize,
    /// Transaction fee in satoshis
    pub fee: u64,
    /// Fee rate in satoshis per byte
    pub fee_rate: u64,
    /// Time when transaction was added to mempool
    pub time: u64,
}

/// Mempool statistics
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct MempoolStatistics {
    /// Number of transactions
    pub transaction_count: usize,
    /// Total size in bytes
    pub total_size: usize,
    /// Average transaction size
    pub avg_transaction_size: f64,
    /// Total fees
    pub total_fees: u64,
    /// Average fee rate
    pub avg_fee_rate: f64,
    /// Memory usage in bytes
    pub memory_usage: usize,
    /// Minimum fee rate in mempool
    pub min_fee_rate: u64,
    /// Maximum fee rate in mempool
    pub max_fee_rate: u64,
    /// Age of oldest transaction (seconds)
    pub oldest_transaction_age: u64,
}

/// Transaction validation result
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct TransactionValidationResult {
    /// Whether the transaction is valid
    pub valid: bool,
    /// Validation error message
    pub error: Option<String>,
    /// Fee rate in satoshis per byte
    pub fee_rate: Option<u64>,
    /// Transaction size in bytes
    pub size: Option<usize>,
}

/// Transaction fee estimates
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct TransactionFees {
    /// Low priority fee rate (satoshis per byte)
    pub low_priority: u64,
    /// Normal priority fee rate (satoshis per byte)
    pub normal_priority: u64,
    /// High priority fee rate (satoshis per byte)
    pub high_priority: u64,
    /// Target number of blocks for confirmation
    pub target_blocks: u32,
}

//
// Network data types
//

/// Network information response
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct NetworkInfo {
    /// Network version
    pub version: String,
    /// Protocol version
    pub protocol_version: u32,
    /// Number of connections
    pub connections: u32,
    /// Number of inbound connections
    pub inbound_connections: u32,
    /// Number of outbound connections
    pub outbound_connections: u32,
    /// Network type (mainnet, testnet, etc.)
    pub network: String,
    /// Whether the node is listening
    pub is_listening: bool,
    /// Whether the node accepts inbound connections
    pub accepts_incoming: bool,
    /// Local addresses
    pub local_addresses: Vec<NetworkAddress>,
    /// External IP address (if detected)
    pub external_ip: Option<String>,
    /// Network stats
    pub network_stats: NetworkStats,
}

/// Network address
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct NetworkAddress {
    /// Address
    pub address: String,
    /// Port
    pub port: u16,
    /// Score
    pub score: u32,
}

/// Network statistics
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct NetworkStats {
    /// Total bytes sent
    pub total_bytes_sent: u64,
    /// Total bytes received
    pub total_bytes_received: u64,
    /// Upload rate in bytes per second
    pub upload_rate: f64,
    /// Download rate in bytes per second
    pub download_rate: f64,
    /// Ping times in milliseconds
    pub ping_time: f64,
}

/// Peer information
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct PeerInfo {
    /// Peer ID
    pub id: u64,
    /// Peer address
    pub address: String,
    /// Connection direction
    pub direction: String,
    /// Connected time in seconds
    pub connected_time: u64,
    /// Last send time
    pub last_send: u64,
    /// Last receive time
    pub last_recv: u64,
    /// Bytes sent
    pub bytes_sent: u64,
    /// Bytes received
    pub bytes_received: u64,
    /// Ping time in milliseconds
    pub ping_time: Option<f64>,
    /// Peer version
    pub version: String,
    /// User agent
    pub user_agent: String,
    /// Peer height
    pub height: u64,
    /// Services
    pub services: String,
    /// Whether the peer is banned
    pub banned: bool,
    /// Peer reputation score
    pub reputation_score: f64,
}

/// Peer connection status
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct PeerConnectionStatus {
    /// Connection state
    pub state: String,
    /// Connection time
    pub connected_time: u64,
    /// Last activity time
    pub last_activity: u64,
}

/// Bandwidth usage statistics
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct BandwidthUsage {
    /// Total bytes sent
    pub total_sent: u64,
    /// Total bytes received
    pub total_received: u64,
    /// Current upload rate (bytes/sec)
    pub upload_rate: f64,
    /// Current download rate (bytes/sec)
    pub download_rate: f64,
    /// Peak upload rate (bytes/sec)
    pub peak_upload_rate: f64,
    /// Peak download rate (bytes/sec)
    pub peak_download_rate: f64,
}

/// Peer add request
#[derive(Debug, Deserialize, ToSchema)]
pub struct PeerAddRequest {
    /// Peer address
    pub address: String,
    /// Whether to make connection permanent
    #[serde(default)]
    pub permanent: Option<bool>,
}

/// Peer add response
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct PeerAddResponse {
    /// Whether the peer was added successfully
    pub success: bool,
    /// Peer ID (if successful)
    pub peer_id: Option<String>,
    /// Error message (if failed)
    pub error: Option<String>,
}

/// Node address
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct NodeAddress {
    /// Address string
    pub address: String,
    /// Port number
    pub port: u16,
}

/// Connection count
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct ConnectionCount {
    /// Total connections
    pub total: u32,
    /// Inbound connections
    pub inbound: u32,
    /// Outbound connections
    pub outbound: u32,
}

//
// Mining data types
//

/// Mining information response
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct MiningInfo {
    /// Whether the node is mining
    pub is_mining: bool,
    /// Number of mining threads
    pub mining_threads: usize,
    /// Hashrate in hashes per second
    pub hashrate: u64,
    /// Network difficulty
    pub difficulty: f64,
    /// Network hashrate estimate
    pub network_hashrate: u64,
    /// Current block height
    pub current_height: u64,
    /// Time since last block
    pub seconds_since_last_block: u64,
    /// Transaction fee rates
    pub fee_rates: FeeTiers,
    /// Environmental impact
    pub environmental_impact: Option<EnvironmentalImpact>,
}

/// Fee tiers
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct FeeTiers {
    /// High priority fee rate (satoshis per byte)
    pub high_priority: f64,
    /// Medium priority fee rate (satoshis per byte)
    pub medium_priority: f64,
    /// Low priority fee rate (satoshis per byte)
    pub low_priority: f64,
    /// Minimum fee rate (satoshis per byte)
    pub minimum: f64,
}

/// Block template response
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct MiningTemplate {
    /// Block version
    pub version: u32,
    /// Previous block hash
    pub prev_hash: String,
    /// Block timestamp
    pub timestamp: u64,
    /// Block height
    pub height: u64,
    /// Block difficulty target
    pub target: u32,
    /// Merkle root
    pub merkle_root: String,
    /// Transactions
    pub transactions: Vec<TemplateTransaction>,
    /// Total fees
    pub total_fees: u64,
    /// Block size in bytes
    pub size: usize,
    /// Block weight
    pub weight: usize,
    /// Estimated time to mine block
    pub estimated_time_to_mine: f64,
    /// Environmental data
    pub environmental_data: Option<TemplateEnvironmentalData>,
}

/// Template transaction
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct TemplateTransaction {
    /// Transaction ID
    pub txid: String,
    /// Transaction data in hex
    pub data: String,
    /// Transaction fee
    pub fee: u64,
    /// Transaction weight
    pub weight: usize,
    /// Ancestor fee (for sorting)
    pub ancestor_fee: u64,
    /// Ancestor weight (for sorting)
    pub ancestor_weight: usize,
}

/// Template environmental data
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct TemplateEnvironmentalData {
    /// Estimated energy consumption for this block
    pub estimated_energy_kwh: f64,
    /// Estimated carbon emissions
    pub estimated_carbon_grams: f64,
    /// Green mining bonus
    pub green_mining_bonus: u64,
}

/// Mining statistics
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct MiningStats {
    /// Total hashes computed
    pub total_hashes: u64,
    /// Blocks found
    pub blocks_found: u64,
    /// Mining uptime in seconds
    pub uptime_seconds: u64,
    /// Average hashrate over last hour
    pub avg_hashrate_1h: f64,
    /// Current difficulty
    pub current_difficulty: f64,
    /// Estimated time to next block
    pub estimated_time_to_block: f64,
    /// Power consumption estimate (watts)
    pub power_consumption_watts: f64,
    /// Energy efficiency (J/TH)
    pub energy_efficiency: f64,
    /// Carbon emissions (gCO2/hash)
    pub carbon_emissions_per_hash: f64,
    /// Renewable energy percentage
    pub renewable_percentage: f64,
}

/// Submit block response
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct SubmitBlockResponse {
    /// Whether the block was accepted
    pub accepted: bool,
    /// Block hash
    pub block_hash: String,
    /// Rejection reason (if any)
    pub reject_reason: Option<String>,
}

/// Submit block request
#[derive(Debug, Deserialize, ToSchema)]
pub struct SubmitBlockRequest {
    /// Block data in hexadecimal format
    pub block_data: String,
}

/// Mining status
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct MiningStatus {
    /// Current mining state
    pub state: String,
    /// Number of active workers
    pub active_workers: usize,
    /// Current template age in seconds
    pub template_age_seconds: u64,
    /// Hashrate over different time periods
    pub hashrate_1m: u64,
    pub hashrate_5m: u64,
    pub hashrate_15m: u64,
    /// Hardware temperature (if available)
    pub hardware_temperature: Option<f64>,
    /// Fan speed percentage (if available)
    pub fan_speed_percentage: Option<f64>,
}

/// Mining configuration
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct MiningConfiguration {
    /// Number of mining threads
    pub threads: Option<u32>,
    /// Mining intensity (0.0 to 1.0)
    pub intensity: Option<f64>,
    /// Target temperature (Celsius)
    pub target_temperature: Option<f64>,
    /// Enable green mining features
    pub green_mining_enabled: Option<bool>,
    /// Quantum-resistant mining
    pub quantum_resistant: Option<bool>,
    /// Custom mining algorithm parameters
    pub algorithm_params: Option<HashMap<String, serde_json::Value>>,
}

/// Environmental treasury status
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct EnvironmentalTreasuryStatus {
    /// Treasury balance in NOVA
    pub balance: f64,
    /// Transaction fee allocation percentage
    pub fee_allocation_percentage: f64,
    /// Total carbon offsets purchased in tons CO2e
    pub total_carbon_offsets: f64,
    /// Total renewable energy certificates purchased in MWh
    pub total_renewable_certificates: f64,
    /// Carbon negativity percentage
    pub carbon_negativity_percentage: f64,
    /// Treasury allocations
    pub allocations: HashMap<String, f64>,
}

/// Emissions information
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct EmissionsInfo {
    /// Carbon emissions in grams CO2e
    pub carbon_emissions: f64,
    /// Energy consumption in kilowatt-hours
    pub energy_consumption: f64,
    /// Primary energy source if known
    pub energy_source: Option<String>,
    /// Percentage of renewable energy
    pub renewable_percentage: Option<f64>,
    /// Carbon offset amount
    pub carbon_offset: Option<f64>,
    /// Net emissions after offsets
    pub net_emissions: f64,
}



//
// Lightning Network data types
//

/// Lightning Network information
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct LightningInfo {
    /// Node ID
    pub node_id: String,
    /// Number of active channels
    pub num_channels: usize,
    /// Number of pending channels
    pub num_pending_channels: usize,
    /// Number of inactive channels
    pub num_inactive_channels: usize,
    /// Total channel balance in millisatoshis
    pub total_balance_msat: u64,
    /// Total outbound capacity in millisatoshis
    pub total_outbound_capacity_msat: u64,
    /// Total inbound capacity in millisatoshis
    pub total_inbound_capacity_msat: u64,
    /// Number of peers
    pub num_peers: usize,
    /// Whether synced to chain
    pub synced_to_chain: bool,
    /// Whether synced to graph
    pub synced_to_graph: bool,
    /// Block height
    pub block_height: u64,
}

/// Lightning channel information
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct LightningChannel {
    /// Channel ID
    pub channel_id: String,
    /// Funding transaction ID
    pub funding_txid: String,
    /// Output index
    pub output_index: u32,
    /// Remote node ID
    pub remote_node_id: String,
    /// Channel capacity in satoshis
    pub capacity: u64,
    /// Local balance in millisatoshis
    pub local_balance_msat: u64,
    /// Remote balance in millisatoshis
    pub remote_balance_msat: u64,
    /// Channel state
    pub state: String,
    /// Whether channel is public
    pub is_public: bool,
    /// Whether channel is active
    pub is_active: bool,
    /// Total sent in millisatoshis
    pub total_sent_msat: u64,
    /// Total received in millisatoshis
    pub total_received_msat: u64,
    /// Number of updates
    pub num_updates: u64,
    /// CSV delay
    pub csv_delay: u16,
    /// Local channel reserve in satoshis
    pub local_reserve_sat: u64,
    /// Remote channel reserve in satoshis
    pub remote_reserve_sat: u64,
    /// Commit fee in satoshis
    pub commit_fee: u64,
    /// Fee per kiloweight
    pub fee_per_kw: u64,
    /// Unsettled balance in millisatoshis
    pub unsettled_balance_msat: u64,
    /// Commit weight
    pub commit_weight: u64,
    /// Fee base in millisatoshis
    pub fee_base_msat: u32,
    /// Fee rate in parts per million
    pub fee_rate_ppm: u32,
    /// Last update time
    pub last_update: u64,
}

/// Lightning invoice information
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct LightningInvoice {
    /// Payment hash
    pub payment_hash: String,
    /// Payment request (BOLT11 invoice)
    pub payment_request: String,
    /// Creation date
    pub creation_date: u64,
    /// Expiry date
    pub expiry_date: u64,
    /// Amount in millisatoshis
    pub amount_msat: Option<u64>,
    /// Description
    pub description: Option<String>,
    /// Description hash
    pub description_hash: Option<String>,
    /// Payment preimage
    pub payment_preimage: Option<String>,
    /// Settled date
    pub settled_date: Option<u64>,
    /// Settled amount in millisatoshis
    pub settled_amount_msat: Option<u64>,
    /// State
    pub state: String,
    /// Features
    pub features: HashMap<u32, String>,
}

/// Lightning payment information
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct LightningPayment {
    /// Payment hash
    pub payment_hash: String,
    /// Creation date
    pub creation_date: u64,
    /// Payment preimage
    pub payment_preimage: Option<String>,
    /// Value in millisatoshis
    pub value_msat: u64,
    /// Payment request
    pub payment_request: Option<String>,
    /// Status
    pub status: String,
    /// Fee in millisatoshis
    pub fee_msat: u64,
    /// Creation time in nanoseconds
    pub creation_time_ns: u64,
    /// HTLCs
    pub htlcs: Vec<LightningHtlcInfo>,
    /// Payment index
    pub payment_index: u64,
    /// Failure reason
    pub failure_reason: Option<String>,
}

/// Lightning HTLC information
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct LightningHtlcInfo {
    /// HTLC index
    pub htlc_index: u64,
    /// Attempt time in nanoseconds
    pub attempt_time_ns: u64,
    /// Status
    pub status: String,
    /// Route
    pub route: Vec<LightningRouteHop>,
    /// Attempt ID
    pub attempt_id: u64,
    /// Failure
    pub failure: Option<String>,
}

/// Lightning route hop
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct LightningRouteHop {
    /// Channel ID
    pub channel_id: u64,
    /// Channel capacity
    pub channel_capacity: u64,
    /// Amount to forward in millisatoshis
    pub amount_to_forward_msat: u64,
    /// Fee in millisatoshis
    pub fee_msat: u64,
    /// Expiry
    pub expiry: u32,
    /// Public key
    pub pub_key: String,
}

/// Node information
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct NodeInfo {
    /// Node ID
    pub node_id: String,
    /// Node version
    pub version: String,
    /// Protocol version
    pub protocol_version: u32,
    /// Network name
    pub network: String,
    /// Current block height
    pub height: u64,
    /// Best block hash
    pub best_block_hash: String,
    /// Number of connections
    pub connections: u32,
    /// Whether the node is synced
    pub synced: bool,
    /// Node uptime in seconds
    pub uptime: u64,
}

/// Route information
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct Route {
    /// Total time lock
    pub total_time_lock: u32,
    /// Total fees in millisatoshis
    pub total_fees_msat: u64,
    /// Total amount in millisatoshis
    pub total_amt_msat: u64,
    /// Route hops
    pub hops: Vec<RouteHop>,
}

/// Route hop
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct RouteHop {
    /// Channel ID
    pub chan_id: String,
    /// Channel capacity
    pub chan_capacity: u64,
    /// Amount to forward in millisatoshis
    pub amt_to_forward_msat: u64,
    /// Fee in millisatoshis
    pub fee_msat: u64,
    /// Expiry
    pub expiry: u32,
    /// Public key
    pub pub_key: String,
}

//
// Request types for Lightning Network
//

/// Open channel request
#[derive(Debug, Deserialize, ToSchema)]
pub struct OpenChannelRequest {
    /// Node ID
    pub node_id: String,
    /// Local funding amount in satoshis
    pub local_funding_amount: u64,
    /// Push amount in millisatoshis
    #[serde(default)]
    pub push_amount_msat: u64,
    /// Whether to make the channel private
    #[serde(default)]
    pub private: Option<bool>,
    /// Minimum HTLC value in millisatoshis
    pub min_htlc_msat: Option<u64>,
}

/// Open channel response
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct OpenChannelResponse {
    /// Channel ID
    pub channel_id: String,
    /// Funding transaction ID
    pub funding_txid: String,
    /// Output index
    pub output_index: u32,
}

/// Close channel request
#[derive(Debug, Deserialize, ToSchema)]
pub struct CloseChannelRequest {
    /// Channel ID
    pub channel_id: String,
    /// Whether to force close
    #[serde(default)]
    pub force: Option<bool>,
}

/// Payment request
#[derive(Debug, Deserialize, ToSchema)]
pub struct PaymentRequest {
    /// Payment request (BOLT11 invoice)
    pub payment_request: String,
    /// Amount in millisatoshis (optional, can override invoice amount)
    pub amount_msat: Option<u64>,
    /// Fee limit in millisatoshis
    pub fee_limit_msat: Option<u64>,
    /// Timeout in seconds
    pub timeout_seconds: Option<u32>,
}

/// Payment response
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct PaymentResponse {
    /// Payment hash
    pub payment_hash: String,
    /// Payment preimage
    pub payment_preimage: Option<String>,
    /// Payment route
    pub payment_route: Vec<String>,
    /// Payment error
    pub payment_error: Option<String>,
    /// Payment index
    pub payment_index: u64,
    /// Status
    pub status: String,
    /// Fee in millisatoshis
    pub fee_msat: u64,
    /// Value in millisatoshis
    pub value_msat: u64,
    /// Creation time in nanoseconds
    pub creation_time_ns: u64,
}

/// Invoice request
#[derive(Debug, Deserialize, ToSchema)]
pub struct InvoiceRequest {
    /// Amount in millisatoshis
    pub value_msat: u64,
    /// Description
    pub memo: Option<String>,
    /// Expiry in seconds
    pub expiry: Option<u32>,
    /// Whether invoice is private
    #[serde(default)]
    pub private: Option<bool>,
}

/// Invoice response
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct InvoiceResponse {
    /// Payment request (BOLT11 invoice)
    pub payment_request: String,
    /// Payment hash
    pub payment_hash: String,
    /// Add index
    pub add_index: u64,
}

/// Load average information
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct LoadAverage {
    /// 1 minute load average
    pub one: f64,
    /// 5 minute load average
    pub five: f64,
    /// 15 minute load average
    pub fifteen: f64,
}

/// System information
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct SystemInfo {
    /// Operating system
    pub os: String,
    /// System architecture
    pub arch: String,
    /// Number of CPU cores
    pub cpu_count: u32,
    /// Total memory in bytes
    pub total_memory: u64,
    /// Used memory in bytes
    pub used_memory: u64,
    /// Total swap in bytes
    pub total_swap: u64,
    /// Used swap in bytes
    pub used_swap: u64,
    /// System uptime in seconds
    pub uptime: u64,
    /// Load average
    pub load_average: LoadAverage,
}

/// Log entry
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct LogEntry {
    /// Timestamp
    pub timestamp: u64,
    /// Log level
    pub level: String,
    /// Component that generated the log
    pub component: String,
    /// Log message
    pub message: String,
    /// Additional context
    pub context: Option<HashMap<String, serde_json::Value>>,
}

/// Node status
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct NodeStatus {
    /// Current state
    pub state: String,
    /// Current block height
    pub height: u64,
    /// Best block hash
    pub best_block_hash: String,
    /// Number of peers
    pub peer_count: usize,
    /// Mempool size
    pub mempool_size: usize,
    /// Whether the node is mining
    pub is_mining: bool,
    /// Current hashrate
    pub hashrate: u64,
    /// Current difficulty
    pub difficulty: f64,
    /// Network hashrate estimate
    pub network_hashrate: u64,
}

/// Version information
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct VersionInfo {
    /// Software version
    pub version: String,
    /// Protocol version
    pub protocol_version: u32,
    /// Git commit hash
    pub git_commit: String,
    /// Build date
    pub build_date: String,
    /// Rust version used for compilation
    pub rust_version: String,
}

/// Node metrics
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct NodeMetrics {
    /// Node uptime in seconds
    pub uptime: u64,
    /// Number of connected peers
    pub peer_count: usize,
    /// Current block height
    pub block_height: u64,
    /// Mempool size in transactions
    pub mempool_size: usize,
    /// Mempool size in bytes
    pub mempool_bytes: usize,
    /// Sync progress (0.0 to 1.0)
    pub sync_progress: f64,
    /// Network bytes sent
    pub network_bytes_sent: u64,
    /// Network bytes received
    pub network_bytes_received: u64,
    /// CPU usage percentage
    pub cpu_usage: f64,
    /// Memory usage in bytes
    pub memory_usage: u64,
    /// Disk usage in bytes
    pub disk_usage: u64,
}

/// Block height parameter for API requests
#[derive(Debug, Deserialize, ToSchema)]
pub struct BlockHeightParams {
    /// Block height
    pub height: u64,
}

/// Block hash parameter for API requests
#[derive(Debug, Deserialize, ToSchema)]
pub struct BlockHashParams {
    /// Block hash
    pub hash: String,
}

/// Transaction hash parameter for API requests
#[derive(Debug, Deserialize, ToSchema)]
pub struct TxHashParams {
    /// Transaction hash
    pub txid: String,
}

/// Submit transaction request
#[derive(Debug, Deserialize, ToSchema)]
pub struct SubmitTxRequest {
    /// Raw transaction data in hex format
    pub raw_tx: String,
}

/// Transaction submission response
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct TransactionSubmissionResponse {
    /// Transaction ID if accepted
    pub txid: Option<String>,
    /// Whether the transaction was accepted
    pub accepted: bool,
    /// Error message if rejected
    pub error: Option<String>,
}

/// Node version information
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct NodeVersion {
    /// Software version
    pub version: String,
    /// Protocol version
    pub protocol_version: u32,
    /// Git commit hash
    pub git_commit: String,
    /// Build date
    pub build_date: String,
    /// Rust version
    pub rust_version: String,
    /// Features enabled
    pub features: Vec<String>,
}

/// Node configuration
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct NodeConfiguration {
    /// Network name
    pub network: String,
    /// Data directory
    pub data_dir: String,
    /// Listen address
    pub listen_address: String,
    /// Listen port
    pub listen_port: u16,
    /// Maximum peers
    pub max_peers: usize,
    /// Mining enabled
    pub mining_enabled: bool,
    /// Testnet enabled
    pub testnet_enabled: bool,
    /// Lightning Network enabled
    pub lightning_enabled: bool,
    /// Environmental tracking enabled
    pub environmental_tracking: bool,
}

/// Backup information
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct BackupInfo {
    /// Backup ID
    pub id: String,
    /// Backup timestamp
    pub timestamp: u64,
    /// Backup size in bytes
    pub size: u64,
    /// Backup type
    pub backup_type: String,
    /// Backup status
    pub status: String,
    /// Backup file path
    pub file_path: String,
    /// Verification status
    pub verified: bool,
}

/// Debug information for troubleshooting
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct DebugInfo {
    /// Node information
    pub node_info: NodeInfo,
    /// System information
    pub system_info: SystemInfo,
    /// Performance metrics
    pub performance_metrics: serde_json::Value,
    /// Network statistics
    pub network_stats: serde_json::Value,
    /// Mempool statistics
    pub mempool_stats: serde_json::Value,
    /// Blockchain statistics
    pub blockchain_stats: serde_json::Value,
    /// Lightning Network statistics
    pub lightning_stats: serde_json::Value,
}

/// Faucet information (for testnet)
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct FaucetInfo {
    /// Whether the faucet is enabled
    pub enabled: bool,
    /// Current faucet balance
    pub balance: u64,
    /// Maximum request amount
    pub max_request: u64,
    /// Cooldown period in seconds
    pub cooldown_seconds: u64,
    /// Number of requests today
    pub requests_today: u32,
    /// Daily request limit
    pub daily_limit: u32,
}

/// Faucet status response
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct FaucetStatusResponse {
    /// Whether the faucet is active
    pub is_active: bool,
    /// Current balance
    pub balance: u64,
    /// Number of transactions today
    pub transactions_today: u32,
    /// Last distribution timestamp
    pub last_distribution: Option<i64>,
    /// Cooldown period in seconds
    pub cooldown_secs: u64,
    /// Distribution amount
    pub distribution_amount: u64,
}

/// Faucet coin request
#[derive(Debug, Deserialize, ToSchema)]
pub struct FaucetRequest {
    /// Recipient address
    pub address: String,
}

/// Faucet coin response
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct FaucetResponse {
    /// Transaction ID
    pub txid: String,
    /// Amount distributed
    pub amount: u64,
    /// Recipient address
    pub recipient: String,
    /// Timestamp
    pub timestamp: i64,
}

/// Recent faucet transactions response
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct RecentTransactionsResponse {
    /// List of recent transactions
    pub transactions: Vec<FaucetTransaction>,
}

/// Faucet transaction record
#[derive(Debug, Serialize, Deserialize, ToSchema, Clone)]
pub struct FaucetTransaction {
    /// Transaction ID
    pub txid: String,
    /// Recipient address
    pub recipient: String,
    /// Amount
    pub amount: u64,
    /// Timestamp
    pub timestamp: i64,
}

//
// Wallet API types
//

/// Wallet address information
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct Address {
    /// Address string
    pub address: String,
    /// Address type (P2PKH, P2SH, etc.)
    pub address_type: String,
    /// Whether this is a change address
    pub is_change: bool,
    /// Address index in HD wallet
    pub index: u32,
    /// Address label (if any)
    pub label: Option<String>,
}

/// Extended address information
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct AddressInfo {
    /// Address string
    pub address: String,
    /// Address type
    pub address_type: String,
    /// Balance in NOVA
    pub balance: u64,
    /// Number of transactions
    pub tx_count: u32,
    /// Whether this is a change address
    pub is_change: bool,
    /// Address index
    pub index: u32,
    /// Address label
    pub label: Option<String>,
    /// First seen timestamp
    pub first_seen: Option<u64>,
    /// Last used timestamp
    pub last_used: Option<u64>,
}

/// Wallet information
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct WalletInfo {
    /// Wallet name
    pub name: String,
    /// Total balance in NOVA
    pub balance: u64,
    /// Confirmed balance in NOVA
    pub confirmed_balance: u64,
    /// Unconfirmed balance in NOVA
    pub unconfirmed_balance: u64,
    /// Number of addresses
    pub address_count: u32,
    /// Number of transactions
    pub tx_count: u32,
    /// Whether wallet is encrypted
    pub encrypted: bool,
    /// Whether wallet is locked
    pub locked: bool,
    /// HD wallet master fingerprint
    pub master_fingerprint: Option<String>,
    /// Wallet version
    pub version: u32,
}

/// Balance information
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct BalanceInfo {
    /// Total balance in NOVA
    pub total: u64,
    /// Confirmed balance in NOVA
    pub confirmed: u64,
    /// Unconfirmed balance in NOVA
    pub unconfirmed: u64,
    /// Immature balance (coinbase) in NOVA
    pub immature: u64,
    /// Spendable balance in NOVA
    pub spendable: u64,
}

/// Transaction for wallet API
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct Transaction {
    /// Transaction ID
    pub txid: String,
    /// Transaction version
    pub version: u32,
    /// Transaction size
    pub size: usize,
    /// Virtual size
    pub vsize: usize,
    /// Weight
    pub weight: usize,
    /// Locktime
    pub locktime: u32,
    /// Inputs
    pub inputs: Vec<TransactionInput>,
    /// Outputs
    pub outputs: Vec<TransactionOutput>,
    /// Transaction fee
    pub fee: u64,
    /// Block hash (if confirmed)
    pub block_hash: Option<String>,
    /// Block height (if confirmed)
    pub block_height: Option<u64>,
    /// Confirmations
    pub confirmations: u64,
    /// Timestamp
    pub timestamp: u64,
    /// Transaction label
    pub label: Option<String>,
}

/// Transaction list response
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct TransactionList {
    /// List of transactions
    pub transactions: Vec<Transaction>,
    /// Total count
    pub total_count: usize,
    /// Page number
    pub page: u32,
    /// Page size
    pub page_size: u32,
    /// Whether there are more pages
    pub has_more: bool,
}

/// Sign request
#[derive(Debug, Deserialize, ToSchema)]
pub struct SignRequest {
    /// Message to sign
    pub message: String,
    /// Address to sign with
    pub address: String,
    /// Passphrase (if wallet is encrypted)
    pub passphrase: Option<String>,
}

/// Sign response
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct SignResponse {
    /// Signature
    pub signature: String,
    /// Address used for signing
    pub address: String,
    /// Message that was signed
    pub message: String,
}

/// Verify request
#[derive(Debug, Deserialize, ToSchema)]
pub struct VerifyRequest {
    /// Message that was signed
    pub message: String,
    /// Signature to verify
    pub signature: String,
    /// Address that supposedly signed
    pub address: String,
}

/// Verify response
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct VerifyResponse {
    /// Whether signature is valid
    pub valid: bool,
    /// Address that signed (if valid)
    pub address: Option<String>,
    /// Message that was verified
    pub message: String,
}

/// Send transaction request
#[derive(Debug, Deserialize, ToSchema)]
pub struct SendRequest {
    /// Destination address
    pub to_address: String,
    /// Amount in NOVA
    pub amount: u64,
    /// Transaction fee in NOVA (optional, will estimate if not provided)
    pub fee: Option<u64>,
    /// Transaction label
    pub label: Option<String>,
    /// Subtract fee from amount
    pub subtract_fee_from_amount: Option<bool>,
    /// Passphrase (if wallet is encrypted)
    pub passphrase: Option<String>,
}

/// Send transaction response
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct SendResponse {
    /// Transaction ID
    pub txid: String,
    /// Transaction fee paid
    pub fee: u64,
    /// Raw transaction hex
    pub raw_tx: String,
}

/// Label request
#[derive(Debug, Deserialize, ToSchema)]
pub struct LabelRequest {
    /// Address or transaction ID to label
    pub target: String,
    /// Label to set
    pub label: String,
}

/// Label response
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct LabelResponse {
    /// Target that was labeled
    pub target: String,
    /// Label that was set
    pub label: String,
    /// Success status
    pub success: bool,
}

/// Address generation request
#[derive(Debug, Deserialize, ToSchema)]
pub struct AddressRequest {
    /// Address label
    pub label: Option<String>,
    /// Address type
    pub address_type: Option<String>,
}

/// Address generation response
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct AddressResponse {
    /// Generated address
    pub address: String,
    /// Address type
    pub address_type: String,
    /// Address index
    pub index: u32,
    /// Address label
    pub label: Option<String>,
}

/// Backup response
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct BackupResponse {
    /// Backup data (encrypted)
    pub backup_data: String,
    /// Backup timestamp
    pub timestamp: u64,
    /// Backup version
    pub version: u32,
    /// Checksum for verification
    pub checksum: String,
}

/// UTXO list response
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct UTXOList {
    /// List of UTXOs
    pub utxos: Vec<UTXO>,
    /// Total count
    pub total_count: usize,
    /// Total value in NOVA
    pub total_value: u64,
}

/// UTXO information
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct UTXO {
    /// Transaction ID
    pub txid: String,
    /// Output index
    pub vout: u32,
    /// Output value in NOVA
    pub value: u64,
    /// Output script
    pub script_pub_key: String,
    /// Address (if available)
    pub address: Option<String>,
    /// Block height
    pub height: u64,
    /// Confirmations
    pub confirmations: u64,
    /// Whether this is a coinbase output
    pub coinbase: bool,
    /// Spendable status
    pub spendable: bool,
}

/// Transaction input
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct TransactionInput {
    /// Previous transaction ID
    pub txid: String,
    /// Previous transaction output index
    pub vout: u32,
    /// Script signature
    pub script_sig: String,
    /// Script signature as human-readable ASM
    pub script_sig_asm: String,
    /// Witness data (segwit only)
    pub witness: Option<Vec<String>>,
    /// Sequence number
    pub sequence: u32,
    /// Previous output value in satoshis
    pub value: u64,
    /// Address (if available)
    pub address: Option<String>,
}

/// Transaction output
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct TransactionOutput {
    /// Output value in satoshis
    pub value: u64,
    /// Output script
    pub script_pub_key: String,
    /// Output script as human-readable ASM
    pub script_pub_key_asm: String,
    /// Output script type
    pub script_type: String,
    /// Address (if available)
    pub address: Option<String>,
    /// Whether this output has been spent
    pub spent: Option<bool>,
    /// Transaction ID of spending transaction (if spent)
    pub spent_by_tx: Option<String>,
}

/// Response for mempool transaction submission
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct MempoolTransactionSubmissionResponse {
    /// Transaction ID
    pub txid: String,
    /// Whether the transaction was accepted
    pub accepted: bool,
}

/// Mining work request for getwork-style mining
#[derive(Debug, Deserialize, ToSchema)]
pub struct MiningWorkRequest {
    /// Miner address for rewards
    pub miner_address: String,
}

/// Mining work response
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct MiningWorkResponse {
    /// Block header to work on (hex)
    pub header: String,
    /// Target difficulty (hex)
    pub target: String,
    /// Job ID
    pub job_id: String,
    /// Height of the block being mined
    pub height: u64,
}

/// Mining submit request
#[derive(Debug, Deserialize, ToSchema)]
pub struct MiningSubmitRequest {
    /// Job ID from work request
    pub job_id: String,
    /// Nonce found by miner
    pub nonce: u32,
    /// Miner address
    pub miner_address: String,
}

/// Mining submit response
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct MiningSubmitResponse {
    /// Whether the submission was accepted
    pub accepted: bool,
    /// Block hash if accepted
    pub block_hash: Option<String>,
    /// Rejection reason if not accepted
    pub reject_reason: Option<String>,
}

/// Channel information for Lightning Network
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct ChannelInfo {
    /// Channel ID
    pub channel_id: String,
    /// Remote peer ID
    pub remote_peer: String,
    /// Channel capacity
    pub capacity: u64,
    /// Local balance
    pub local_balance: u64,
    /// Remote balance
    pub remote_balance: u64,
    /// Channel state
    pub state: String,
}

/// Energy usage history entry
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct EnergyUsageHistory {
    /// Timestamp
    pub timestamp: u64,
    /// Energy consumption in kWh
    pub consumption: f64,
    /// Renewable energy percentage
    pub renewable_percentage: f64,
}

// Add Responder implementations for all API types

impl Responder for BlockchainInfo {
    type Body = actix_web::body::BoxBody;
    
    fn respond_to(self, _req: &actix_web::HttpRequest) -> HttpResponse<Self::Body> {
        HttpResponse::Ok().json(self)
    }
}

impl Responder for BlockInfo {
    type Body = actix_web::body::BoxBody;
    
    fn respond_to(self, _req: &actix_web::HttpRequest) -> HttpResponse<Self::Body> {
        HttpResponse::Ok().json(self)
    }
}

impl Responder for TransactionInfo {
    type Body = actix_web::body::BoxBody;
    
    fn respond_to(self, _req: &actix_web::HttpRequest) -> HttpResponse<Self::Body> {
        HttpResponse::Ok().json(self)
    }
}

impl Responder for TransactionSubmissionResponse {
    type Body = actix_web::body::BoxBody;
    
    fn respond_to(self, _req: &actix_web::HttpRequest) -> HttpResponse<Self::Body> {
        HttpResponse::Ok().json(self)
    }
}

impl Responder for BlockchainStats {
    type Body = actix_web::body::BoxBody;
    
    fn respond_to(self, _req: &actix_web::HttpRequest) -> HttpResponse<Self::Body> {
        HttpResponse::Ok().json(self)
    }
}

impl Responder for MempoolInfo {
    type Body = actix_web::body::BoxBody;
    
    fn respond_to(self, _req: &actix_web::HttpRequest) -> HttpResponse<Self::Body> {
        HttpResponse::Ok().json(self)
    }
}

impl Responder for MempoolTransaction {
    type Body = actix_web::body::BoxBody;
    
    fn respond_to(self, _req: &actix_web::HttpRequest) -> HttpResponse<Self::Body> {
        HttpResponse::Ok().json(self)
    }
}

impl Responder for MempoolStatistics {
    type Body = actix_web::body::BoxBody;
    
    fn respond_to(self, _req: &actix_web::HttpRequest) -> HttpResponse<Self::Body> {
        HttpResponse::Ok().json(self)
    }
}

impl Responder for BalanceInfo {
    type Body = actix_web::body::BoxBody;
    
    fn respond_to(self, _req: &actix_web::HttpRequest) -> HttpResponse<Self::Body> {
        HttpResponse::Ok().json(self)
    }
}

impl Responder for AddressInfo {
    type Body = actix_web::body::BoxBody;
    
    fn respond_to(self, _req: &actix_web::HttpRequest) -> HttpResponse<Self::Body> {
        HttpResponse::Ok().json(self)
    }
}

impl Responder for TransactionList {
    type Body = actix_web::body::BoxBody;
    
    fn respond_to(self, _req: &actix_web::HttpRequest) -> HttpResponse<Self::Body> {
        HttpResponse::Ok().json(self)
    }
}

impl Responder for SignResponse {
    type Body = actix_web::body::BoxBody;
    
    fn respond_to(self, _req: &actix_web::HttpRequest) -> HttpResponse<Self::Body> {
        HttpResponse::Ok().json(self)
    }
}

impl Responder for VerifyResponse {
    type Body = actix_web::body::BoxBody;
    
    fn respond_to(self, _req: &actix_web::HttpRequest) -> HttpResponse<Self::Body> {
        HttpResponse::Ok().json(self)
    }
}

impl Responder for SendResponse {
    type Body = actix_web::body::BoxBody;
    
    fn respond_to(self, _req: &actix_web::HttpRequest) -> HttpResponse<Self::Body> {
        HttpResponse::Ok().json(self)
    }
}

impl Responder for LabelResponse {
    type Body = actix_web::body::BoxBody;
    
    fn respond_to(self, _req: &actix_web::HttpRequest) -> HttpResponse<Self::Body> {
        HttpResponse::Ok().json(self)
    }
}

impl Responder for AddressResponse {
    type Body = actix_web::body::BoxBody;
    
    fn respond_to(self, _req: &actix_web::HttpRequest) -> HttpResponse<Self::Body> {
        HttpResponse::Ok().json(self)
    }
}

impl Responder for BackupResponse {
    type Body = actix_web::body::BoxBody;
    
    fn respond_to(self, _req: &actix_web::HttpRequest) -> HttpResponse<Self::Body> {
        HttpResponse::Ok().json(self)
    }
}

impl Responder for UTXOList {
    type Body = actix_web::body::BoxBody;
    
    fn respond_to(self, _req: &actix_web::HttpRequest) -> HttpResponse<Self::Body> {
        HttpResponse::Ok().json(self)
    }
}

impl Responder for EnergyUsage {
    type Body = actix_web::body::BoxBody;
    
    fn respond_to(self, _req: &actix_web::HttpRequest) -> HttpResponse<Self::Body> {
        HttpResponse::Ok().json(self)
    }
}

impl Responder for CarbonFootprint {
    type Body = actix_web::body::BoxBody;
    
    fn respond_to(self, _req: &actix_web::HttpRequest) -> HttpResponse<Self::Body> {
        HttpResponse::Ok().json(self)
    }
}

impl Responder for ResourceUtilization {
    type Body = actix_web::body::BoxBody;
    
    fn respond_to(self, _req: &actix_web::HttpRequest) -> HttpResponse<Self::Body> {
        HttpResponse::Ok().json(self)
    }
}

impl Responder for EnvironmentalSettings {
    type Body = actix_web::body::BoxBody;
    
    fn respond_to(self, _req: &actix_web::HttpRequest) -> HttpResponse<Self::Body> {
        HttpResponse::Ok().json(self)
    }
}

impl Responder for EmissionsInfo {
    type Body = actix_web::body::BoxBody;
    
    fn respond_to(self, _req: &actix_web::HttpRequest) -> HttpResponse<Self::Body> {
        HttpResponse::Ok().json(self)
    }
}

impl Responder for EnvironmentalTreasuryStatus {
    type Body = actix_web::body::BoxBody;
    
    fn respond_to(self, _req: &actix_web::HttpRequest) -> HttpResponse<Self::Body> {
        HttpResponse::Ok().json(self)
    }
}

impl Responder for EnergyUsageHistory {
    type Body = actix_web::body::BoxBody;
    
    fn respond_to(self, _req: &actix_web::HttpRequest) -> HttpResponse<Self::Body> {
        HttpResponse::Ok().json(self)
    }
}

impl Responder for LoadAverage {
    type Body = actix_web::body::BoxBody;
    
    fn respond_to(self, _req: &actix_web::HttpRequest) -> HttpResponse<Self::Body> {
        HttpResponse::Ok().json(self)
    }
}

impl Responder for NodeInfo {
    type Body = actix_web::body::BoxBody;
    
    fn respond_to(self, _req: &actix_web::HttpRequest) -> HttpResponse<Self::Body> {
        HttpResponse::Ok().json(self)
    }
}

impl Responder for SystemInfo {
    type Body = actix_web::body::BoxBody;
    
    fn respond_to(self, _req: &actix_web::HttpRequest) -> HttpResponse<Self::Body> {
        HttpResponse::Ok().json(self)
    }
}

impl Responder for NodeStatus {
    type Body = actix_web::body::BoxBody;
    
    fn respond_to(self, _req: &actix_web::HttpRequest) -> HttpResponse<Self::Body> {
        HttpResponse::Ok().json(self)
    }
}

impl Responder for VersionInfo {
    type Body = actix_web::body::BoxBody;
    
    fn respond_to(self, _req: &actix_web::HttpRequest) -> HttpResponse<Self::Body> {
        HttpResponse::Ok().json(self)
    }
}

impl Responder for NodeMetrics {
    type Body = actix_web::body::BoxBody;
    
    fn respond_to(self, _req: &actix_web::HttpRequest) -> HttpResponse<Self::Body> {
        HttpResponse::Ok().json(self)
    }
}

impl Responder for FaucetInfo {
    type Body = actix_web::body::BoxBody;
    
    fn respond_to(self, _req: &actix_web::HttpRequest) -> HttpResponse<Self::Body> {
        HttpResponse::Ok().json(self)
    }
}

impl Responder for BackupInfo {
    type Body = actix_web::body::BoxBody;
    
    fn respond_to(self, _req: &actix_web::HttpRequest) -> HttpResponse<Self::Body> {
        HttpResponse::Ok().json(self)
    }
}

impl Responder for DebugInfo {
    type Body = actix_web::body::BoxBody;
    
    fn respond_to(self, _req: &actix_web::HttpRequest) -> HttpResponse<Self::Body> {
        HttpResponse::Ok().json(self)
    }
}

impl Responder for MempoolTransactionSubmissionResponse {
    type Body = actix_web::body::BoxBody;
    
    fn respond_to(self, _req: &actix_web::HttpRequest) -> HttpResponse<Self::Body> {
        HttpResponse::Ok().json(self)
    }
}

impl Responder for TransactionValidationResult {
    type Body = actix_web::body::BoxBody;
    
    fn respond_to(self, _req: &actix_web::HttpRequest) -> HttpResponse<Self::Body> {
        HttpResponse::Ok().json(self)
    }
}

impl Responder for TransactionFees {
    type Body = actix_web::body::BoxBody;
    
    fn respond_to(self, _req: &actix_web::HttpRequest) -> HttpResponse<Self::Body> {
        HttpResponse::Ok().json(self)
    }
}

impl Responder for NetworkInfo {
    type Body = actix_web::body::BoxBody;
    
    fn respond_to(self, _req: &actix_web::HttpRequest) -> HttpResponse<Self::Body> {
        HttpResponse::Ok().json(self)
    }
}

impl Responder for ConnectionCount {
    type Body = actix_web::body::BoxBody;
    
    fn respond_to(self, _req: &actix_web::HttpRequest) -> HttpResponse<Self::Body> {
        HttpResponse::Ok().json(self)
    }
}

impl Responder for PeerInfo {
    type Body = actix_web::body::BoxBody;
    
    fn respond_to(self, _req: &actix_web::HttpRequest) -> HttpResponse<Self::Body> {
        HttpResponse::Ok().json(self)
    }
}

impl Responder for PeerAddResponse {
    type Body = actix_web::body::BoxBody;
    
    fn respond_to(self, _req: &actix_web::HttpRequest) -> HttpResponse<Self::Body> {
        HttpResponse::Ok().json(self)
    }
}

impl Responder for BandwidthUsage {
    type Body = actix_web::body::BoxBody;
    
    fn respond_to(self, _req: &actix_web::HttpRequest) -> HttpResponse<Self::Body> {
        HttpResponse::Ok().json(self)
    }
}

impl Responder for MiningInfo {
    type Body = actix_web::body::BoxBody;
    
    fn respond_to(self, _req: &actix_web::HttpRequest) -> HttpResponse<Self::Body> {
        HttpResponse::Ok().json(self)
    }
}

impl Responder for MiningTemplate {
    type Body = actix_web::body::BoxBody;
    
    fn respond_to(self, _req: &actix_web::HttpRequest) -> HttpResponse<Self::Body> {
        HttpResponse::Ok().json(self)
    }
}

impl Responder for SubmitBlockResponse {
    type Body = actix_web::body::BoxBody;
    
    fn respond_to(self, _req: &actix_web::HttpRequest) -> HttpResponse<Self::Body> {
        HttpResponse::Ok().json(self)
    }
}

impl Responder for MiningStats {
    type Body = actix_web::body::BoxBody;
    
    fn respond_to(self, _req: &actix_web::HttpRequest) -> HttpResponse<Self::Body> {
        HttpResponse::Ok().json(self)
    }
}

impl Responder for MiningStatus {
    type Body = actix_web::body::BoxBody;
    
    fn respond_to(self, _req: &actix_web::HttpRequest) -> HttpResponse<Self::Body> {
        HttpResponse::Ok().json(self)
    }
}

impl Responder for MiningConfiguration {
    type Body = actix_web::body::BoxBody;
    
    fn respond_to(self, _req: &actix_web::HttpRequest) -> HttpResponse<Self::Body> {
        HttpResponse::Ok().json(self)
    }
}

impl Responder for WalletInfo {
    type Body = actix_web::body::BoxBody;
    
    fn respond_to(self, _req: &actix_web::HttpRequest) -> HttpResponse<Self::Body> {
        HttpResponse::Ok().json(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_api_response() {
        let success = ApiResponse::success(42);
        assert!(success.success);
        assert_eq!(success.data, Some(42));
        assert_eq!(success.error, None);

        let error = ApiResponse::<i32>::error("Test error");
        assert!(!error.success);
        assert_eq!(error.data, None);
        assert_eq!(error.error, Some("Test error".to_string()));
    }
} 