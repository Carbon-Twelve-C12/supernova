//! API data types for requests and responses
//!
//! This module defines the data structures used in the SuperNova API for
//! serializing and deserializing requests and responses.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use utoipa::ToSchema;

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

//
// Blockchain data types
//

/// Block information response
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct BlockInfo {
    /// Block hash
    pub hash: String,
    /// Block height
    pub height: u64,
    /// Previous block hash
    pub prev_hash: String,
    /// Merkle root
    pub merkle_root: String,
    /// Block timestamp
    pub timestamp: u64,
    /// Block version
    pub version: u32,
    /// Block difficulty target
    pub target: u32,
    /// Block nonce
    pub nonce: u32,
    /// Number of transactions in block
    pub tx_count: usize,
    /// Size of block in bytes
    pub size: usize,
    /// Block weight
    pub weight: usize,
    /// Total transaction fees
    pub fees: u64,
    /// Confirmed status
    pub confirmed: bool,
    /// Confirmations
    pub confirmations: u64,
}

/// Transaction information response
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct TransactionInfo {
    /// Transaction ID
    pub txid: String,
    /// Transaction version
    pub version: u32,
    /// Transaction size in bytes
    pub size: usize,
    /// Transaction weight
    pub weight: usize,
    /// Locktime
    pub locktime: u32,
    /// Block hash containing this transaction (if confirmed)
    pub block_hash: Option<String>,
    /// Block height containing this transaction (if confirmed)
    pub block_height: Option<u64>,
    /// Transaction inputs
    pub inputs: Vec<TransactionInput>,
    /// Transaction outputs
    pub outputs: Vec<TransactionOutput>,
    /// Transaction fee
    pub fee: u64,
    /// Fee rate in satoshis per byte
    pub fee_rate: f64,
    /// Confirmations
    pub confirmations: u64,
    /// Timestamp of confirmation (if confirmed)
    pub confirmed_time: Option<u64>,
    /// Transaction carbon emissions estimate
    pub estimated_emissions: Option<EmissionsInfo>,
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

/// Blockchain information response
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct BlockchainInfo {
    /// Current block height
    pub height: u64,
    /// Best block hash
    pub best_block_hash: String,
    /// Current difficulty
    pub difficulty: f64,
    /// Median time of past several blocks
    pub median_time: u64,
    /// Chain work
    pub chain_work: String,
    /// Verification progress
    pub verification_progress: f64,
    /// Chain size on disk in bytes
    pub size_on_disk: u64,
    /// Network hashrate estimate
    pub network_hashrate: u64,
    /// Whether the blockchain is synced
    pub is_synced: bool,
    /// Sync progress percentage
    pub sync_progress: f64,
}

//
// Mempool data types
//

/// Mempool information response
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct MempoolInfo {
    /// Number of transactions in mempool
    pub tx_count: usize,
    /// Total size of mempool in bytes
    pub size: usize,
    /// Total memory usage of mempool in bytes
    pub memory_usage: usize,
    /// Minimum fee rate in satoshis per byte
    pub min_fee_rate: f64,
    /// Maximum fee rate in satoshis per byte
    pub max_fee_rate: f64,
    /// Median fee rate in satoshis per byte
    pub median_fee_rate: f64,
    /// Mempool transactions grouped by fee rate
    pub fee_histogram: Vec<FeeHistogramEntry>,
}

/// Fee histogram entry
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct FeeHistogramEntry {
    /// Fee rate in satoshis per byte
    pub fee_rate: f64,
    /// Number of transactions at this fee rate
    pub tx_count: usize,
    /// Total size of transactions at this fee rate
    pub size: usize,
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
pub struct BlockTemplate {
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

//
// Environmental data types
//

/// Emissions information response
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

/// Network emissions response
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct NetworkEmissionsInfo {
    /// Total network carbon emissions in tons CO2e
    pub total_carbon_emissions: f64,
    /// Total network energy consumption in megawatt-hours
    pub total_energy_consumption: f64,
    /// Average carbon intensity in gCO2e/kWh
    pub average_carbon_intensity: f64,
    /// Network hashrate in hashes per second
    pub network_hashrate: u64,
    /// Estimated renewable energy percentage
    pub estimated_renewable_percentage: f64,
    /// Region-specific emissions breakdown
    pub regional_breakdown: HashMap<String, RegionalEmissions>,
    /// Carbon offset amount in tons CO2e
    pub carbon_offset: f64,
    /// Net emissions after offsets in tons CO2e
    pub net_emissions: f64,
    /// Carbon negativity percentage
    pub carbon_negativity_percentage: f64,
}

/// Regional emissions
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct RegionalEmissions {
    /// Region name
    pub region: String,
    /// Percentage of hashrate
    pub hashrate_percentage: f64,
    /// Carbon emissions in tons CO2e
    pub carbon_emissions: f64,
    /// Energy consumption in megawatt-hours
    pub energy_consumption: f64,
    /// Carbon intensity in gCO2e/kWh
    pub carbon_intensity: f64,
    /// Estimated renewable energy percentage
    pub estimated_renewable_percentage: f64,
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
pub struct LightningChannelInfo {
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
pub struct LightningInvoiceInfo {
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
pub struct LightningPaymentInfo {
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

//
// Request parameters
//

/// Block height path parameters
#[derive(Debug, Deserialize, ToSchema)]
pub struct BlockHeightParams {
    /// Block height
    pub height: u64,
}

/// Block hash path parameters
#[derive(Debug, Deserialize, ToSchema)]
pub struct BlockHashParams {
    /// Block hash
    pub hash: String,
}

/// Transaction hash path parameters
#[derive(Debug, Deserialize, ToSchema)]
pub struct TxHashParams {
    /// Transaction hash
    pub txid: String,
}

/// Address path parameters
#[derive(Debug, Deserialize, ToSchema)]
pub struct AddressParams {
    /// Address
    pub address: String,
}

/// Submit transaction request
#[derive(Debug, Deserialize, ToSchema)]
pub struct SubmitTxRequest {
    /// Transaction data in hex
    pub tx_data: String,
    /// Whether to allow high fees
    #[serde(default)]
    pub allow_high_fees: bool,
}

/// Submit block request
#[derive(Debug, Deserialize, ToSchema)]
pub struct SubmitBlockRequest {
    /// Block data in hex
    pub block_data: String,
}

/// Transaction emissions request
#[derive(Debug, Deserialize, ToSchema)]
pub struct TransactionEmissionsRequest {
    /// Transaction ID
    pub txid: String,
    /// Include detailed breakdown
    #[serde(default)]
    pub include_details: bool,
}

/// Register renewable energy request
#[derive(Debug, Deserialize, ToSchema)]
pub struct RenewableEnergyRequest {
    /// Miner address
    pub miner_address: String,
    /// Renewable energy percentage
    pub renewable_percentage: f64,
    /// Energy source
    pub energy_source: String,
    /// Certificate URL
    pub certificate_url: Option<String>,
    /// Certificate issuer
    pub certificate_issuer: Option<String>,
    /// Energy location
    pub energy_location: Option<String>,
    /// Signature
    pub signature: String,
}

/// Create lightning invoice request
#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateInvoiceRequest {
    /// Amount in millisatoshis
    pub amount_msat: Option<u64>,
    /// Description
    pub description: String,
    /// Expiry in seconds
    #[serde(default = "default_invoice_expiry")]
    pub expiry: u32,
    /// Whether invoice is private
    #[serde(default)]
    pub private: bool,
}

fn default_invoice_expiry() -> u32 {
    3600 // 1 hour
}

/// Pay lightning invoice request
#[derive(Debug, Deserialize, ToSchema)]
pub struct PayInvoiceRequest {
    /// Payment request (BOLT11 invoice)
    pub payment_request: String,
    /// Amount in millisatoshis (optional, can override invoice amount)
    pub amount_msat: Option<u64>,
    /// Fee limit in millisatoshis
    #[serde(default = "default_fee_limit")]
    pub fee_limit_msat: u64,
    /// Timeout in seconds
    #[serde(default = "default_payment_timeout")]
    pub timeout_seconds: u32,
    /// Whether to allow sending to self
    #[serde(default)]
    pub allow_self_payment: bool,
}

fn default_fee_limit() -> u64 {
    10000 // 10 satoshis
}

fn default_payment_timeout() -> u32 {
    60 // 1 minute
}

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
    pub private: bool,
    /// Minimum HTLC value in millisatoshis
    #[serde(default = "default_min_htlc")]
    pub min_htlc_msat: u64,
    /// Remote CSV delay
    #[serde(default = "default_remote_csv_delay")]
    pub remote_csv_delay: u16,
    /// Minimum depth
    #[serde(default = "default_min_depth")]
    pub min_depth: u32,
    /// Whether to use post-quantum security
    #[serde(default)]
    pub use_quantum_security: bool,
}

fn default_min_htlc() -> u64 {
    1000 // 1 satoshi
}

fn default_remote_csv_delay() -> u16 {
    144 // ~1 day
}

fn default_min_depth() -> u32 {
    3
}

/// Close channel request
#[derive(Debug, Deserialize, ToSchema)]
pub struct CloseChannelRequest {
    /// Channel ID
    pub channel_id: String,
    /// Whether to force close
    #[serde(default)]
    pub force: bool,
    /// Target confirmation blocks
    #[serde(default = "default_target_conf")]
    pub target_conf: u32,
    /// Whether to wait for confirmation
    #[serde(default)]
    pub wait_for_confirmation: bool,
}

fn default_target_conf() -> u32 {
    6
}

/// Pagination parameters
#[derive(Debug, Deserialize, ToSchema)]
pub struct PaginationParams {
    /// Offset
    #[serde(default)]
    pub offset: usize,
    /// Limit
    #[serde(default = "default_limit")]
    pub limit: usize,
}

fn default_limit() -> usize {
    100
}

/// Time range parameters
#[derive(Debug, Deserialize, ToSchema)]
pub struct TimeRangeParams {
    /// Start time (Unix timestamp)
    pub start_time: Option<u64>,
    /// End time (Unix timestamp)
    pub end_time: Option<u64>,
}

/// Wallet information
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct WalletInfo {
    /// Wallet identifier
    pub wallet_id: String,
    
    /// Type of wallet (HD, Legacy, etc.)
    pub wallet_type: String,
    
    /// Total confirmed balance in satoshis
    pub balance: u64,
    
    /// Unconfirmed balance in satoshis
    pub unconfirmed_balance: u64,
    
    /// Number of addresses in the wallet
    pub address_count: u32,
    
    /// Number of transactions in the wallet
    pub tx_count: u32,
    
    /// Timestamp of last wallet activity
    pub last_active: Option<String>,
    
    /// HD master key fingerprint (if HD wallet)
    pub hd_master_key_fingerprint: Option<String>,
    
    /// Whether the wallet is currently locked
    pub is_locked: bool,
}

/// Wallet balance information
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct BalanceInfo {
    /// Confirmed balance in satoshis
    pub confirmed: u64,
    
    /// Unconfirmed balance in satoshis
    pub unconfirmed: u64,
    
    /// Immature balance in satoshis (from mining)
    pub immature: u64,
    
    /// Total balance in satoshis
    pub total: u64,
    
    /// Spendable balance in satoshis
    pub spendable: u64,
    
    /// Pending mining rewards in satoshis
    pub pending_rewards: u64,
}

/// Wallet address
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct Address {
    /// The address string
    pub address: String,
    
    /// Address type (receive, change)
    pub type_: String,
    
    /// HD derivation path (if HD wallet)
    pub hd_path: Option<String>,
    
    /// Current balance in satoshis
    pub balance: u64,
    
    /// Number of transactions for this address
    pub tx_count: u32,
    
    /// User-defined label
    pub label: Option<String>,
    
    /// Timestamp of last use
    pub last_used: Option<String>,
}

/// Detailed address information
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct AddressInfo {
    /// The address string
    pub address: String,
    
    /// Address type (receive, change)
    pub type_: String,
    
    /// HD derivation path (if HD wallet)
    pub hd_path: Option<String>,
    
    /// Current balance in satoshis
    pub balance: u64,
    
    /// Number of transactions for this address
    pub tx_count: u32,
    
    /// User-defined label
    pub label: Option<String>,
    
    /// Timestamp of last use
    pub last_used: Option<String>,
    
    /// UTXOs associated with this address
    pub utxos: Vec<UTXO>,
}

/// Unspent transaction output (UTXO)
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct UTXO {
    /// Transaction ID
    pub txid: String,
    
    /// Output index
    pub vout: u32,
    
    /// Address
    pub address: String,
    
    /// Amount in satoshis
    pub amount: u64,
    
    /// Number of confirmations
    pub confirmations: u32,
    
    /// Block height
    pub height: Option<u32>,
    
    /// Whether the UTXO is spendable
    pub spendable: bool,
    
    /// Whether the UTXO is safe to spend
    pub safe: bool,
}

/// List of UTXOs
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct UTXOList {
    /// List of UTXOs
    pub utxos: Vec<UTXO>,
    
    /// Total amount in satoshis
    pub total_amount: u64,
}

/// Carbon footprint information
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct CarbonFootprint {
    /// Emissions in grams of CO2 equivalent
    pub emissions_gCO2: f64,
    
    /// Energy consumption in kilowatt-hours
    pub energy_consumption_kWh: f64,
}

/// Transaction information
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct Transaction {
    /// Transaction ID
    pub txid: String,
    
    /// Timestamp
    pub time: String,
    
    /// Amount in satoshis (positive for receive, negative for send)
    pub amount: i64,
    
    /// Fee in satoshis
    pub fee: u64,
    
    /// Number of confirmations
    pub confirmations: u32,
    
    /// Block height
    pub height: Option<u32>,
    
    /// Block hash
    pub blockhash: Option<String>,
    
    /// Transaction category (send, receive, generate, immature, fee)
    pub category: String,
    
    /// Address (primary address for transaction)
    pub address: Option<String>,
    
    /// Label (user-defined)
    pub label: Option<String>,
    
    /// Transaction inputs
    pub inputs: Vec<TransactionInput>,
    
    /// Transaction outputs
    pub outputs: Vec<TransactionOutput>,
    
    /// Carbon footprint of the transaction
    pub carbon_footprint: Option<CarbonFootprint>,
    
    /// Raw transaction hex (if requested)
    pub raw: Option<String>,
}

/// List of transactions
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct TransactionList {
    /// List of transactions
    pub transactions: Vec<Transaction>,
    
    /// Total number of transactions
    pub total_count: u32,
}

/// Request to create a new address
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct AddressRequest {
    /// User-defined label
    pub label: Option<String>,
    
    /// Address type (receive, change)
    #[serde(rename = "type")]
    pub type_: Option<String>,
    
    /// Whether to use quantum-resistant addressing
    pub quantum_resistant: Option<bool>,
}

/// Response for address creation
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct AddressResponse {
    /// The address string
    pub address: String,
    
    /// Address type (receive, change)
    pub type_: String,
    
    /// HD derivation path (if HD wallet)
    pub hd_path: Option<String>,
    
    /// User-defined label
    pub label: Option<String>,
    
    /// Whether it's a quantum-resistant address
    pub quantum_resistant: Option<bool>,
}

/// Transaction output request for sending
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct SendOutput {
    /// Recipient address
    pub address: String,
    
    /// Amount in satoshis
    pub amount: u64,
}

/// Quantum signature options
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct QuantumSignatureOptions {
    /// Signature scheme ("dilithium" or "falcon")
    pub scheme: String,
    
    /// Security strength level ("low", "medium", "high")
    pub strength: String,
}

/// Request to send a transaction
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct SendRequest {
    /// Transaction outputs
    pub outputs: Vec<SendOutput>,
    
    /// Fee rate in satoshis per byte
    pub fee_rate: Option<f64>,
    
    /// Whether to subtract fee from outputs
    pub subtract_fee_from_amount: Option<bool>,
    
    /// Whether transaction is replaceable (RBF)
    pub replaceable: Option<bool>,
    
    /// User-defined comment
    pub comment: Option<String>,
    
    /// Quantum signature options
    pub quantum_signature: Option<QuantumSignatureOptions>,
}

/// Response for sending a transaction
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct SendResponse {
    /// Transaction ID
    pub txid: String,
    
    /// Fee in satoshis
    pub fee: u64,
    
    /// Size in bytes
    pub size: u32,
    
    /// Inputs used
    pub inputs: Vec<TransactionInput>,
    
    /// Outputs created
    pub outputs: Vec<TransactionOutput>,
    
    /// Raw transaction hex
    pub raw_tx: String,
    
    /// Carbon footprint of the transaction
    pub carbon_footprint: Option<CarbonFootprint>,
}

/// Request to sign a message or transaction
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct SignRequest {
    /// Type ("message" or "transaction")
    #[serde(rename = "type")]
    pub type_: String,
    
    /// Data to sign (message text or raw transaction hex)
    pub data: String,
    
    /// Address to sign with (for message signing)
    pub address: Option<String>,
}

/// Response for signing
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct SignResponse {
    /// Signature
    pub signature: String,
    
    /// Address used for signing
    pub address: Option<String>,
    
    /// Type of signature
    #[serde(rename = "type")]
    pub type_: String,
}

/// Request to verify a message signature
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct VerifyRequest {
    /// Message to verify
    pub message: String,
    
    /// Signature to verify
    pub signature: String,
    
    /// Address that created the signature
    pub address: String,
}

/// Response for verification
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct VerifyResponse {
    /// Whether the signature is valid
    pub valid: bool,
    
    /// Address that created the signature
    pub address: String,
}

/// Request to set an address label
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct LabelRequest {
    /// Address to label
    pub address: String,
    
    /// New label
    pub label: String,
}

/// Response for setting an address label
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct LabelResponse {
    /// Address that was labeled
    pub address: String,
    
    /// New label
    pub label: String,
}

/// Response for creating a wallet backup
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct BackupResponse {
    /// Path to the backup file
    pub backup_file: String,
    
    /// Timestamp of backup
    pub timestamp: String,
    
    /// Checksum of backup file
    pub checksum: String,
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