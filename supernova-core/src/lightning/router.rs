// supernova Lightning Network - Router Implementation
//
// This file contains the implementation of the Lightning Network router,
// which handles finding payment paths and routing payments through the network.

use crate::lightning::channel::ChannelId;
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use thiserror::Error;
use tracing::{error, info, warn};

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

/// Node identifier in the Lightning Network
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
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

impl std::fmt::Display for NodeId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
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
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PathHop {
    /// Node ID
    pub node_id: NodeId,

    /// Channel ID
    pub channel_id: ChannelId,

    /// Amount to forward in millinovas
    pub amount_msat: u64,

    /// CLTV expiry
    pub cltv_expiry: u32,

    /// Base fee in millinovas
    pub base_fee_msat: u32,

    /// Fee rate in parts per million
    pub fee_rate_millionths: u32,

    /// CLTV expiry delta
    pub cltv_expiry_delta: u16,
}

impl PathHop {
    /// Calculate the fee for forwarding a payment through this channel
    pub fn channel_fee(&self, amount_msat: u64) -> u64 {
        // Fee calculation: base_fee + (amount * fee_rate / 1_000_000)
        let proportional_fee = (amount_msat * self.fee_rate_millionths as u64) / 1_000_000;
        self.base_fee_msat as u64 + proportional_fee
    }
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

impl Default for PaymentPath {
    fn default() -> Self {
        Self::new()
    }
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
            max_cltv_expiry_delta: 1440, // Max 24 hours (assuming 10 min blocks)
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
#[derive(Debug)]
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

impl Clone for ScoringFunction {
    fn clone(&self) -> Self {
        match self {
            ScoringFunction::SuccessProbability => ScoringFunction::SuccessProbability,
            ScoringFunction::LowestFee => ScoringFunction::LowestFee,
            ScoringFunction::ShortestPath => ScoringFunction::ShortestPath,
            ScoringFunction::Custom(func) => ScoringFunction::Custom(*func),
        }
    }
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

impl Clone for ChannelScorer {
    fn clone(&self) -> Self {
        Self {
            scoring_function: self.scoring_function.clone(),
            success_probability: self.success_probability.clone(),
            historical_data: self.historical_data.clone(),
        }
    }
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
                let prob = self
                    .success_probability
                    .get(&channel.channel_id)
                    .cloned()
                    .unwrap_or(0.5); // Default 50% if no data

                (prob * 1_000_000.0) as u64
            }
            ScoringFunction::LowestFee => {
                // Score inversely proportional to fees
                let fee_score =
                    1_000_000 - channel.base_fee_msat as u64 - channel.fee_rate_millionths as u64;
                std::cmp::max(1, fee_score) // Ensure score is at least 1
            }
            ScoringFunction::ShortestPath => {
                // All channels get the same score
                1
            }
            ScoringFunction::Custom(func) => func(channel),
        }
    }

    /// Update success probability for a channel
    pub fn update_success_probability(&mut self, channel_id: &ChannelId, success: bool) {
        let data = self
            .historical_data
            .entry(channel_id.clone())
            .or_insert_with(|| ChannelHistoricalData {
                successful_payments: 0,
                failed_payments: 0,
                average_time_ms: 0,
                last_success: None,
            });

        if success {
            data.successful_payments += 1;
            data.last_success = Some(
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or(Duration::from_secs(0))
                    .as_secs(),
            );
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
#[derive(Clone)]
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

impl Default for NetworkGraph {
    fn default() -> Self {
        Self::new()
    }
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
        self.nodes.entry(node_id).or_default();
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
            self.private_channels
                .insert(channel.channel_id.clone(), channel);
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
    pub fn get_channel(
        &self,
        channel_id: &ChannelId,
        include_private: bool,
    ) -> Option<&ChannelInfo> {
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
#[derive(Clone)]
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

impl Default for Router {
    fn default() -> Self {
        Self::new()
    }
}

impl Router {
    /// Create a new router
    pub fn new() -> Self {
        Self {
            graph: NetworkGraph::new(),
            preferences: RouterPreferences::default(),
            scorer: ChannelScorer::new(ScoringFunction::SuccessProbability),
            local_node: NodeId::new("local".to_string()),
        }
    }

    /// Start the router
    pub async fn start(&self) -> Result<(), RoutingError> {
        info!("Starting Lightning Network router");
        // In a real implementation, this would start background tasks for:
        // - Network graph synchronization
        // - Channel monitoring
        // - Route optimization
        Ok(())
    }

    /// Stop the router
    pub async fn stop(&self) -> Result<(), RoutingError> {
        info!("Stopping Lightning Network router");
        // In a real implementation, this would stop background tasks
        Ok(())
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
            return Err(RoutingError::InvalidDestination(format!(
                "Destination node {} not found in graph",
                destination
            )));
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
        // Implement Dijkstra's algorithm for Lightning Network routing
        use std::collections::BinaryHeap;

        #[derive(Debug, Clone, PartialEq, Eq)]
        struct RouteState {
            cost: u64,
            node: NodeId,
            path: Vec<PathHop>,
            total_cltv: u32,
        }

        impl Ord for RouteState {
            fn cmp(&self, other: &Self) -> Ordering {
                // Reverse for min-heap behavior
                other
                    .cost
                    .cmp(&self.cost)
                    .then_with(|| other.total_cltv.cmp(&self.total_cltv))
            }
        }

        impl PartialOrd for RouteState {
            fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
                Some(self.cmp(other))
            }
        }

        let mut heap = BinaryHeap::new();
        let mut distances: HashMap<NodeId, u64> = HashMap::new();
        let mut visited: HashSet<NodeId> = HashSet::new();

        // Initialize with source node
        heap.push(RouteState {
            cost: 0,
            node: source.clone(),
            path: Vec::new(),
            total_cltv: 0,
        });
        distances.insert(source.clone(), 0);

        while let Some(current) = heap.pop() {
            // Check timeout
            if start_time.elapsed() > *timeout {
                return Err(RoutingError::Timeout(format!(
                    "Path finding timed out after {} ms",
                    timeout.as_millis()
                )));
            }

            // Skip if we've already found a better path to this node
            if visited.contains(&current.node) {
                continue;
            }

            // Mark as visited
            visited.insert(current.node.clone());

            // Check if we've reached the destination
            if current.node == *destination {
                let mut path = PaymentPath::new();
                path.hops = current.path;
                path.total_fee_msat = current.cost;
                path.total_cltv_delta = current.total_cltv;
                path.total_amount_msat = amount_msat + current.cost;
                return Ok(path);
            }

            // Check hop limit
            if current.path.len() >= self.preferences.max_hops as usize {
                continue;
            }

            // Explore neighbors
            let channels =
                graph.get_node_channels(&current.node, self.preferences.use_private_channels);

            for channel in channels {
                // Skip if channel is not active
                if !channel.is_active {
                    continue;
                }

                // Skip if channel doesn't have enough capacity
                if channel.capacity * 1000 < amount_msat + current.cost {
                    continue;
                }

                // Skip if we should avoid this channel
                if self
                    .preferences
                    .avoid_channels
                    .contains(&channel.channel_id)
                {
                    continue;
                }

                // Skip if we should avoid the destination node
                if self.preferences.avoid_nodes.contains(&channel.destination) {
                    continue;
                }

                // Calculate fee for this hop
                let hop_amount = amount_msat + current.cost;
                let fee = channel.base_fee_msat as u64
                    + (hop_amount * channel.fee_rate_millionths as u64) / 1_000_000;

                // Check fee rate limits
                if channel.fee_rate_millionths > self.preferences.max_fee_rate_millionths {
                    continue;
                }

                let new_cost = current.cost + fee;
                let new_cltv = current.total_cltv + channel.cltv_expiry_delta as u32;

                // Check CLTV limits
                if new_cltv > self.preferences.max_cltv_expiry_delta as u32 {
                    continue;
                }

                // Check if this is a better path to the destination node
                let current_distance = distances
                    .get(&channel.destination)
                    .cloned()
                    .unwrap_or(u64::MAX);
                if new_cost < current_distance {
                    distances.insert(channel.destination.clone(), new_cost);

                    // Create new hop
                    let hop = PathHop {
                        node_id: channel.destination.clone(),
                        channel_id: channel.channel_id.clone(),
                        amount_msat: hop_amount,
                        cltv_expiry: new_cltv,
                        base_fee_msat: channel.base_fee_msat,
                        fee_rate_millionths: channel.fee_rate_millionths,
                        cltv_expiry_delta: channel.cltv_expiry_delta,
                    };

                    // Create new path
                    let mut new_path = current.path.clone();
                    new_path.push(hop);

                    // Add to heap for exploration
                    heap.push(RouteState {
                        cost: new_cost,
                        node: channel.destination.clone(),
                        path: new_path,
                        total_cltv: new_cltv,
                    });
                }
            }
        }

        // No route found - try fallback to direct path
        self.find_direct_path(graph, source, destination, amount_msat)
    }

    /// Find a direct path as fallback
    fn find_direct_path(
        &self,
        graph: &NetworkGraph,
        source: &NodeId,
        destination: &NodeId,
        amount_msat: u64,
    ) -> Result<PaymentPath, RoutingError> {
        // Find a channel between source and destination
        let channels = graph.get_node_channels(source, self.preferences.use_private_channels);

        for channel in channels {
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
                let fee_msat = channel.base_fee_msat as u64
                    + (amount_msat * channel.fee_rate_millionths as u64) / 1_000_000;

                // Create hop
                let hop = PathHop {
                    node_id: destination.clone(),
                    channel_id: channel.channel_id.clone(),
                    amount_msat,
                    cltv_expiry: 40, // Default CLTV delta
                    base_fee_msat: channel.base_fee_msat,
                    fee_rate_millionths: channel.fee_rate_millionths,
                    cltv_expiry_delta: channel.cltv_expiry_delta,
                };

                // Create path
                let mut path = PaymentPath::new();
                path.add_hop(hop);
                path.total_fee_msat = fee_msat;
                path.total_cltv_delta = 40;
                path.total_amount_msat = amount_msat + fee_msat;

                return Ok(path);
            }
        }

        // No route found
        Err(RoutingError::NoRouteFound)
    }

    /// Handle a route failure and find a new route
    pub fn handle_route_failure(
        &mut self,
        path: &PaymentPath,
        failure_point: usize,
        _failure_reason: &str,
    ) -> Result<PaymentPath, RoutingError> {
        // In a real implementation, this would:
        // 1. Update the success probability for failed channels
        // 2. Add to avoid list
        // 3. Find a new route

        // Update success probability for the failed channel
        if failure_point < path.hops.len() {
            let failed_hop = &path.hops[failure_point];
            self.scorer
                .update_success_probability(&failed_hop.channel_id, false);
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
        let last_hop = path.hops.last().ok_or(RoutingError::NoRouteFound)?;
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

    /// Find multiple alternative routes to a destination
    pub fn find_alternative_routes(
        &self,
        destination: &str,
        amount_msat: u64,
        route_hints: &[RouteHint],
        num_routes: usize,
    ) -> Result<Vec<PaymentPath>, RoutingError> {
        let mut routes = Vec::with_capacity(num_routes);

        // First, try to find the best route
        let first_route = self.find_route(destination, amount_msat, route_hints)?;
        routes.push(first_route);

        // Create a set of avoided channels and nodes for finding alternative routes
        let mut avoid_channels = self.preferences.avoid_channels.clone();
        let mut avoid_nodes = self.preferences.avoid_nodes.clone();

        // Add channels and nodes from the first route to avoided sets
        for hop in &routes[0].hops {
            avoid_channels.insert(hop.channel_id.clone());
            avoid_nodes.insert(hop.node_id.clone());
        }

        // Create modified preferences for alternative routes
        let mut alt_preferences = self.preferences.clone();
        alt_preferences.avoid_channels = avoid_channels;
        alt_preferences.avoid_nodes = avoid_nodes;

        // Try to find additional routes
        for _ in 1..num_routes {
            let mut router_copy = self.clone();
            router_copy.preferences = alt_preferences.clone();

            match router_copy.find_route(destination, amount_msat, route_hints) {
                Ok(route) => {
                    // Add this route to the list
                    routes.push(route.clone());

                    // Add channels and nodes from this route to avoided sets
                    for hop in &route.hops {
                        alt_preferences
                            .avoid_channels
                            .insert(hop.channel_id.clone());
                        alt_preferences.avoid_nodes.insert(hop.node_id.clone());
                    }
                }
                Err(_) => {
                    // No more routes found
                    break;
                }
            }
        }

        Ok(routes)
    }

    /// Split a payment across multiple routes for better reliability
    pub fn split_payment(
        &self,
        destination: &str,
        total_amount_msat: u64,
        route_hints: &[RouteHint],
        num_parts: usize,
    ) -> Result<Vec<(PaymentPath, u64)>, RoutingError> {
        if num_parts <= 1 {
            // No splitting needed
            let route = self.find_route(destination, total_amount_msat, route_hints)?;
            return Ok(vec![(route, total_amount_msat)]);
        }

        // Calculate amount per part (ensure it's above minimum)
        let min_amount = 1000; // 1 sat minimum
        let amount_per_part = std::cmp::max(total_amount_msat / num_parts as u64, min_amount);

        // Adjust num_parts if necessary
        let adjusted_num_parts = if amount_per_part * num_parts as u64 > total_amount_msat {
            (total_amount_msat / amount_per_part) as usize
        } else {
            num_parts
        };

        // Find multiple routes
        let routes = self.find_alternative_routes(
            destination,
            amount_per_part,
            route_hints,
            adjusted_num_parts,
        )?;

        if routes.is_empty() {
            return Err(RoutingError::NoRouteFound);
        }

        let mut result = Vec::with_capacity(adjusted_num_parts);
        let mut remaining_amount = total_amount_msat;

        // Distribute amount across routes
        for i in 0..adjusted_num_parts {
            if i == adjusted_num_parts - 1 {
                // Last part gets any remainder
                result.push((routes[i % routes.len()].clone(), remaining_amount));
                break;
            }

            result.push((routes[i % routes.len()].clone(), amount_per_part));
            remaining_amount -= amount_per_part;
        }

        Ok(result)
    }

    /// Find optimal path based on channel capacity constraints
    pub fn find_capacity_constrained_path(
        &self,
        destination: &str,
        amount_msat: u64,
        route_hints: &[RouteHint],
        min_channel_capacity: u64,
    ) -> Result<PaymentPath, RoutingError> {
        // Create a custom scoring function that prioritizes channels with sufficient capacity
        let capacity_scorer = ChannelScorer::new(ScoringFunction::SuccessProbability);

        // Find route with capacity constraints
        let mut router_copy = self.clone();
        router_copy.scorer = capacity_scorer;

        // Try to find a route, filtering by capacity in the routing logic
        let mut attempts = 0;
        let max_attempts = 5;

        while attempts < max_attempts {
            match router_copy.find_route(destination, amount_msat, route_hints) {
                Ok(path) => {
                    // Check if all channels in the path meet capacity requirements
                    let mut meets_capacity = true;
                    for hop in &path.hops {
                        if let Some(channel) = router_copy.graph.get_channel(&hop.channel_id, true)
                        {
                            if channel.capacity < min_channel_capacity {
                                meets_capacity = false;
                                // Add this channel to avoid list for next attempt
                                router_copy
                                    .preferences
                                    .avoid_channels
                                    .insert(hop.channel_id.clone());
                                break;
                            }
                        }
                    }

                    if meets_capacity {
                        return Ok(path);
                    }
                }
                Err(e) => return Err(e),
            }

            attempts += 1;
        }

        // If we can't find a route with sufficient capacity, return the best available
        router_copy.find_route(destination, amount_msat, route_hints)
    }

    /// Calculate reliability score for a path
    pub fn calculate_path_reliability(&self, path: &PaymentPath) -> f64 {
        if path.hops.is_empty() {
            return 0.0;
        }

        // Calculate individual hop success probabilities
        let mut hop_probabilities = Vec::with_capacity(path.hops.len());

        for hop in &path.hops {
            // Get channel success probability from scorer
            let prob = self
                .scorer
                .success_probability
                .get(&hop.channel_id)
                .cloned()
                .unwrap_or(0.9); // Default 90% if no data

            hop_probabilities.push(prob);
        }

        // Calculate overall path reliability (product of individual hop probabilities)
        hop_probabilities.iter().fold(1.0, |acc, &p| acc * p)
    }

    /// Update routing table with gossip information
    pub fn update_from_gossip(
        &mut self,
        channels: Vec<ChannelInfo>,
    ) -> Result<usize, RoutingError> {
        let mut updated_count = 0;

        for channel in channels {
            // Update the channel in the graph
            self.graph.add_channel(channel.clone(), false);
            updated_count += 1;
        }

        info!("Updated {} channels from gossip information", updated_count);

        Ok(updated_count)
    }

    /// Discover and add new nodes to the routing graph
    pub fn discover_nodes(&mut self, seed_nodes: &[NodeId]) -> Result<usize, RoutingError> {
        let mut discovered_count = 0;

        // In a real implementation, this would:
        // 1. Connect to seed nodes
        // 2. Request their known nodes and channels
        // 3. Recursively explore the network up to a certain depth

        for node_id in seed_nodes {
            self.graph.add_node(node_id.clone());
            discovered_count += 1;
        }

        info!("Discovered {} new nodes", discovered_count);

        Ok(discovered_count)
    }

    /// Optimize a route for lower fees
    pub fn optimize_route_for_fees(&self, path: &PaymentPath) -> Result<PaymentPath, RoutingError> {
        if path.hops.len() <= 1 {
            return Ok(path.clone()); // No optimization needed for single-hop paths
        }

        // Create a new path with the same endpoints
        let _optimized = PaymentPath::new();
        let amount_msat = path.total_amount_msat;

        // Get start and end nodes
        let _source = &path.hops.first().unwrap().node_id;
        let destination = &path.hops.last().unwrap().node_id;

        // Create modified preferences that optimize for fees
        let fee_preferences = self.preferences.clone();

        // Set a custom scoring function that prioritizes lower fees
        let fee_scorer = ChannelScorer::new(ScoringFunction::LowestFee);

        // Find route with fee optimization
        let mut router_copy = self.clone();
        router_copy.preferences = fee_preferences;
        router_copy.scorer = fee_scorer;

        let destination_str = destination.as_str().to_string();
        router_copy.find_route(&destination_str, amount_msat, &[])
    }

    /// Find a path optimized for reliability
    pub fn find_reliable_path(
        &self,
        destination: &str,
        amount_msat: u64,
        min_reliability: f64,
    ) -> Result<PaymentPath, RoutingError> {
        // Try to find a path with standard algorithm
        let path = self.find_route(destination, amount_msat, &[])?;

        // Calculate reliability
        let reliability = self.calculate_path_reliability(&path);

        if reliability >= min_reliability {
            return Ok(path);
        }

        // If reliability is too low, try to find alternative paths
        let alt_paths = self.find_alternative_routes(destination, amount_msat, &[], 5)?;

        // Find the most reliable path
        let mut most_reliable_path = path;
        let mut highest_reliability = reliability;

        for alt_path in alt_paths {
            let alt_reliability = self.calculate_path_reliability(&alt_path);

            if alt_reliability > highest_reliability {
                most_reliable_path = alt_path;
                highest_reliability = alt_reliability;

                if highest_reliability >= min_reliability {
                    break; // Found a path with sufficient reliability
                }
            }
        }

        if highest_reliability < min_reliability {
            warn!(
                "Could not find path with required reliability {} (best: {})",
                min_reliability, highest_reliability
            );
        }

        Ok(most_reliable_path)
    }
}

/// Payment tracker for multi-hop payments
pub struct PaymentTracker {
    /// Payment hash
    payment_hash: [u8; 32],

    /// Payment parts
    parts: Vec<PaymentPart>,

    /// Total amount
    total_amount_msat: u64,

    /// Status
    status: PaymentStatus,

    /// Creation time
    creation_time: u64,

    /// Completion time
    completion_time: Option<u64>,
}

/// Status of a payment
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PaymentStatus {
    /// Payment is pending
    Pending,

    /// Payment succeeded
    Succeeded,

    /// Payment failed
    Failed(String),

    /// Payment is in progress
    InProgress,
}

/// Part of a split payment
#[derive(Debug, Clone)]
pub struct PaymentPart {
    /// Part ID
    id: u64,

    /// Amount in millisatoshis
    amount_msat: u64,

    /// Payment path
    path: PaymentPath,

    /// Status
    status: PaymentStatus,

    /// Attempt count
    attempts: u32,

    /// Last attempted time
    last_attempt: u64,
}

impl PaymentTracker {
    /// Create a new payment tracker
    pub fn new(payment_hash: [u8; 32], total_amount_msat: u64) -> Self {
        Self {
            payment_hash,
            parts: Vec::new(),
            total_amount_msat,
            status: PaymentStatus::Pending,
            creation_time: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or(Duration::from_secs(0))
                .as_secs(),
            completion_time: None,
        }
    }

    /// Add a payment part
    pub fn add_part(&mut self, path: PaymentPath, amount_msat: u64) -> u64 {
        let id = self.parts.len() as u64;

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or(Duration::from_secs(0))
            .as_secs();

        let part = PaymentPart {
            id,
            amount_msat,
            path,
            status: PaymentStatus::Pending,
            attempts: 0,
            last_attempt: now,
        };

        self.parts.push(part);

        id
    }

    /// Update the status of a payment part
    pub fn update_part_status(&mut self, part_id: u64, status: PaymentStatus) {
        if let Some(part) = self.parts.iter_mut().find(|p| p.id == part_id) {
            part.status = status.clone();
            part.attempts += 1;

            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or(Duration::from_secs(0))
                .as_secs();

            part.last_attempt = now;
        }

        // Check if all parts are complete
        self.update_overall_status();
    }

    /// Update the overall payment status
    fn update_overall_status(&mut self) {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or(Duration::from_secs(0))
            .as_secs();

        // Count succeeded and failed parts
        let succeeded = self
            .parts
            .iter()
            .filter(|p| p.status == PaymentStatus::Succeeded)
            .count();
        let failed = self
            .parts
            .iter()
            .filter(|p| matches!(p.status, PaymentStatus::Failed(_)))
            .count();

        if succeeded == self.parts.len() {
            // All parts succeeded
            self.status = PaymentStatus::Succeeded;
            self.completion_time = Some(now);
        } else if failed > 0 && failed + succeeded == self.parts.len() {
            // All parts are complete, but some failed
            self.status = PaymentStatus::Failed("Some payment parts failed".to_string());
            self.completion_time = Some(now);
        } else if failed > 0 || succeeded > 0 {
            // Some parts are complete
            self.status = PaymentStatus::InProgress;
        }
    }

    /// Get all failed payment parts
    pub fn get_failed_parts(&self) -> Vec<&PaymentPart> {
        self.parts
            .iter()
            .filter(|p| matches!(p.status, PaymentStatus::Failed(_)))
            .collect()
    }

    /// Get the overall payment status
    pub fn status(&self) -> &PaymentStatus {
        &self.status
    }

    /// Get the payment hash
    pub fn payment_hash(&self) -> &[u8; 32] {
        &self.payment_hash
    }

    /// Get the total amount
    pub fn total_amount_msat(&self) -> u64 {
        self.total_amount_msat
    }

    /// Get the successful amount
    pub fn successful_amount_msat(&self) -> u64 {
        self.parts
            .iter()
            .filter(|p| p.status == PaymentStatus::Succeeded)
            .map(|p| p.amount_msat)
            .sum()
    }

    /// Get elapsed time in seconds
    pub fn elapsed_seconds(&self) -> u64 {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or(Duration::from_secs(0))
            .as_secs();

        now - self.creation_time
    }
}

/// Multi-path payment coordinator
pub struct MultiPathPaymentCoordinator {
    /// Active payments
    payments: HashMap<[u8; 32], PaymentTracker>,

    /// Router for finding paths
    router: Arc<RwLock<Router>>,
}

impl MultiPathPaymentCoordinator {
    /// Create a new multi-path payment coordinator
    pub fn new(router: Arc<RwLock<Router>>) -> Self {
        Self {
            payments: HashMap::new(),
            router,
        }
    }

    /// Start a new multi-path payment
    pub fn start_payment(
        &mut self,
        payment_hash: [u8; 32],
        destination: &str,
        amount_msat: u64,
        num_parts: usize,
    ) -> Result<Vec<PaymentPart>, RoutingError> {
        // Create a new payment tracker
        let mut tracker = PaymentTracker::new(payment_hash, amount_msat);

        // Get the router
        let router = self.router.read().unwrap();

        // Split the payment across multiple routes
        let split_payment = router.split_payment(destination, amount_msat, &[], num_parts)?;

        // Add each part to the tracker
        for (path, part_amount) in split_payment {
            tracker.add_part(path, part_amount);
        }

        // Register the payment
        let parts = tracker.parts.clone();
        self.payments.insert(payment_hash, tracker);

        Ok(parts)
    }

    /// Update the status of a payment part
    pub fn update_part_status(
        &mut self,
        payment_hash: &[u8; 32],
        part_id: u64,
        status: PaymentStatus,
    ) -> Result<PaymentStatus, RoutingError> {
        // Find the payment
        let tracker = self.payments.get_mut(payment_hash).ok_or_else(|| {
            RoutingError::InvalidDestination(format!(
                "Payment with hash {:x?} not found",
                &payment_hash[0..4]
            ))
        })?;

        // Update the part status
        tracker.update_part_status(part_id, status);

        // Return the overall payment status
        Ok(tracker.status().clone())
    }

    /// Get the status of a payment
    pub fn get_payment_status(
        &self,
        payment_hash: &[u8; 32],
    ) -> Result<PaymentStatus, RoutingError> {
        // Find the payment
        let tracker = self.payments.get(payment_hash).ok_or_else(|| {
            RoutingError::InvalidDestination(format!(
                "Payment with hash {:x?} not found",
                &payment_hash[0..4]
            ))
        })?;

        Ok(tracker.status().clone())
    }

    /// Retry failed payment parts
    pub fn retry_failed_parts(
        &mut self,
        payment_hash: &[u8; 32],
        max_attempts: u32,
    ) -> Result<Vec<PaymentPart>, RoutingError> {
        // Find the payment
        let tracker = self.payments.get_mut(payment_hash).ok_or_else(|| {
            RoutingError::InvalidDestination(format!(
                "Payment with hash {:x?} not found",
                &payment_hash[0..4]
            ))
        })?;

        // Find failed parts that haven't exceeded max attempts
        // Collect the data we need instead of holding references to avoid borrowing conflicts
        let failed_part_data: Vec<_> = tracker
            .parts
            .iter()
            .filter(|p| matches!(p.status, PaymentStatus::Failed(_)) && p.attempts < max_attempts)
            .map(|p| (p.amount_msat, p.path.clone()))
            .collect();

        let mut retried_parts = Vec::new();

        // Get the router
        let router = self.router.read().unwrap();

        for (amount_msat, path) in failed_part_data {
            // Try to find a new route for this part
            let destination = path
                .hops
                .last()
                .map(|h| h.node_id.as_str().to_string())
                .unwrap_or_default();

            // Find a different route
            match router.find_route(&destination, amount_msat, &[]) {
                Ok(new_path) => {
                    // Add a new part with the new path
                    let part_id = tracker.add_part(new_path, amount_msat);

                    if let Some(new_part) = tracker.parts.iter().find(|p| p.id == part_id) {
                        retried_parts.push(new_part.clone());
                    }
                }
                Err(e) => {
                    warn!("Failed to find alternative route for payment part: {}", e);
                }
            }
        }

        Ok(retried_parts)
    }
}

/// A hop in a payment route (for compatibility with manager)
#[derive(Debug, Clone)]
pub struct RouteHop {
    /// Channel ID
    pub channel_id: ChannelId,
    /// Node ID
    pub node_id: String,
    /// Amount to forward in millisatoshis
    pub amount_msat: u64,
    /// Fee for this hop in millisatoshis
    pub fee_msat: u64,
    /// CLTV expiry delta
    pub cltv_expiry_delta: u16,
}

impl RouteHop {
    /// Calculate fee for forwarding an amount through this hop
    pub fn channel_fee(&self, amount_msat: u64) -> u64 {
        // Base fee + proportional fee
        let base_fee = 1000; // 1 sat base fee
        let proportional_fee = (amount_msat * 100) / 1_000_000; // 0.01% proportional fee
        base_fee + proportional_fee
    }
}
