// SuperNova Lightning Network - Router Implementation
//
// This file contains the implementation of the Lightning Network router,
// which handles finding payment paths and routing payments through the network.

use crate::lightning::channel::{ChannelId, ChannelState};
use std::collections::{HashMap, HashSet, BTreeMap};
use std::cmp::Ordering;
use std::sync::{Arc, RwLock, Mutex};
use thiserror::Error;
use tracing::{debug, info, warn, error};
use std::time::{Duration, Instant};
use priority_queue::PriorityQueue;

/// Error types for routing operations
#[derive(Debug, Error)]
pub enum RoutingError {
    #[error("No route found")]
    NoRouteFound,
    
    #[error("Insufficient capacity: {0}")]
    InsufficientCapacity(String),
    
    #[error("Graph error: {0}")]
    GraphError(String),
    
    #[error("Invalid destination: {0}")]
    InvalidDestination(String),
    
    #[error("Timeout error: {0}")]
    Timeout(String),
    
    #[error("Routing constraint error: {0}")]
    ConstraintError(String),
}

/// A node in the Lightning Network
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct NodeId(String);

impl NodeId {
    /// Create a new node ID
    pub fn new(id: String) -> Self {
        Self(id)
    }
    
    /// Get the node ID as a string
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Information about a channel in the network
#[derive(Debug, Clone)]
pub struct ChannelInfo {
    /// Channel ID
    pub channel_id: ChannelId,
    
    /// Source node
    pub source: NodeId,
    
    /// Destination node
    pub destination: NodeId,
    
    /// Channel capacity in satoshis
    pub capacity: u64,
    
    /// Base fee in millisatoshis
    pub base_fee_msat: u32,
    
    /// Fee rate in parts per million
    pub fee_rate_millionths: u32,
    
    /// CLTV expiry delta
    pub cltv_expiry_delta: u16,
    
    /// Whether the channel is active
    pub is_active: bool,
    
    /// Last update timestamp
    pub last_update: u64,
}

/// A hint for routing through a private channel
#[derive(Debug, Clone)]
pub struct RouteHint {
    /// Node ID
    pub node_id: NodeId,
    
    /// Channel ID
    pub channel_id: ChannelId,
    
    /// Base fee in millisatoshis
    pub base_fee_msat: u32,
    
    /// Fee rate in parts per million
    pub fee_rate_millionths: u32,
    
    /// CLTV expiry delta
    pub cltv_expiry_delta: u16,
}

/// A hop in a payment path
#[derive(Debug, Clone)]
pub struct PathHop {
    /// Node ID
    pub node_id: NodeId,
    
    /// Channel ID
    pub channel_id: ChannelId,
    
    /// Amount to forward in millisatoshis
    pub amount_msat: u64,
    
    /// CLTV expiry
    pub cltv_expiry: u32,
}

/// A complete payment path
#[derive(Debug, Clone)]
pub struct PaymentPath {
    /// Hops in the path
    pub hops: Vec<PathHop>,
    
    /// Total fee in millisatoshis
    pub total_fee_msat: u64,
    
    /// Total CLTV delta
    pub total_cltv_delta: u32,
    
    /// Total amount to send including fees
    pub total_amount_msat: u64,
}

impl PaymentPath {
    /// Create a new payment path
    pub fn new() -> Self {
        Self {
            hops: Vec::new(),
            total_fee_msat: 0,
            total_cltv_delta: 0,
            total_amount_msat: 0,
        }
    }
    
    /// Check if the path is empty
    pub fn is_empty(&self) -> bool {
        self.hops.is_empty()
    }
    
    /// Get the number of hops
    pub fn len(&self) -> usize {
        self.hops.len()
    }
    
    /// Add a hop to the path
    pub fn add_hop(&mut self, hop: PathHop) {
        self.hops.push(hop);
    }
}

/// Router preferences for path finding
#[derive(Debug, Clone)]
pub struct RouterPreferences {
    /// Maximum number of hops
    pub max_hops: u8,
    
    /// Maximum CLTV expiry delta
    pub max_cltv_expiry_delta: u16,
    
    /// Maximum fee rate in parts per million
    pub max_fee_rate_millionths: u32,
    
    /// Timeout for path finding
    pub path_finding_timeout_ms: u64,
    
    /// Whether to use private channels
    pub use_private_channels: bool,
    
    /// Preferred nodes to route through
    pub preferred_nodes: HashSet<NodeId>,
    
    /// Nodes to avoid
    pub avoid_nodes: HashSet<NodeId>,
    
    /// Channels to avoid
    pub avoid_channels: HashSet<ChannelId>,
}

impl Default for RouterPreferences {
    fn default() -> Self {
        Self {
            max_hops: 20,
            max_cltv_expiry_delta: 1440,  // Max 24 hours (assuming 10 min blocks)
            max_fee_rate_millionths: 5000, // Max 0.5% fee rate
            path_finding_timeout_ms: 5000, // 5 second timeout
            use_private_channels: true,
            preferred_nodes: HashSet::new(),
            avoid_nodes: HashSet::new(),
            avoid_channels: HashSet::new(),
        }
    }
}

/// Score function for channel ranking
#[derive(Debug, Clone)]
pub enum ScoringFunction {
    /// Score based on success probability
    SuccessProbability,
    
    /// Score based on fees
    LowestFee,
    
    /// Score based on path length
    ShortestPath,
    
    /// Custom scoring function
    Custom(fn(&ChannelInfo) -> u64),
}

/// Channel scorer for ranking channels
pub struct ChannelScorer {
    /// Scoring function
    scoring_function: ScoringFunction,
    
    /// Success probability by channel
    success_probability: HashMap<ChannelId, f64>,
    
    /// Historical data by channel
    historical_data: HashMap<ChannelId, ChannelHistoricalData>,
}

/// Historical data for a channel
#[derive(Debug, Clone)]
struct ChannelHistoricalData {
    /// Number of successful payments
    successful_payments: u64,
    
    /// Number of failed payments
    failed_payments: u64,
    
    /// Average time to complete payment
    average_time_ms: u64,
    
    /// Last success time
    last_success: Option<u64>,
}

impl ChannelScorer {
    /// Create a new channel scorer
    pub fn new(scoring_function: ScoringFunction) -> Self {
        Self {
            scoring_function,
            success_probability: HashMap::new(),
            historical_data: HashMap::new(),
        }
    }
    
    /// Score a channel
    pub fn score_channel(&self, channel: &ChannelInfo) -> u64 {
        match &self.scoring_function {
            ScoringFunction::SuccessProbability => {
                // Score based on success probability
                let prob = self.success_probability.get(&channel.channel_id)
                    .cloned()
                    .unwrap_or(0.5); // Default 50% if no data
                
                (prob * 1_000_000.0) as u64
            },
            ScoringFunction::LowestFee => {
                // Score inversely proportional to fees
                let fee_score = 1_000_000 - channel.base_fee_msat as u64 - channel.fee_rate_millionths as u64;
                std::cmp::max(1, fee_score) // Ensure score is at least 1
            },
            ScoringFunction::ShortestPath => {
                // All channels get the same score
                1
            },
            ScoringFunction::Custom(func) => {
                func(channel)
            },
        }
    }
    
    /// Update success probability for a channel
    pub fn update_success_probability(&mut self, channel_id: &ChannelId, success: bool) {
        let data = self.historical_data.entry(channel_id.clone())
            .or_insert_with(|| ChannelHistoricalData {
                successful_payments: 0,
                failed_payments: 0,
                average_time_ms: 0,
                last_success: None,
            });
        
        if success {
            data.successful_payments += 1;
            data.last_success = Some(std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or(Duration::from_secs(0))
                .as_secs());
        } else {
            data.failed_payments += 1;
        }
        
        // Update success probability
        let total = data.successful_payments + data.failed_payments;
        if total > 0 {
            let prob = data.successful_payments as f64 / total as f64;
            self.success_probability.insert(channel_id.clone(), prob);
        }
    }
}

/// Network graph representing the Lightning Network
pub struct NetworkGraph {
    /// Nodes in the network
    nodes: HashMap<NodeId, Vec<ChannelId>>,
    
    /// Channels in the network
    channels: HashMap<ChannelId, ChannelInfo>,
    
    /// Private channels not announced to the network
    private_channels: HashMap<ChannelId, ChannelInfo>,
    
    /// Last update time
    last_update: u64,
}

impl NetworkGraph {
    /// Create a new network graph
    pub fn new() -> Self {
        Self {
            nodes: HashMap::new(),
            channels: HashMap::new(),
            private_channels: HashMap::new(),
            last_update: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or(Duration::from_secs(0))
                .as_secs(),
        }
    }
    
    /// Add a node to the graph
    pub fn add_node(&mut self, node_id: NodeId) {
        if !self.nodes.contains_key(&node_id) {
            self.nodes.insert(node_id, Vec::new());
        }
    }
    
    /// Add a channel to the graph
    pub fn add_channel(&mut self, channel: ChannelInfo, is_private: bool) {
        // Make sure nodes exist
        self.add_node(channel.source.clone());
        self.add_node(channel.destination.clone());
        
        // Add channel to source node's channel list
        if let Some(channels) = self.nodes.get_mut(&channel.source) {
            if !channels.contains(&channel.channel_id) {
                channels.push(channel.channel_id.clone());
            }
        }
        
        // Add channel to destination node's channel list
        if let Some(channels) = self.nodes.get_mut(&channel.destination) {
            if !channels.contains(&channel.channel_id) {
                channels.push(channel.channel_id.clone());
            }
        }
        
        // Store channel
        if is_private {
            self.private_channels.insert(channel.channel_id.clone(), channel);
        } else {
            self.channels.insert(channel.channel_id.clone(), channel);
        }
        
        // Update last update time
        self.last_update = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or(Duration::from_secs(0))
            .as_secs();
    }
    
    /// Remove a channel from the graph
    pub fn remove_channel(&mut self, channel_id: &ChannelId) {
        // Remove from public channels
        if let Some(channel) = self.channels.remove(channel_id) {
            // Remove from source node's channel list
            if let Some(channels) = self.nodes.get_mut(&channel.source) {
                channels.retain(|id| id != channel_id);
            }
            
            // Remove from destination node's channel list
            if let Some(channels) = self.nodes.get_mut(&channel.destination) {
                channels.retain(|id| id != channel_id);
            }
        }
        
        // Remove from private channels
        if let Some(channel) = self.private_channels.remove(channel_id) {
            // Remove from source node's channel list
            if let Some(channels) = self.nodes.get_mut(&channel.source) {
                channels.retain(|id| id != channel_id);
            }
            
            // Remove from destination node's channel list
            if let Some(channels) = self.nodes.get_mut(&channel.destination) {
                channels.retain(|id| id != channel_id);
            }
        }
        
        // Update last update time
        self.last_update = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or(Duration::from_secs(0))
            .as_secs();
    }
    
    /// Get a channel by ID
    pub fn get_channel(&self, channel_id: &ChannelId, include_private: bool) -> Option<&ChannelInfo> {
        // First check public channels
        if let Some(channel) = self.channels.get(channel_id) {
            return Some(channel);
        }
        
        // Then check private channels if allowed
        if include_private {
            return self.private_channels.get(channel_id);
        }
        
        None
    }
    
    /// Get all channels for a node
    pub fn get_node_channels(&self, node_id: &NodeId, include_private: bool) -> Vec<&ChannelInfo> {
        let mut result = Vec::new();
        
        if let Some(channel_ids) = self.nodes.get(node_id) {
            for channel_id in channel_ids {
                if let Some(channel) = self.get_channel(channel_id, include_private) {
                    result.push(channel);
                }
            }
        }
        
        result
    }
    
    /// Get the number of nodes in the graph
    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }
    
    /// Get the number of channels in the graph
    pub fn channel_count(&self, include_private: bool) -> usize {
        let mut count = self.channels.len();
        if include_private {
            count += self.private_channels.len();
        }
        count
    }
}

/// Main router implementation
pub struct Router {
    /// Network graph
    graph: NetworkGraph,
    
    /// Routing preferences
    preferences: RouterPreferences,
    
    /// Channel scorer
    scorer: ChannelScorer,
    
    /// Local node ID
    local_node: NodeId,
}

impl Router {
    /// Create a new router
    pub fn new() -> Self {
        Self {
            graph: NetworkGraph::new(),
            preferences: RouterPreferences::default(),
            scorer: ChannelScorer::new(ScoringFunction::LowestFee),
            local_node: NodeId::new("".to_string()), // Will be set later
        }
    }
    
    /// Set the local node ID
    pub fn set_local_node(&mut self, node_id: NodeId) {
        self.local_node = node_id;
    }
    
    /// Set routing preferences
    pub fn set_preferences(&mut self, preferences: RouterPreferences) {
        self.preferences = preferences;
    }
    
    /// Find a route to a destination
    pub fn find_route(
        &self,
        destination: &str,
        amount_msat: u64,
        route_hints: &[RouteHint],
    ) -> Result<PaymentPath, RoutingError> {
        let destination_id = NodeId::new(destination.to_string());
        
        // Check if destination exists in the graph
        if !self.graph.nodes.contains_key(&destination_id) {
            return Err(RoutingError::InvalidDestination(
                format!("Destination node {} not found in graph", destination)
            ));
        }
        
        // Set up timeout
        let timeout = Duration::from_millis(self.preferences.path_finding_timeout_ms);
        let start_time = Instant::now();
        
        // Add route hints to the graph temporarily
        let mut temp_graph = self.graph.clone();
        for hint in route_hints {
            let channel_info = ChannelInfo {
                channel_id: hint.channel_id.clone(),
                source: hint.node_id.clone(),
                destination: destination_id.clone(),
                capacity: amount_msat / 1000, // Assume sufficient capacity
                base_fee_msat: hint.base_fee_msat,
                fee_rate_millionths: hint.fee_rate_millionths,
                cltv_expiry_delta: hint.cltv_expiry_delta,
                is_active: true,
                last_update: std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or(Duration::from_secs(0))
                    .as_secs(),
            };
            
            temp_graph.add_channel(channel_info, true);
        }
        
        // Use Dijkstra's algorithm to find the shortest path
        let path = self.find_shortest_path(
            &temp_graph,
            &self.local_node,
            &destination_id,
            amount_msat,
            &start_time,
            &timeout,
        )?;
        
        Ok(path)
    }
    
    /// Find the shortest path using Dijkstra's algorithm
    fn find_shortest_path(
        &self,
        graph: &NetworkGraph,
        source: &NodeId,
        destination: &NodeId,
        amount_msat: u64,
        start_time: &Instant,
        timeout: &Duration,
    ) -> Result<PaymentPath, RoutingError> {
        // This is a simplified implementation
        // A real implementation would use Dijkstra's algorithm or similar
        
        // For the MVP, we'll just create a direct path if possible
        let mut path = PaymentPath::new();
        
        // Find a channel between source and destination
        let channels = graph.get_node_channels(source, self.preferences.use_private_channels);
        
        for channel in channels {
            // Check if we've timed out
            if start_time.elapsed() > *timeout {
                return Err(RoutingError::Timeout(
                    format!("Path finding timed out after {} ms", timeout.as_millis())
                ));
            }
            
            // Check if this channel connects to destination
            if channel.destination == *destination {
                // Check if channel has sufficient capacity
                if channel.capacity * 1000 < amount_msat {
                    continue;
                }
                
                // Check if channel is active
                if !channel.is_active {
                    continue;
                }
                
                // Calculate fee
                let fee_msat = channel.base_fee_msat as u64 + 
                    (amount_msat * channel.fee_rate_millionths as u64) / 1_000_000;
                
                // Create hop
                let hop = PathHop {
                    node_id: destination.clone(),
                    channel_id: channel.channel_id.clone(),
                    amount_msat,
                    cltv_expiry: 40, // Default CLTV delta
                };
                
                // Add hop to path
                path.add_hop(hop);
                path.total_fee_msat = fee_msat;
                path.total_cltv_delta = 40;
                path.total_amount_msat = amount_msat + fee_msat;
                
                return Ok(path);
            }
        }
        
        // No direct path found
        // A real implementation would try multi-hop paths
        Err(RoutingError::NoRouteFound)
    }
    
    /// Handle a route failure and find a new route
    pub fn handle_route_failure(
        &mut self,
        path: &PaymentPath,
        failure_point: usize,
        failure_reason: &str,
    ) -> Result<PaymentPath, RoutingError> {
        // In a real implementation, this would:
        // 1. Update the success probability for failed channels
        // 2. Add to avoid list
        // 3. Find a new route
        
        // Update success probability for the failed channel
        if failure_point < path.hops.len() {
            let failed_hop = &path.hops[failure_point];
            self.scorer.update_success_probability(&failed_hop.channel_id, false);
        }
        
        // Find a new route with modified preferences
        let mut new_prefs = self.preferences.clone();
        
        // Add failed channels to avoid list
        for (i, hop) in path.hops.iter().enumerate() {
            if i <= failure_point {
                new_prefs.avoid_channels.insert(hop.channel_id.clone());
            }
        }
        
        // Save preferences temporarily
        let old_prefs = self.preferences.clone();
        self.preferences = new_prefs;
        
        // Find a new route
        let last_hop = path.hops.last().ok_or_else(|| RoutingError::NoRouteFound)?;
        let result = self.find_route(
            last_hop.node_id.as_str(),
            last_hop.amount_msat,
            &[], // No route hints for retry
        );
        
        // Restore preferences
        self.preferences = old_prefs;
        
        result
    }
    
    /// Update the network graph with a new channel
    pub fn update_channel(&mut self, channel: ChannelInfo, is_private: bool) {
        self.graph.add_channel(channel, is_private);
    }
    
    /// Remove a channel from the network graph
    pub fn remove_channel(&mut self, channel_id: &ChannelId) {
        self.graph.remove_channel(channel_id);
    }
    
    /// Get the number of nodes in the network
    pub fn node_count(&self) -> usize {
        self.graph.node_count()
    }
    
    /// Get the number of channels in the network
    pub fn channel_count(&self, include_private: bool) -> usize {
        self.graph.channel_count(include_private)
    }
} 