use utoipa::openapi::security::{ApiKey, ApiKeyValue, SecurityScheme};
use utoipa::{Modify, OpenApi};
use crate::api::types;
use crate::api::routes::{
    blockchain,
    mempool,
    network,
    mining,
    environmental,
    lightning,
    node,
    faucet,
};

/// API security schema modification
struct SecurityAddon;

impl Modify for SecurityAddon {
    fn modify(&self, openapi: &mut utoipa::openapi::OpenApi) {
        // Add JWT bearer token security scheme
        if let Some(components) = openapi.components.as_mut() {
            components.add_security_scheme(
                "jwt_auth",
                SecurityScheme::ApiKey(ApiKey::Header(ApiKeyValue::new("Authorization"))),
            );
        }
    }
}

/// Generate the OpenAPI documentation for the API
#[derive(OpenApi)]
#[openapi(
    paths(
        // Blockchain routes
        blockchain::get_blockchain_info,
        blockchain::get_block_by_height,
        blockchain::get_block_by_hash,
        blockchain::get_transaction,
        blockchain::submit_transaction,
        
        // Mempool routes
        mempool::get_mempool_info,
        mempool::get_mempool_transactions,
        mempool::get_mempool_transaction,
        mempool::submit_transaction,
        mempool::validate_transaction,
        mempool::get_fee_estimates,
        
        // Network routes
        network::get_network_info,
        network::get_connection_count,
        network::get_peers,
        network::get_peer,
        network::add_peer,
        network::remove_peer,
        network::get_bandwidth_usage,
        
        // Mining routes
        mining::get_mining_info,
        mining::get_mining_template,
        mining::submit_block,
        mining::get_mining_stats,
        mining::get_mining_status,
        mining::start_mining,
        mining::stop_mining,
        mining::get_mining_config,
        mining::update_mining_config,
        
        // Environmental routes
        environmental::get_environmental_impact,
        environmental::get_energy_usage,
        environmental::get_carbon_footprint,
        environmental::get_resource_utilization,
        environmental::get_environmental_settings,
        environmental::update_environmental_settings,
        
        // Lightning routes
        lightning::get_lightning_info,
        lightning::get_channels,
        lightning::get_channel,
        lightning::open_channel,
        lightning::close_channel,
        lightning::get_payments,
        lightning::send_payment,
        lightning::get_invoices,
        lightning::create_invoice,
        lightning::get_network_nodes,
        lightning::get_node_info,
        lightning::find_route,
        
        // Node routes
        node::get_node_info,
        node::get_system_info,
        node::get_logs,
        node::get_node_status,
        node::get_version,
        node::get_metrics,
        node::get_config,
        node::update_config,
        node::create_backup,
        node::get_backup_info,
        node::restart_node,
        node::shutdown_node,
        node::get_debug_info,
        
        // Faucet routes
        faucet::get_faucet_status,
        faucet::request_tokens,
        faucet::get_recent_transactions,
    ),
    components(
        schemas(
            // Blockchain types
            types::BlockchainInfo,
            types::Block,
            types::Transaction,
            types::TransactionInput,
            types::TransactionOutput,
            types::BlockHeader,
            types::TransactionSubmissionResponse,
            
            // Mempool types
            types::MempoolInfo,
            types::MempoolStatistics,
            types::MempoolTransaction,
            types::MempoolTransactionSubmissionResponse,
            types::TransactionValidationResult,
            types::TransactionFees,
            mempool::SubmitTransactionRequest,
            mempool::ValidateTransactionRequest,
            
            // Network types
            types::NetworkInfo,
            types::PeerInfo,
            types::PeerConnectionStatus,
            types::BandwidthUsage,
            types::PeerAddRequest,
            types::PeerAddResponse,
            types::NodeAddress,
            types::ConnectionCount,
            
            // Mining types
            types::MiningInfo,
            types::MiningTemplate,
            types::MiningStats,
            types::SubmitBlockRequest,
            types::SubmitBlockResponse,
            types::MiningStatus,
            types::MiningConfiguration,
            mining::StartMiningRequest,
            
            // Environmental types
            types::EnvironmentalImpact,
            types::EnergyUsage,
            types::CarbonFootprint,
            types::EnvironmentalSettings,
            types::ResourceUtilization,
            
            // Lightning types
            types::LightningInfo,
            types::LightningChannel,
            types::LightningPayment,
            types::LightningInvoice,
            types::OpenChannelRequest,
            types::OpenChannelResponse,
            types::CloseChannelRequest,
            types::PaymentRequest,
            types::PaymentResponse,
            types::InvoiceRequest,
            types::InvoiceResponse,
            types::NodeInfo,
            types::Route,
            
            // Node types
            types::NodeInfo,
            types::SystemInfo,
            types::LogEntry,
            types::NodeStatus,
            types::VersionInfo,
            types::BackupInfo,
            types::NodeMetrics,
            types::DebugInfo,
            node::BackupRequest,
            
            // Faucet types
            faucet::FaucetStatusResponse,
            faucet::FaucetRequest,
            faucet::FaucetResponse,
            faucet::FaucetTransaction,
            faucet::RecentTransactionsResponse,
            
            // Error types
            types::ErrorResponse,
        )
    ),
    modifiers(&SecurityAddon),
    tags(
        (name = "blockchain", description = "Blockchain API endpoints"),
        (name = "mempool", description = "Mempool API endpoints"),
        (name = "network", description = "Network API endpoints"),
        (name = "mining", description = "Mining API endpoints"),
        (name = "environmental", description = "Environmental monitoring API endpoints"),
        (name = "lightning", description = "Lightning Network API endpoints"),
        (name = "node", description = "Node management API endpoints"),
        (name = "faucet", description = "Testnet faucet API endpoints"),
    ),
    info(
        title = "Supernova Node API",
        version = "0.1.0",
        description = "API for interacting with the Supernova blockchain node",
        contact(
            name = "Supernova Team",
            email = "support@supernova.network",
            url = "https://supernovanetwork.xyz"
        ),
        license(
            name = "MIT",
            url = "https://opensource.org/licenses/MIT"
        )
    )
)]
pub struct ApiDoc;

/// Initialize the OpenAPI documentation
pub fn init() -> utoipa::openapi::OpenApi {
    ApiDoc::openapi()
} 