use anyhow::{Context, Result};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::time::Duration;

#[derive(Debug, Clone)]
pub struct RpcClient {
    client: Client,
    url: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RpcRequest {
    jsonrpc: String,
    method: String,
    params: serde_json::Value,
    id: u64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RpcResponse<T> {
    jsonrpc: String,
    result: Option<T>,
    error: Option<RpcError>,
    id: u64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RpcError {
    code: i32,
    message: String,
    data: Option<serde_json::Value>,
}

// Common RPC response types
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct BlockchainInfo {
    pub chain: String,
    pub blocks: u64,
    pub headers: u64,
    pub best_block_hash: String,
    pub difficulty: f64,
    pub verification_progress: f64,
    pub chain_work: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct NodeInfo {
    pub version: String,
    pub protocol_version: u32,
    pub network: String,
    pub connections: u32,
    pub uptime: u64,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PeerInfo {
    pub id: String,
    pub addr: String,
    pub version: String,
    pub services: String,
    pub last_send: u64,
    pub last_recv: u64,
    pub connection_time: u64,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct MempoolInfo {
    pub size: u32,
    pub bytes: u64,
    pub usage: u64,
    pub max_mempool: u64,
    pub mempool_min_fee: f64,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct TransactionInfo {
    pub txid: String,
    pub size: u32,
    pub version: u32,
    pub locktime: u32,
    pub confirmations: u32,
    pub block_hash: Option<String>,
    pub time: Option<u64>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AddressBalance {
    pub address: String,
    pub balance: f64,
    pub confirmed: f64,
    pub unconfirmed: f64,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct MiningInfo {
    pub blocks: u64,
    pub difficulty: f64,
    pub network_hashrate: f64,
    pub mining_enabled: bool,
    pub threads: u32,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct EnvironmentalMetrics {
    pub carbon_footprint: f64,
    pub renewable_percentage: f64,
    pub green_miners: u32,
    pub carbon_credits_earned: f64,
}

impl RpcClient {
    pub fn new(url: String, timeout: u64) -> Result<Self> {
        let client = Client::builder()
            .timeout(Duration::from_secs(timeout))
            .build()
            .context("Failed to create HTTP client")?;
        
        Ok(Self { client, url })
    }
    
    async fn call<T>(&self, method: &str, params: serde_json::Value) -> Result<T>
    where
        T: for<'de> Deserialize<'de>,
    {
        let request = RpcRequest {
            jsonrpc: "2.0".to_string(),
            method: method.to_string(),
            params,
            id: 1,
        };
        
        let response = self.client
            .post(&self.url)
            .json(&request)
            .send()
            .await
            .context("Failed to send RPC request")?;
        
        let rpc_response: RpcResponse<T> = response
            .json()
            .await
            .context("Failed to parse RPC response")?;
        
        if let Some(error) = rpc_response.error {
            anyhow::bail!("RPC error {}: {}", error.code, error.message);
        }
        
        rpc_response.result
            .context("Empty RPC response")
    }
    
    // Blockchain methods
    pub async fn get_blockchain_info(&self) -> Result<BlockchainInfo> {
        self.call("getblockchaininfo", json!([])).await
    }
    
    pub async fn get_node_info(&self) -> Result<NodeInfo> {
        self.call("getinfo", json!([])).await
    }
    
    pub async fn get_peer_info(&self) -> Result<Vec<PeerInfo>> {
        self.call("getpeerinfo", json!([])).await
    }
    
    pub async fn get_mempool_info(&self) -> Result<MempoolInfo> {
        self.call("getmempoolinfo", json!([])).await
    }
    
    pub async fn get_mining_info(&self) -> Result<MiningInfo> {
        self.call("getmininginfo", json!([])).await
    }
    
    pub async fn get_environmental_metrics(&self) -> Result<EnvironmentalMetrics> {
        self.call("getenvironmentalmetrics", json!([])).await
    }
    
    // Address methods
    pub async fn get_balance(&self, address: &str) -> Result<AddressBalance> {
        self.call("getaddressbalance", json!([address])).await
    }
    
    pub async fn get_new_address(&self) -> Result<String> {
        self.call("getnewaddress", json!([])).await
    }
    
    // Transaction methods
    pub async fn send_transaction(&self, to: &str, amount: f64) -> Result<String> {
        self.call("sendtoaddress", json!([to, amount])).await
    }
    
    pub async fn get_transaction(&self, txid: &str) -> Result<TransactionInfo> {
        self.call("gettransaction", json!([txid])).await
    }
    
    // Mining methods
    pub async fn start_mining(&self, threads: u32) -> Result<bool> {
        self.call("setgenerate", json!([true, threads])).await
    }
    
    pub async fn stop_mining(&self) -> Result<bool> {
        self.call("setgenerate", json!([false])).await
    }
    
    // Utility methods
    pub async fn validate_address(&self, address: &str) -> Result<bool> {
        #[derive(Deserialize)]
        struct ValidateResult {
            isvalid: bool,
        }
        
        let result: ValidateResult = self.call("validateaddress", json!([address])).await?;
        Ok(result.isvalid)
    }
    
    pub async fn ping(&self) -> Result<()> {
        self.call::<serde_json::Value>("ping", json!([])).await?;
        Ok(())
    }
} 