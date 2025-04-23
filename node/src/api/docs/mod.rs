//! API documentation module
//!
//! This module contains the OpenAPI documentation for the API and related utilities.

pub mod openapi;

pub use openapi::init as init_openapi;

use utoipa::OpenApi;

/// Generate OpenAPI documentation
#[derive(OpenApi)]
#[openapi(
    paths(
        // Blockchain routes
        crate::api::routes::blockchain::get_blockchain_info,
        crate::api::routes::blockchain::get_block_by_height,
        crate::api::routes::blockchain::get_block_by_hash,
        crate::api::routes::blockchain::get_transaction,
        crate::api::routes::blockchain::submit_transaction,
        
        // Mempool routes
        crate::api::routes::mempool::get_mempool_info,
        crate::api::routes::mempool::get_mempool_transactions,
        crate::api::routes::mempool::get_mempool_transaction,
        crate::api::routes::mempool::submit_mempool_transaction,
        crate::api::routes::mempool::validate_transaction,
        crate::api::routes::mempool::estimate_fee,
        
        // Network routes
        crate::api::routes::network::get_network_info,
        crate::api::routes::network::get_connection_count,
        crate::api::routes::network::get_peers,
        crate::api::routes::network::get_peer,
        crate::api::routes::network::add_peer,
        crate::api::routes::network::remove_peer,
        crate::api::routes::network::get_bandwidth_usage,
        
        // Mining routes
        crate::api::routes::mining::get_mining_info,
        crate::api::routes::mining::get_mining_template,
        crate::api::routes::mining::submit_block,
        crate::api::routes::mining::get_mining_stats,
        crate::api::routes::mining::get_mining_status,
        crate::api::routes::mining::start_mining,
        crate::api::routes::mining::stop_mining,
        crate::api::routes::mining::get_mining_config,
        crate::api::routes::mining::update_mining_config,
        
        // Environmental routes
        crate::api::routes::environmental::get_environmental_impact,
        crate::api::routes::environmental::get_energy_usage,
        crate::api::routes::environmental::get_carbon_footprint,
        crate::api::routes::environmental::get_resource_utilization,
        crate::api::routes::environmental::get_environmental_settings,
        crate::api::routes::environmental::update_environmental_settings,
        
        // Lightning routes
        crate::api::routes::lightning::get_lightning_info,
        crate::api::routes::lightning::get_channels,
        crate::api::routes::lightning::get_channel,
        crate::api::routes::lightning::open_channel,
        crate::api::routes::lightning::close_channel,
        crate::api::routes::lightning::get_payments,
        crate::api::routes::lightning::send_payment,
        crate::api::routes::lightning::get_invoices,
        crate::api::routes::lightning::create_invoice,
        crate::api::routes::lightning::get_network_nodes,
        crate::api::routes::lightning::get_node_info,
        crate::api::routes::lightning::find_route,
        
        // Node routes
        crate::api::routes::node::get_node_info,
        crate::api::routes::node::get_system_info,
        crate::api::routes::node::get_logs,
        crate::api::routes::node::get_node_status,
        crate::api::routes::node::get_node_version,
        crate::api::routes::node::get_node_metrics,
        crate::api::routes::node::get_node_config,
        crate::api::routes::node::update_node_config,
        crate::api::routes::node::create_backup,
        crate::api::routes::node::get_backup_info,
        crate::api::routes::node::restart_node,
        crate::api::routes::node::shutdown_node,
        crate::api::routes::node::get_debug_info,
    ),
    components(
        schemas(
            // API response
            crate::api::types::ApiResponse<String>,
            
            // Blockchain
            crate::api::types::BlockInfo,
            crate::api::types::TransactionInfo,
            crate::api::types::TransactionInput,
            crate::api::types::TransactionOutput,
            crate::api::types::BlockchainInfo,
            crate::api::types::Block,
            crate::api::types::Transaction,
            crate::api::types::BlockHeader,
            crate::api::types::TransactionSubmissionResponse,
            
            // Mempool
            crate::api::types::MempoolInfo,
            crate::api::types::MempoolStatistics,
            crate::api::types::MempoolTransaction,
            crate::api::types::MempoolTransactionSubmissionResponse,
            crate::api::types::TransactionValidationResult,
            crate::api::types::TransactionFees,
            crate::api::routes::mempool::SubmitTransactionRequest,
            crate::api::routes::mempool::ValidateTransactionRequest,
            
            // Network
            crate::api::types::NetworkInfo,
            crate::api::types::NetworkAddress,
            crate::api::types::NetworkStats,
            crate::api::types::PeerInfo,
            crate::api::types::PeerConnectionStatus,
            crate::api::types::BandwidthUsage,
            crate::api::types::PeerAddRequest,
            crate::api::types::PeerAddResponse,
            crate::api::types::NodeAddress,
            crate::api::types::ConnectionCount,
            
            // Mining
            crate::api::types::MiningInfo,
            crate::api::types::MiningTemplate,
            crate::api::types::MiningStats,
            crate::api::types::SubmitBlockRequest,
            crate::api::types::SubmitBlockResponse,
            crate::api::types::MiningStatus,
            crate::api::types::MiningConfiguration,
            crate::api::routes::mining::StartMiningRequest,
            
            // Environmental
            crate::api::types::EnvironmentalImpact,
            crate::api::types::EnergyUsage,
            crate::api::types::CarbonFootprint,
            crate::api::types::EnvironmentalSettings,
            crate::api::types::ResourceUtilization,
            
            // Lightning Network
            crate::api::types::LightningInfo,
            crate::api::types::LightningChannel,
            crate::api::types::LightningPayment,
            crate::api::types::LightningInvoice,
            crate::api::types::OpenChannelRequest,
            crate::api::types::OpenChannelResponse,
            crate::api::types::CloseChannelRequest,
            crate::api::types::PaymentRequest,
            crate::api::types::PaymentResponse,
            crate::api::types::InvoiceRequest,
            crate::api::types::InvoiceResponse,
            crate::api::types::NodeInfo,
            crate::api::types::Route,
            
            // Node
            crate::api::types::SystemInfo,
            crate::api::types::LogEntry,
            crate::api::types::NodeStatus,
            crate::api::types::NodeVersion,
            crate::api::types::NodeConfiguration,
            crate::api::types::BackupInfo,
            crate::api::types::NodeMetrics,
            crate::api::types::DebugInfo,
            crate::api::routes::node::CreateBackupRequest,
            
            // Request parameters
            crate::api::types::BlockHeightParams,
            crate::api::types::BlockHashParams,
            crate::api::types::TxHashParams,
            crate::api::types::AddressParams,
            crate::api::types::SubmitTxRequest,
            crate::api::types::PaginationParams,
            crate::api::types::TimeRangeParams,
        )
    ),
    tags(
        (name = "blockchain", description = "Blockchain API - Access blocks and transactions"),
        (name = "mempool", description = "Mempool API - View and manage pending transactions"),
        (name = "network", description = "Network API - P2P network information and management"),
        (name = "mining", description = "Mining API - Mining operations and block templates"),
        (name = "environmental", description = "Environmental API - Carbon emissions and energy tracking"),
        (name = "lightning", description = "Lightning Network API - Payment channels and off-chain payments"),
        (name = "node", description = "Node API - Node status, configuration, and operations"),
    ),
    info(
        title = "SuperNova Blockchain API",
        version = "1.0.0",
        description = "RESTful API for SuperNova blockchain node with support for environmental impact tracking and the Lightning Network",
        license(
            name = "MIT",
            url = "https://opensource.org/licenses/MIT"
        ),
        contact(
            name = "SuperNova Developer Team",
            url = "https://supernova.io",
            email = "dev@supernova.io"
        )
    )
)]
pub struct ApiDoc;

/// OpenAPI error response example
const ERROR_RESPONSE_EXAMPLE: &str = r#"
{
  "success": false,
  "error": "Resource not found"
}
"#;

/// OpenAPI success response example
const SUCCESS_RESPONSE_EXAMPLE: &str = r#"
{
  "success": true,
  "data": {}
}
"#; 