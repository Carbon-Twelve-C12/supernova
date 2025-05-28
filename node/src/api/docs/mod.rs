//! API documentation module
//!
//! This module contains the OpenAPI documentation for the API and related utilities.

pub mod openapi;

pub use openapi::init as init_openapi;

use utoipa::OpenApi;
use super::types;

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
        crate::api::routes::mempool::submit_transaction,
        crate::api::routes::mempool::validate_transaction,
        crate::api::routes::mempool::get_fee_estimates,
        
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
            types::ApiResponse<String>,
            
            // Blockchain
            types::BlockInfo,
            types::TransactionInfo,
            types::TransactionInput,
            types::TransactionOutput,
            types::BlockchainInfo,
            types::TransactionSubmissionResponse,
            
            // Mempool
            types::MempoolInfo,
            types::MempoolStatistics,
            types::MempoolTransaction,
            types::MempoolTransactionSubmissionResponse,
            types::TransactionValidationResult,
            types::TransactionFees,
            crate::api::routes::mempool::SubmitTxRequest,
            
            // Network
            types::NetworkInfo,
            types::NetworkAddress,
            types::NetworkStats,
            types::PeerInfo,
            types::PeerConnectionStatus,
            types::BandwidthUsage,
            types::PeerAddRequest,
            types::PeerAddResponse,
            types::NodeAddress,
            types::ConnectionCount,
            
            // Mining
            types::MiningInfo,
            types::MiningTemplate,
            types::MiningStats,
            types::SubmitBlockRequest,
            types::SubmitBlockResponse,
            types::MiningStatus,
            types::MiningConfiguration,
            crate::api::routes::mining::StartMiningRequest,
            
            // Environmental
            types::EnvironmentalImpact,
            types::EnergyUsage,
            types::CarbonFootprint,
            types::EnvironmentalSettings,
            types::ResourceUtilization,
            
            // Lightning Network
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
            
            // Node
            types::SystemInfo,
            types::LogEntry,
            types::NodeStatus,
            types::NodeVersion,
            types::NodeConfiguration,
            types::BackupInfo,
            types::NodeMetrics,
            types::DebugInfo,
            crate::api::routes::node::CreateBackupRequest,
            
            // Request parameters
            types::BlockHeightParams,
            types::BlockHashParams,
            types::TxHashParams,
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
            url = "https://supernovanetwork.xyz",
            email = "dev@supernovanetwork.xyz"
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

/// Add JSON-RPC documentation
pub mod jsonrpc {
    /// JSON-RPC documentation
    pub struct JsonRpcDoc;
    
    impl JsonRpcDoc {
        /// Generate JSON-RPC documentation
        pub fn generate() -> String {
            let doc = r#"
# JSON-RPC API Reference

SuperNova provides a JSON-RPC 2.0 compatible API that follows the Bitcoin Core JSON-RPC specification.

## Endpoint

The JSON-RPC API is available at `/rpc` by default.

## Request Format

```json
{
  "jsonrpc": "2.0",
  "id": "request-id",
  "method": "method-name",
  "params": {}
}
```

## Response Format

```json
{
  "jsonrpc": "2.0",
  "id": "request-id",
  "result": {}
}
```

Or in case of an error:

```json
{
  "jsonrpc": "2.0",
  "id": "request-id",
  "error": {
    "code": -32000,
    "message": "Error message"
  }
}
```

## Available Methods

### Blockchain Methods

- `getblockchaininfo`: Get blockchain information
- `getblock`: Get block by hash
- `getblockhash`: Get block hash by height
- `getbestblockhash`: Get the hash of the best (tip) block
- `getblockcount`: Get the current block count
- `getdifficulty`: Get the proof-of-work difficulty

### Transaction Methods

- `gettransaction`: Get transaction information
- `getrawtransaction`: Get raw transaction data
- `sendrawtransaction`: Send raw transaction

### Mempool Methods

- `getmempoolinfo`: Get mempool information
- `getrawmempool`: Get raw mempool transactions

### Network Methods

- `getnetworkinfo`: Get network information
- `getpeerinfo`: Get peer information

### Mining Methods

- `getmininginfo`: Get mining information
- `getblocktemplate`: Get block template for mining
- `submitblock`: Submit a mined block

## Error Codes

- `-32700`: Parse error - Invalid JSON was received
- `-32600`: Invalid Request - The JSON sent is not a valid Request object
- `-32601`: Method not found - The method does not exist / is not available
- `-32602`: Invalid params - Invalid method parameter(s)
- `-32603`: Internal error - Internal JSON-RPC error
- `-32000`: Server error - Generic server error
- `-32001`: Node syncing - Node is still syncing with the network
- `-32002`: Blockchain error - Error in blockchain operations
- `-32003`: Transaction error - Error in transaction processing
- `-32004`: Wallet error - Error in wallet operations
- `-32005`: Network error - Error in network operations
            "#;
            
            doc.to_string()
        }
    }
} 