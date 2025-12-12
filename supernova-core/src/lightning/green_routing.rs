// Green Lightning Routing System for Supernova
// Implements carbon-conscious payment routing with environmental optimization
// Prioritizes renewable energy nodes and carbon-negative routes

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::collections::{BinaryHeap, HashMap};
use std::sync::{Arc, RwLock};

use crate::environmental::{carbon_tracking::CarbonTracker, types::Region};
use crate::lightning::quantum_lightning::{GreenLightningRoute, GreenRouteHop};

/// Green routing optimizer for Lightning Network
pub struct GreenLightningRouter {
    /// Network graph with environmental data
    network_graph: Arc<RwLock<EnvironmentalNetworkGraph>>,

    /// Routing algorithm parameters
    routing_params: Arc<RwLock<GreenRoutingParameters>>,

    /// Carbon tracker
    carbon_tracker: Arc<CarbonTracker>,

    /// Route cache
    route_cache: Arc<RwLock<HashMap<RouteCacheKey, CachedRoute>>>,

    /// Performance metrics
    metrics: Arc<RwLock<RoutingMetrics>>,
}

/// Environmental network graph
#[derive(Debug, Clone)]
pub struct EnvironmentalNetworkGraph {
    /// Nodes with environmental data
    pub nodes: HashMap<NodeId, EnvironmentalNode>,

    /// Channels between nodes
    pub channels: HashMap<ChannelId, EnvironmentalChannel>,

    /// Environmental zones
    pub zones: HashMap<Region, EnvironmentalZone>,
}

/// Node with environmental attributes
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnvironmentalNode {
    #[serde(with = "serde_arrays")]
    pub node_id: NodeId,
    pub public_key: Vec<u8>,
    pub alias: String,

    /// Environmental data
    pub renewable_percentage: f64,
    pub carbon_footprint_per_tx: f64,
    pub green_certified: bool,
    pub environmental_score: f64,

    /// Carbon offset status
    pub carbon_negative: bool,
    pub monthly_carbon_saved: f64,

    /// Location
    pub region: Region,
    pub coordinates: Option<(f64, f64)>,

    /// Routing preferences
    pub prefers_green_routes: bool,
    pub green_fee_discount: f64,
}

/// Channel with environmental metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnvironmentalChannel {
    #[serde(with = "serde_arrays")]
    pub channel_id: ChannelId,
    #[serde(with = "serde_arrays")]
    pub node1: NodeId,
    #[serde(with = "serde_arrays")]
    pub node2: NodeId,

    /// Channel capacity
    pub capacity_sats: u64,
    pub available_balance: u64,

    /// Routing fees
    pub base_fee_mnova: u32,
    pub fee_rate_ppm: u32,

    /// Environmental metrics
    pub carbon_footprint: f64,
    pub renewable_powered: bool,
    pub environmental_score: f64,

    /// Performance
    pub success_rate: f64,
    pub avg_settlement_time: f64,
}

/// Environmental zone data
#[derive(Debug, Clone)]
pub struct EnvironmentalZone {
    pub region: Region,
    pub average_renewable_percentage: f64,
    pub carbon_intensity: f64,
    pub green_nodes_count: usize,
    pub total_nodes: usize,
}

/// Green routing parameters
#[derive(Debug, Clone)]
pub struct GreenRoutingParameters {
    /// Weight factors for route selection
    pub fee_weight: f64,
    pub carbon_weight: f64,
    pub renewable_weight: f64,
    pub reliability_weight: f64,

    /// Constraints
    pub max_carbon_per_route: f64,
    pub min_renewable_percentage: f64,
    pub max_route_length: usize,

    /// Incentives
    pub green_node_preference: f64,
    pub carbon_negative_bonus: f64,
}

/// Route cache key
#[derive(Debug, Clone, Hash, Eq, PartialEq)]
struct RouteCacheKey {
    source: NodeId,
    destination: NodeId,
    amount_sats: u64,
    prefer_green: bool,
}

/// Cached route with expiry
#[derive(Debug, Clone)]
struct CachedRoute {
    route: GreenLightningRoute,
    cached_at: DateTime<Utc>,
    expires_at: DateTime<Utc>,
}

/// Routing metrics
#[derive(Debug, Clone, Default)]
struct RoutingMetrics {
    pub total_routes_calculated: u64,
    pub green_routes_found: u64,
    pub carbon_negative_routes: u64,
    pub average_carbon_per_route: f64,
    pub total_carbon_saved: f64,
}

type NodeId = [u8; 33];
type ChannelId = [u8; 32];

impl GreenLightningRouter {
    /// Create new green Lightning router
    pub fn new(carbon_tracker: Arc<CarbonTracker>) -> Self {
        let default_params = GreenRoutingParameters {
            fee_weight: 0.4,
            carbon_weight: 0.3,
            renewable_weight: 0.2,
            reliability_weight: 0.1,
            max_carbon_per_route: 0.01, // 10g CO2e max
            min_renewable_percentage: 50.0,
            max_route_length: 6,
            green_node_preference: 1.2,
            carbon_negative_bonus: 1.5,
        };

        Self {
            network_graph: Arc::new(RwLock::new(EnvironmentalNetworkGraph {
                nodes: HashMap::new(),
                channels: HashMap::new(),
                zones: HashMap::new(),
            })),
            routing_params: Arc::new(RwLock::new(default_params)),
            carbon_tracker,
            route_cache: Arc::new(RwLock::new(HashMap::new())),
            metrics: Arc::new(RwLock::new(RoutingMetrics::default())),
        }
    }

    /// Calculate route carbon footprint
    pub fn calculate_route_carbon_footprint(&self, route: &GreenLightningRoute) -> f64 {

        let mut total_carbon = 0.0;

        for hop in &route.hops {
            // Hop carbon footprint
            total_carbon += hop.carbon_footprint;

            // Additional factors
            if !hop.green_certified {
                total_carbon += 0.0001; // Penalty for non-green nodes
            }
        }

        // Apply route length factor
        let length_factor = 1.0 + (route.hops.len() as f64 - 1.0) * 0.05;
        total_carbon *= length_factor;


        total_carbon
    }

    /// Optimize route for renewable energy nodes
    pub async fn optimize_for_renewable_energy_nodes(
        &self,
        source: NodeId,
        destination: NodeId,
        amount_sats: u64,
    ) -> Result<GreenLightningRoute, RoutingError> {

        // Check cache first
        let cache_key = RouteCacheKey {
            source,
            destination,
            amount_sats,
            prefer_green: true,
        };

        if let Some(cached) = self.get_cached_route(&cache_key) {
            return Ok(cached);
        }

        // Find optimal green route
        let route = self.find_optimal_green_route(source, destination, amount_sats)?;

        // Cache the route
        self.cache_route(cache_key, route.clone());

        // Update metrics
        self.update_routing_metrics(&route);


        Ok(route)
    }

    /// Apply environmental routing preferences
    pub fn apply_environmental_routing_preferences(
        &self,
        preferences: EnvironmentalRoutingPreferences,
    ) -> Result<(), RoutingError> {

        let mut params = self.routing_params.write().unwrap();

        // Update weights based on preferences
        match preferences.priority {
            RoutingPriority::LowestCarbon => {
                params.carbon_weight = 0.5;
                params.fee_weight = 0.2;
                params.renewable_weight = 0.3;
            }
            RoutingPriority::HighestRenewable => {
                params.renewable_weight = 0.5;
                params.carbon_weight = 0.3;
                params.fee_weight = 0.2;
            }
            RoutingPriority::Balanced => {
                params.fee_weight = 0.35;
                params.carbon_weight = 0.35;
                params.renewable_weight = 0.3;
            }
            RoutingPriority::CheapestGreen => {
                params.fee_weight = 0.5;
                params.carbon_weight = 0.25;
                params.renewable_weight = 0.25;
            }
        }

        // Apply constraints
        if let Some(max_carbon) = preferences.max_carbon_footprint {
            params.max_carbon_per_route = max_carbon;
        }

        if let Some(min_renewable) = preferences.min_renewable_percentage {
            params.min_renewable_percentage = min_renewable;
        }


        Ok(())
    }

    /// Incentivize green Lightning nodes
    pub fn incentivize_green_lightning_nodes(&self) -> GreenIncentiveProgram {

        let graph = self.network_graph.read().unwrap();

        // Calculate network-wide stats
        let total_nodes = graph.nodes.len();
        let green_nodes = graph.nodes.values().filter(|n| n.green_certified).count();
        let carbon_negative_nodes = graph.nodes.values().filter(|n| n.carbon_negative).count();

        let avg_renewable: f64 = graph
            .nodes
            .values()
            .map(|n| n.renewable_percentage)
            .sum::<f64>()
            / total_nodes.max(1) as f64;

        // Create incentive tiers
        let incentives = GreenIncentiveProgram {
            base_tier: GreenIncentiveTier {
                name: "Green Starter".to_string(),
                min_renewable: 50.0,
                fee_discount: 5.0,
                routing_preference: 1.1,
                badge: "🌱".to_string(),
            },
            silver_tier: GreenIncentiveTier {
                name: "Renewable Champion".to_string(),
                min_renewable: 75.0,
                fee_discount: 10.0,
                routing_preference: 1.25,
                badge: "⚡".to_string(),
            },
            gold_tier: GreenIncentiveTier {
                name: "Carbon Negative Hero".to_string(),
                min_renewable: 100.0,
                fee_discount: 20.0,
                routing_preference: 1.5,
                badge: "🌍".to_string(),
            },
            network_stats: NetworkGreenStats {
                total_nodes,
                green_nodes,
                carbon_negative_nodes,
                average_renewable_percentage: avg_renewable,
                total_carbon_saved: self.metrics.read().unwrap().total_carbon_saved,
            },
        };


        incentives
    }

    /// Measure payment carbon footprint
    pub fn measure_payment_carbon_footprint(
        &self,
        payment_amount_sats: u64,
        route: &GreenLightningRoute,
    ) -> PaymentEnvironmentalImpact {
        let base_carbon = route.total_carbon_footprint;

        // Scale by payment size (larger payments have slightly higher impact)
        let size_factor = 1.0 + (payment_amount_sats as f64 / 1_000_000_000.0).ln().max(0.0) * 0.01;
        let total_carbon = base_carbon * size_factor;

        // Calculate savings vs traditional payment
        let traditional_carbon = 0.01; // Traditional payment carbon estimate
        let carbon_saved = traditional_carbon - total_carbon;

        // Environmental equivalents
        let trees_equivalent = (carbon_saved.abs() * 50.0) as u32;
        let miles_equivalent = (carbon_saved.abs() * 2.5) as u32;

        PaymentEnvironmentalImpact {
            payment_amount_sats,
            route_length: route.hops.len(),
            total_carbon_kg: total_carbon,
            carbon_saved_kg: carbon_saved,
            renewable_percentage: route.average_renewable_percentage,
            green_nodes_used: route.green_nodes_count,
            trees_equivalent,
            miles_equivalent,
            is_carbon_negative: total_carbon < 0.0,
        }
    }

    /// Calculate environmental savings
    pub fn calculate_environmental_savings(
        &self,
        time_period: TimePeriod,
    ) -> EnvironmentalSavingsReport {
        let metrics = self.metrics.read().unwrap();
        let graph = self.network_graph.read().unwrap();

        // Calculate period savings
        let total_payments = metrics.total_routes_calculated;
        let green_payments = metrics.green_routes_found;
        let carbon_negative_payments = metrics.carbon_negative_routes;

        // Traditional payment carbon (estimated)
        let traditional_total_carbon = total_payments as f64 * 0.01;
        let actual_total_carbon = metrics.average_carbon_per_route * total_payments as f64;
        let total_carbon_saved = traditional_total_carbon - actual_total_carbon;

        // Network-wide impact
        let network_renewable_average = graph
            .nodes
            .values()
            .map(|n| n.renewable_percentage)
            .sum::<f64>()
            / graph.nodes.len().max(1) as f64;

        EnvironmentalSavingsReport {
            time_period,
            total_payments,
            green_payments,
            carbon_negative_payments,
            total_carbon_saved_kg: total_carbon_saved,
            average_carbon_per_payment: metrics.average_carbon_per_route,
            network_renewable_percentage: network_renewable_average,
            environmental_milestones: self.calculate_milestones(total_carbon_saved),
        }
    }

    /// Generate green payment certificate
    pub fn generate_green_payment_certificate(
        &self,
        payment_id: &str,
        impact: &PaymentEnvironmentalImpact,
    ) -> GreenPaymentCertificate {
        let cert_id = self.generate_certificate_id(payment_id);

        let achievement_level = if impact.is_carbon_negative {
            "Carbon Negative Champion 🌍"
        } else if impact.renewable_percentage >= 90.0 {
            "Renewable Energy Hero ⚡"
        } else if impact.renewable_percentage >= 75.0 {
            "Green Lightning User 🌱"
        } else {
            "Eco-Conscious Payer 🌿"
        };

        GreenPaymentCertificate {
            certificate_id: cert_id,
            payment_id: payment_id.to_string(),
            issued_at: Utc::now(),
            carbon_saved_kg: impact.carbon_saved_kg,
            renewable_percentage: impact.renewable_percentage,
            green_nodes_used: impact.green_nodes_used,
            achievement_level: achievement_level.to_string(),
            verification_hash: self.generate_verification_hash(payment_id, impact),
        }
    }

    /// Publish environmental Lightning statistics
    pub fn publish_environmental_lightning_stats(&self) -> EnvironmentalLightningStats {
        let metrics = self.metrics.read().unwrap();
        let graph = self.network_graph.read().unwrap();

        // Calculate zone statistics
        let mut zone_stats = Vec::new();
        for (region, zone) in &graph.zones {
            zone_stats.push(ZoneEnvironmentalStats {
                region: *region,
                green_nodes_percentage: (zone.green_nodes_count as f64 / zone.total_nodes as f64)
                    * 100.0,
                average_renewable: zone.average_renewable_percentage,
                carbon_intensity: zone.carbon_intensity,
            });
        }

        EnvironmentalLightningStats {
            timestamp: Utc::now(),
            total_green_routes: metrics.green_routes_found,
            total_carbon_saved_kg: metrics.total_carbon_saved,
            carbon_negative_routes_percentage: (metrics.carbon_negative_routes as f64
                / metrics.total_routes_calculated.max(1) as f64)
                * 100.0,
            network_renewable_average: graph
                .nodes
                .values()
                .map(|n| n.renewable_percentage)
                .sum::<f64>()
                / graph.nodes.len().max(1) as f64,
            top_green_nodes: self.get_top_green_nodes(&graph, 10),
            zone_statistics: zone_stats,
        }
    }

    // Helper methods

    fn find_optimal_green_route(
        &self,
        source: NodeId,
        destination: NodeId,
        amount_sats: u64,
    ) -> Result<GreenLightningRoute, RoutingError> {
        let graph = self.network_graph.read().unwrap();
        let params = self.routing_params.read().unwrap();

        // Use modified Dijkstra's algorithm with environmental weights
        let mut distances: HashMap<NodeId, f64> = HashMap::new();
        let mut previous: HashMap<NodeId, Option<(NodeId, ChannelId)>> = HashMap::new();
        let mut heap = BinaryHeap::new();

        // Initialize
        distances.insert(source, 0.0);
        heap.push(RouteNode {
            node_id: source,
            cost: 0.0,
            carbon_footprint: 0.0,
            renewable_percentage: 100.0,
        });

        while let Some(current) = heap.pop() {
            if current.node_id == destination {
                // Reconstruct path
                return self.reconstruct_green_route(
                    source,
                    destination,
                    &previous,
                    &graph,
                    amount_sats,
                );
            }

            if current.cost > *distances.get(&current.node_id).unwrap_or(&f64::MAX) {
                continue;
            }

            // Explore neighbors
            for (channel_id, channel) in &graph.channels {
                let next_node = if channel.node1 == current.node_id {
                    channel.node2
                } else if channel.node2 == current.node_id {
                    channel.node1
                } else {
                    continue;
                };

                // Calculate edge cost with environmental factors
                let edge_cost = self.calculate_edge_cost(
                    channel,
                    &graph.nodes[&next_node],
                    amount_sats,
                    &params,
                );

                let next_cost = current.cost + edge_cost;

                if next_cost < *distances.get(&next_node).unwrap_or(&f64::MAX) {
                    distances.insert(next_node, next_cost);
                    previous.insert(next_node, Some((current.node_id, *channel_id)));

                    heap.push(RouteNode {
                        node_id: next_node,
                        cost: next_cost,
                        carbon_footprint: current.carbon_footprint + channel.carbon_footprint,
                        renewable_percentage: (current.renewable_percentage
                            + graph.nodes[&next_node].renewable_percentage)
                            / 2.0,
                    });
                }
            }
        }

        Err(RoutingError::NoRouteFound)
    }

    fn calculate_edge_cost(
        &self,
        channel: &EnvironmentalChannel,
        node: &EnvironmentalNode,
        amount_sats: u64,
        params: &GreenRoutingParameters,
    ) -> f64 {
        // Fee component
        let fee_mnova =
            channel.base_fee_mnova as u64 + (amount_sats * channel.fee_rate_ppm as u64 / 1_000_000);
        let fee_cost = (fee_mnova as f64 / 1000.0) * params.fee_weight;

        // Carbon component
        let carbon_cost = channel.carbon_footprint * params.carbon_weight * 10000.0;

        // Renewable component (inverse - higher renewable = lower cost)
        let renewable_cost = (100.0 - node.renewable_percentage) * params.renewable_weight;

        // Reliability component
        let reliability_cost = (1.0 - channel.success_rate) * params.reliability_weight * 1000.0;

        // Apply bonuses
        let mut total_cost = fee_cost + carbon_cost + renewable_cost + reliability_cost;

        if node.green_certified {
            total_cost /= params.green_node_preference;
        }

        if node.carbon_negative {
            total_cost /= params.carbon_negative_bonus;
        }

        total_cost
    }

    fn reconstruct_green_route(
        &self,
        source: NodeId,
        destination: NodeId,
        previous: &HashMap<NodeId, Option<(NodeId, ChannelId)>>,
        graph: &EnvironmentalNetworkGraph,
        amount_nova_units: u64,
    ) -> Result<GreenLightningRoute, RoutingError> {
        let mut hops = Vec::new();
        let mut current = destination;
        let mut total_fees = 0u64;
        let mut total_carbon = 0.0;

        // Reconstruct path
        while current != source {
            if let Some(Some((prev_node, channel_id))) = previous.get(&current) {
                let channel = &graph.channels[channel_id];
                let node = &graph.nodes[&current];

                let fee_mnova = channel.base_fee_mnova as u64
                    + (amount_nova_units * channel.fee_rate_ppm as u64 / 1_000_000);

                hops.push(GreenRouteHop {
                    node_pubkey: node.public_key.clone(),
                    channel_id: *channel_id,
                    fee_nova_units: fee_mnova / 1000,
                    renewable_percentage: node.renewable_percentage,
                    carbon_footprint: channel.carbon_footprint,
                    green_certified: node.green_certified,
                });

                total_fees += fee_mnova / 1000;
                total_carbon += channel.carbon_footprint;
                current = *prev_node;
            } else {
                return Err(RoutingError::NoRouteFound);
            }
        }

        hops.reverse();

        // Calculate averages
        let avg_renewable =
            hops.iter().map(|h| h.renewable_percentage).sum::<f64>() / hops.len() as f64;

        let green_count = hops.iter().filter(|h| h.green_certified).count();

        // Calculate route score
        let route_score = self.calculate_route_score(&hops, total_carbon, avg_renewable);

        Ok(GreenLightningRoute {
            hops,
            total_capacity_nova_units: amount_nova_units,
            total_fees_nova_units: total_fees,
            total_carbon_footprint: total_carbon,
            average_renewable_percentage: avg_renewable,
            green_nodes_count: green_count,
            route_score,
        })
    }

    fn calculate_route_score(
        &self,
        hops: &[GreenRouteHop],
        total_carbon: f64,
        avg_renewable: f64,
    ) -> f64 {
        let mut score = 50.0; // Base score

        // Carbon impact (up to -25/+25 points)
        if total_carbon < 0.0 {
            score += 25.0; // Carbon negative bonus
        } else {
            score -= total_carbon * 2500.0; // Penalty for carbon
        }

        // Renewable percentage (up to 25 points)
        score += avg_renewable * 0.25;

        // Green node percentage (up to 10 points)
        let green_percentage =
            hops.iter().filter(|h| h.green_certified).count() as f64 / hops.len() as f64;
        score += green_percentage * 10.0;

        // Route length penalty
        score -= (hops.len() as f64 - 1.0) * 2.0;

        score.max(0.0).min(100.0)
    }

    fn get_cached_route(&self, key: &RouteCacheKey) -> Option<GreenLightningRoute> {
        let cache = self.route_cache.read().unwrap();

        if let Some(cached) = cache.get(key) {
            if cached.expires_at > Utc::now() {
                return Some(cached.route.clone());
            }
        }

        None
    }

    fn cache_route(&self, key: RouteCacheKey, route: GreenLightningRoute) {
        let mut cache = self.route_cache.write().unwrap();

        cache.insert(
            key,
            CachedRoute {
                route,
                cached_at: Utc::now(),
                expires_at: Utc::now() + chrono::Duration::minutes(5),
            },
        );
    }

    fn update_routing_metrics(&self, route: &GreenLightningRoute) {
        let mut metrics = self.metrics.write().unwrap();

        metrics.total_routes_calculated += 1;

        if route.average_renewable_percentage >= 75.0 {
            metrics.green_routes_found += 1;
        }

        if route.total_carbon_footprint < 0.0 {
            metrics.carbon_negative_routes += 1;
        }

        // Update average
        let old_total =
            metrics.average_carbon_per_route * (metrics.total_routes_calculated - 1) as f64;
        metrics.average_carbon_per_route =
            (old_total + route.total_carbon_footprint) / metrics.total_routes_calculated as f64;

        // Track savings
        let traditional_carbon = 0.01; // Estimated traditional payment carbon
        metrics.total_carbon_saved += traditional_carbon - route.total_carbon_footprint;
    }

    fn calculate_milestones(&self, carbon_saved: f64) -> Vec<String> {
        let mut milestones = Vec::new();

        if carbon_saved >= 1000.0 {
            milestones.push("🏆 1 Tonne CO2 Saved!".to_string());
        }
        if carbon_saved >= 10000.0 {
            milestones.push("🌍 10 Tonnes CO2 Saved!".to_string());
        }
        if carbon_saved >= 100000.0 {
            milestones.push("🌟 100 Tonnes CO2 Saved!".to_string());
        }

        milestones
    }

    fn generate_certificate_id(&self, payment_id: &str) -> String {
        use sha2::{Digest, Sha256};
        let mut hasher = Sha256::new();
        hasher.update(payment_id.as_bytes());
        hasher.update(Utc::now().timestamp().to_string().as_bytes());
        format!("GPC-{}", hex::encode(&hasher.finalize()[..8]))
    }

    fn generate_verification_hash(
        &self,
        payment_id: &str,
        impact: &PaymentEnvironmentalImpact,
    ) -> String {
        use sha2::{Digest, Sha256};
        let mut hasher = Sha256::new();
        hasher.update(payment_id.as_bytes());
        hasher.update(impact.total_carbon_kg.to_string().as_bytes());
        hasher.update(impact.renewable_percentage.to_string().as_bytes());
        hex::encode(hasher.finalize())
    }

    fn get_top_green_nodes(
        &self,
        graph: &EnvironmentalNetworkGraph,
        count: usize,
    ) -> Vec<TopGreenNode> {
        let mut nodes: Vec<_> = graph.nodes.values().filter(|n| n.green_certified).collect();

        nodes.sort_by(|a, b| {
            b.environmental_score
                .partial_cmp(&a.environmental_score)
                .unwrap_or(Ordering::Equal)
        });

        nodes
            .into_iter()
            .take(count)
            .map(|n| TopGreenNode {
                alias: n.alias.clone(),
                renewable_percentage: n.renewable_percentage,
                carbon_saved: n.monthly_carbon_saved,
                is_carbon_negative: n.carbon_negative,
            })
            .collect()
    }
}

/// Route node for priority queue
#[derive(Debug, Clone)]
struct RouteNode {
    node_id: NodeId,
    cost: f64,
    carbon_footprint: f64,
    renewable_percentage: f64,
}

impl Eq for RouteNode {}

impl PartialEq for RouteNode {
    fn eq(&self, other: &Self) -> bool {
        self.cost == other.cost
    }
}

impl Ord for RouteNode {
    fn cmp(&self, other: &Self) -> Ordering {
        other
            .cost
            .partial_cmp(&self.cost)
            .unwrap_or(Ordering::Equal)
    }
}

impl PartialOrd for RouteNode {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

/// Environmental routing preferences
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnvironmentalRoutingPreferences {
    pub priority: RoutingPriority,
    pub max_carbon_footprint: Option<f64>,
    pub min_renewable_percentage: Option<f64>,
    pub prefer_carbon_negative: bool,
    pub max_hops: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RoutingPriority {
    LowestCarbon,
    HighestRenewable,
    Balanced,
    CheapestGreen,
}

/// Time period for statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TimePeriod {
    Hour,
    Day,
    Week,
    Month,
    Year,
}

/// Green incentive program
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GreenIncentiveProgram {
    pub base_tier: GreenIncentiveTier,
    pub silver_tier: GreenIncentiveTier,
    pub gold_tier: GreenIncentiveTier,
    pub network_stats: NetworkGreenStats,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GreenIncentiveTier {
    pub name: String,
    pub min_renewable: f64,
    pub fee_discount: f64,
    pub routing_preference: f64,
    pub badge: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkGreenStats {
    pub total_nodes: usize,
    pub green_nodes: usize,
    pub carbon_negative_nodes: usize,
    pub average_renewable_percentage: f64,
    pub total_carbon_saved: f64,
}

/// Payment environmental impact
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaymentEnvironmentalImpact {
    pub payment_amount_sats: u64,
    pub route_length: usize,
    pub total_carbon_kg: f64,
    pub carbon_saved_kg: f64,
    pub renewable_percentage: f64,
    pub green_nodes_used: usize,
    pub trees_equivalent: u32,
    pub miles_equivalent: u32,
    pub is_carbon_negative: bool,
}

/// Environmental savings report
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnvironmentalSavingsReport {
    pub time_period: TimePeriod,
    pub total_payments: u64,
    pub green_payments: u64,
    pub carbon_negative_payments: u64,
    pub total_carbon_saved_kg: f64,
    pub average_carbon_per_payment: f64,
    pub network_renewable_percentage: f64,
    pub environmental_milestones: Vec<String>,
}

/// Green payment certificate
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GreenPaymentCertificate {
    pub certificate_id: String,
    pub payment_id: String,
    pub issued_at: DateTime<Utc>,
    pub carbon_saved_kg: f64,
    pub renewable_percentage: f64,
    pub green_nodes_used: usize,
    pub achievement_level: String,
    pub verification_hash: String,
}

/// Environmental Lightning statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnvironmentalLightningStats {
    pub timestamp: DateTime<Utc>,
    pub total_green_routes: u64,
    pub total_carbon_saved_kg: f64,
    pub carbon_negative_routes_percentage: f64,
    pub network_renewable_average: f64,
    pub top_green_nodes: Vec<TopGreenNode>,
    pub zone_statistics: Vec<ZoneEnvironmentalStats>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TopGreenNode {
    pub alias: String,
    pub renewable_percentage: f64,
    pub carbon_saved: f64,
    pub is_carbon_negative: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ZoneEnvironmentalStats {
    pub region: Region,
    pub green_nodes_percentage: f64,
    pub average_renewable: f64,
    pub carbon_intensity: f64,
}

/// Routing error types
#[derive(Debug, Clone)]
pub enum RoutingError {
    NoRouteFound,
    InsufficientCapacity,
    MaxCarbonExceeded,
    MinRenewableNotMet,
    NetworkError(String),
}

/// Public API functions

pub fn calculate_route_carbon_footprint(
    router: &GreenLightningRouter,
    route: &GreenLightningRoute,
) -> f64 {
    router.calculate_route_carbon_footprint(route)
}

pub async fn optimize_for_renewable_energy_nodes(
    router: &GreenLightningRouter,
    source: [u8; 33],
    destination: [u8; 33],
    amount_sats: u64,
) -> Result<GreenLightningRoute, RoutingError> {
    router
        .optimize_for_renewable_energy_nodes(source, destination, amount_sats)
        .await
}

pub fn apply_environmental_routing_preferences(
    router: &GreenLightningRouter,
    preferences: EnvironmentalRoutingPreferences,
) -> Result<(), RoutingError> {
    router.apply_environmental_routing_preferences(preferences)
}

pub fn incentivize_green_lightning_nodes(router: &GreenLightningRouter) -> GreenIncentiveProgram {
    router.incentivize_green_lightning_nodes()
}
