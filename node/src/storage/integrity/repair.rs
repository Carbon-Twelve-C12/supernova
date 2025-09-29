// integrity/repair.rs - Handles repairs for integrity issues
use std::sync::Arc;

use tokio::sync::Mutex;
use tracing::{debug, info, warn, error};

use crate::storage::BlockchainDB;
use crate::storage::StorageError;
use crate::storage::corruption::{CorruptionHandler, CorruptionType, RecoveryStrategy};
use crate::storage::integrity::models::{IntegrityConfig, IntegrityIssue, IssueType, IssueLocation, IssueSeverity};

/// Handles repairs for integrity issues
pub struct IntegrityRepairer {
    /// Reference to blockchain database
    db: Arc<BlockchainDB>,
    /// Corruption handler for implementing repairs
    corruption_handler: Arc<Mutex<CorruptionHandler>>,
    /// Configuration for repairs
    config: IntegrityConfig,
}

impl IntegrityRepairer {
    /// Create a new integrity repairer
    pub fn new(
        db: Arc<BlockchainDB>,
        corruption_handler: Arc<Mutex<CorruptionHandler>>,
        config: IntegrityConfig,
    ) -> Self {
        Self {
            db,
            corruption_handler,
            config,
        }
    }

    /// Attempt to repair a list of integrity issues
    pub async fn repair_issues(&self, issues: &[IntegrityIssue]) -> Result<usize, StorageError> {
        if issues.is_empty() {
            return Ok(0);
        }
        
        info!("Attempting to repair {} integrity issues", issues.len());
        
        // First, classify issues by type for more efficient repair
        let mut structure_issues = Vec::new();
        let mut chain_issues = Vec::new();
        let mut utxo_issues = Vec::new();
        let mut index_issues = Vec::new();
        
        // Group repairable issues by type
        for issue in issues {
            if !issue.repairable {
                continue;
            }
            
            match issue.issue_type {
                IssueType::Structure => structure_issues.push(issue),
                IssueType::ChainInconsistency | IssueType::MissingReference => chain_issues.push(issue),
                IssueType::UtxoInconsistency => utxo_issues.push(issue),
                IssueType::IndexInconsistency => index_issues.push(issue),
                _ => {} // Skip other types like CryptoVerification that can't be repaired
            }
        }
        
        // Track repair counts
        let mut total_repairs = 0;
        
        // Handle database structure issues first as they're most critical
        if !structure_issues.is_empty() {
            info!("Repairing {} database structure issues", structure_issues.len());
            let repaired = self.repair_structure_issues(&structure_issues).await?;
            total_repairs += repaired;
        }
        
        // Handle index inconsistencies next as they're relatively easy to fix
        if !index_issues.is_empty() {
            info!("Repairing {} index inconsistencies", index_issues.len());
            let repaired = self.repair_index_issues(&index_issues).await?;
            total_repairs += repaired;
        }
        
        // Handle chain inconsistencies
        if !chain_issues.is_empty() {
            info!("Repairing {} chain inconsistencies", chain_issues.len());
            let repaired = self.repair_chain_issues(&chain_issues).await?;
            total_repairs += repaired;
        }
        
        // Handle UTXO inconsistencies
        if !utxo_issues.is_empty() {
            info!("Repairing {} UTXO inconsistencies", utxo_issues.len());
            let repaired = self.repair_utxo_issues(&utxo_issues).await?;
            total_repairs += repaired;
        }
        
        info!("Completed repairs: {} issues fixed", total_repairs);
        Ok(total_repairs)
    }
    
    /// Repair database structure issues
    async fn repair_structure_issues(&self, issues: &[&IntegrityIssue]) -> Result<usize, StorageError> {
        let mut repaired = 0;
        
        for issue in issues {
            let corruption_type = self.map_issue_to_corruption_type(issue);
            let strategy = self.determine_repair_strategy(issue, &corruption_type);
            
            debug!("Repairing structure issue: {} using strategy {:?}", issue.description, strategy);
            
            let mut handler = self.corruption_handler.lock().await;
            match Self::execute_repair_strategy(&strategy, &corruption_type, &mut handler).await {
                Ok(true) => {
                    repaired += 1;
                    info!("Successfully repaired: {}", issue.description);
                },
                Ok(false) => {
                    warn!("Failed to repair: {}", issue.description);
                },
                Err(e) => {
                    error!("Error during repair: {}", e);
                }
            }
        }
        
        Ok(repaired)
    }
    
    /// Repair index inconsistencies
    async fn repair_index_issues(&self, issues: &[&IntegrityIssue]) -> Result<usize, StorageError> {
        let mut repaired = 0;
        
        // Group issues by affected index
        let mut block_height_issues = Vec::new();
        let mut tx_index_issues = Vec::new();
        
        for issue in issues {
            if let IssueLocation::Chain { .. } = issue.location {
                block_height_issues.push(issue);
            } else if let IssueLocation::Transaction { .. } = issue.location {
                tx_index_issues.push(issue);
            }
        }
        
        // Rebuild block height index if needed
        if !block_height_issues.is_empty() {
            let corruption_type = CorruptionType::IndexCorruption {
                primary_tree: "blocks".to_string(),
                index_tree: "block_height_index".to_string(),
                mismatched_keys: Vec::new(),
            };
            
            let strategy = RecoveryStrategy::RebuildIndexes {
                source_tree: "blocks".to_string(),
                target_index: "block_height_index".to_string(),
            };
            
            let mut handler = self.corruption_handler.lock().await;
            match Self::execute_repair_strategy(&strategy, &corruption_type, &mut handler).await {
                Ok(true) => {
                    repaired += block_height_issues.len();
                    info!("Successfully rebuilt block height index");
                },
                Ok(false) => {
                    warn!("Failed to rebuild block height index");
                },
                Err(e) => {
                    error!("Error rebuilding block height index: {}", e);
                }
            }
        }
        
        // Rebuild transaction index if needed
        if !tx_index_issues.is_empty() {
            let corruption_type = CorruptionType::IndexCorruption {
                primary_tree: "transactions".to_string(),
                index_tree: "tx_index".to_string(),
                mismatched_keys: Vec::new(),
            };
            
            let strategy = RecoveryStrategy::RebuildIndexes {
                source_tree: "transactions".to_string(),
                target_index: "tx_index".to_string(),
            };
            
            let mut handler = self.corruption_handler.lock().await;
            match Self::execute_repair_strategy(&strategy, &corruption_type, &mut handler).await {
                Ok(true) => {
                    repaired += tx_index_issues.len();
                    info!("Successfully rebuilt transaction index");
                },
                Ok(false) => {
                    warn!("Failed to rebuild transaction index");
                },
                Err(e) => {
                    error!("Error rebuilding transaction index: {}", e);
                }
            }
        }
        
        Ok(repaired)
    }
    
    /// Repair chain inconsistencies
    async fn repair_chain_issues(&self, issues: &[&IntegrityIssue]) -> Result<usize, StorageError> {
        let mut repaired = 0;
        
        // If we have checkpoint issues, address those first as they may fix multiple issues
        let checkpoint_issues: Vec<_> = issues.iter()
            .filter(|i| matches!(i.issue_type, IssueType::ChainInconsistency))
            .filter(|i| i.severity == IssueSeverity::Critical)
            .collect();
        
        if !checkpoint_issues.is_empty() {
            // Find an appropriate checkpoint to revert to
            let mut checkpoint_height = 0;
            
            for issue in &checkpoint_issues {
                if let IssueLocation::Chain { start_height, .. } = issue.location {
                    if let Some(height) = start_height {
                        // Find nearest checkpoint below this height
                        for (cp_height, _) in self.config.trusted_checkpoints.iter() {
                            if *cp_height < height && *cp_height > checkpoint_height {
                                checkpoint_height = *cp_height;
                            }
                        }
                    }
                }
            }
            
            if checkpoint_height > 0 {
                let corruption_type = CorruptionType::LogicalCorruption {
                    description: "Chain diverges from checkpoint".to_string(),
                    affected_range: Some((checkpoint_height, 0)), // Just need the start height
                };
                
                let strategy = RecoveryStrategy::RevertToCheckpoint {
                    checkpoint_height,
                };
                
                let mut handler = self.corruption_handler.lock().await;
                match Self::execute_repair_strategy(&strategy, &corruption_type, &mut handler).await {
                    Ok(true) => {
                        // This may fix many issues at once
                        repaired += checkpoint_issues.len();
                        info!("Successfully reverted to checkpoint at height {}", checkpoint_height);
                    },
                    Ok(false) => {
                        warn!("Failed to revert to checkpoint at height {}", checkpoint_height);
                    },
                    Err(e) => {
                        error!("Error reverting to checkpoint: {}", e);
                    }
                }
                
                // Return early as this major operation may have fixed multiple issues
                // A re-verification should be done afterwards
                return Ok(repaired);
            }
        }
        
        // Handle individual missing blocks
        let missing_block_issues: Vec<_> = issues.iter()
            .filter(|i| matches!(i.issue_type, IssueType::MissingReference))
            .collect();
        
        for issue in missing_block_issues {
            if let IssueLocation::Block { hash, .. } = issue.location {
                let corruption_type = CorruptionType::RecordCorruption {
                    tree_name: "blocks".to_string(),
                    affected_keys: vec![hash.to_vec()],
                };
                
                let strategy = RecoveryStrategy::RebuildCorruptedRecords {
                    tree_name: "blocks".to_string(),
                    keys: vec![hash.to_vec()],
                };
                
                let mut handler = self.corruption_handler.lock().await;
                match Self::execute_repair_strategy(&strategy, &corruption_type, &mut handler).await {
                    Ok(true) => {
                        repaired += 1;
                        info!("Successfully repaired missing block {}", hex::encode(&hash[..8]));
                    },
                    Ok(false) => {
                        warn!("Failed to repair missing block {}", hex::encode(&hash[..8]));
                    },
                    Err(e) => {
                        error!("Error repairing missing block: {}", e);
                    }
                }
            }
        }
        
        Ok(repaired)
    }
    
    /// Repair UTXO inconsistencies
    async fn repair_utxo_issues(&self, issues: &[&IntegrityIssue]) -> Result<usize, StorageError> {
        let mut repaired = 0;
        
        // Check if we have critical/many UTXO issues that warrant rebuilding the entire set
        let critical_utxo_issues = issues.iter()
            .filter(|i| i.severity >= IssueSeverity::Error)
            .count();
        
        if critical_utxo_issues > 5 || issues.len() > 20 {
            // Rebuild the entire UTXO set
            info!("Rebuilding entire UTXO set due to {} issues", issues.len());
            
            let corruption_type = CorruptionType::RecordCorruption {
                tree_name: "utxos".to_string(),
                affected_keys: Vec::new(),
            };
            
            let strategy = RecoveryStrategy::RebuildChainState {
                start_height: 0,
                end_height: None,
            };
            
            let mut handler = self.corruption_handler.lock().await;
            match Self::execute_repair_strategy(&strategy, &corruption_type, &mut handler).await {
                Ok(true) => {
                    repaired += issues.len();
                    info!("Successfully rebuilt entire UTXO set");
                },
                Ok(false) => {
                    warn!("Failed to rebuild UTXO set");
                },
                Err(e) => {
                    error!("Error rebuilding UTXO set: {}", e);
                }
            }
            
            return Ok(repaired);
        }
        
        // Handle individual UTXO issues
        for issue in issues {
            if let IssueLocation::Utxo { tx_hash, output_index } = issue.location {
                let utxo_key = Self::create_utxo_key(&tx_hash, output_index);
                
                let corruption_type = CorruptionType::RecordCorruption {
                    tree_name: "utxos".to_string(),
                    affected_keys: vec![utxo_key.clone()],
                };
                
                let strategy = RecoveryStrategy::RebuildCorruptedRecords {
                    tree_name: "utxos".to_string(),
                    keys: vec![utxo_key],
                };
                
                let mut handler = self.corruption_handler.lock().await;
                match Self::execute_repair_strategy(&strategy, &corruption_type, &mut handler).await {
                    Ok(true) => {
                        repaired += 1;
                        info!("Successfully repaired UTXO {}:{}", hex::encode(&tx_hash[..8]), output_index);
                    },
                    Ok(false) => {
                        warn!("Failed to repair UTXO {}:{}", hex::encode(&tx_hash[..8]), output_index);
                    },
                    Err(e) => {
                        error!("Error repairing UTXO: {}", e);
                    }
                }
            }
        }
        
        Ok(repaired)
    }
    
    /// Map integrity issue to corruption type for repair
    fn map_issue_to_corruption_type(&self, issue: &IntegrityIssue) -> CorruptionType {
        match &issue.issue_type {
            IssueType::Structure => {
                // Extract component name from location
                if let IssueLocation::Database { component } = &issue.location {
                    CorruptionType::RecordCorruption {
                        tree_name: component.clone(),
                        affected_keys: Vec::new(),
                    }
                } else {
                    CorruptionType::FileLevelCorruption
                }
            },
            IssueType::IndexInconsistency => {
                let (primary_tree, index_tree) = match &issue.location {
                    IssueLocation::Chain { .. } => ("blocks", "block_height_index"),
                    IssueLocation::Transaction { .. } => ("transactions", "tx_index"),
                    _ => ("blocks", "block_height_index"), // Default
                };
                
                CorruptionType::IndexCorruption {
                    primary_tree: primary_tree.to_string(),
                    index_tree: index_tree.to_string(),
                    mismatched_keys: Vec::new(),
                }
            },
            IssueType::ChainInconsistency => {
                let affected_range = match &issue.location {
                    IssueLocation::Chain { start_height, end_height } => {
                        match (start_height, end_height) {
                            (Some(s), Some(e)) => Some((*s, *e)),
                            (Some(s), None) => Some((*s, *s + 100)),
                            (None, Some(e)) => Some((*e - 100, *e)),
                            (None, None) => None,
                        }
                    },
                    IssueLocation::Block { height, .. } => {
                        height.map(|h| (h, h))
                    },
                    _ => None,
                };
                
                CorruptionType::LogicalCorruption {
                    description: issue.description.clone(),
                    affected_range,
                }
            },
            IssueType::MissingReference => {
                match &issue.location {
                    IssueLocation::Block { hash, .. } => {
                        CorruptionType::RecordCorruption {
                            tree_name: "blocks".to_string(),
                            affected_keys: vec![hash.to_vec()],
                        }
                    },
                    IssueLocation::Transaction { hash, .. } => {
                        CorruptionType::RecordCorruption {
                            tree_name: "transactions".to_string(),
                            affected_keys: vec![hash.to_vec()],
                        }
                    },
                    _ => CorruptionType::FileLevelCorruption,
                }
            },
            IssueType::UtxoInconsistency => {
                if let IssueLocation::Utxo { tx_hash, output_index } = &issue.location {
                    let utxo_key = Self::create_utxo_key(tx_hash, *output_index);
                    
                    CorruptionType::RecordCorruption {
                        tree_name: "utxos".to_string(),
                        affected_keys: vec![utxo_key],
                    }
                } else {
                    CorruptionType::RecordCorruption {
                        tree_name: "utxos".to_string(),
                        affected_keys: Vec::new(),
                    }
                }
            },
            _ => CorruptionType::FileLevelCorruption, // Default for unknown cases
        }
    }
    
    /// Determine repair strategy for an issue
    fn determine_repair_strategy(&self, issue: &IntegrityIssue, corruption_type: &CorruptionType) -> RecoveryStrategy {
        match corruption_type {
            CorruptionType::FileLevelCorruption => {
                RecoveryStrategy::RestoreFromBackup
            },
            CorruptionType::RecordCorruption { tree_name, affected_keys } => {
                if affected_keys.is_empty() {
                    // For entire tree rebuilds
                    match tree_name.as_str() {
                        "utxos" => RecoveryStrategy::RebuildChainState {
                            start_height: 0,
                            end_height: None,
                        },
                        _ => RecoveryStrategy::RestoreFromBackup,
                    }
                } else {
                    // For specific records
                    RecoveryStrategy::RebuildCorruptedRecords {
                        tree_name: tree_name.clone(),
                        keys: affected_keys.clone(),
                    }
                }
            },
            CorruptionType::IndexCorruption { primary_tree, index_tree, .. } => {
                RecoveryStrategy::RebuildIndexes {
                    source_tree: primary_tree.clone(),
                    target_index: index_tree.clone(),
                }
            },
            CorruptionType::LogicalCorruption { affected_range, .. } => {
                // Try to find a checkpoint
                if let Some((start_height, _)) = affected_range {
                    // Find nearest checkpoint below this height
                    let mut checkpoint_height = 0;
                    
                    for (cp_height, _) in self.config.trusted_checkpoints.iter() {
                        if *cp_height < *start_height && *cp_height > checkpoint_height {
                            checkpoint_height = *cp_height;
                        }
                    }
                    
                    if checkpoint_height > 0 {
                        RecoveryStrategy::RevertToCheckpoint {
                            checkpoint_height,
                        }
                    } else {
                        RecoveryStrategy::RebuildChainState {
                            start_height: *start_height,
                            end_height: None,
                        }
                    }
                } else {
                    RecoveryStrategy::RestoreFromBackup
                }
            },
        }
    }
    
    /// Execute a repair strategy using the corruption handler
    async fn execute_repair_strategy(
        strategy: &RecoveryStrategy, 
        corruption_type: &CorruptionType,
        handler: &mut CorruptionHandler,
    ) -> Result<bool, StorageError> {
        match strategy {
            RecoveryStrategy::RestoreFromBackup => {
                match handler.repair_from_backup().await {
                    Ok((success, _, _, _)) => Ok(success),
                    Err(e) => Err(map_corruption_error(e)),
                }
            },
            RecoveryStrategy::RebuildCorruptedRecords { tree_name, keys } => {
                match handler.rebuild_corrupted_records(tree_name, keys).await {
                    Ok((success, _, _, _)) => Ok(success),
                    Err(e) => Err(map_corruption_error(e)),
                }
            },
            RecoveryStrategy::RebuildIndexes { source_tree, target_index } => {
                match handler.rebuild_indexes(source_tree, target_index).await {
                    Ok((success, _, _, _)) => Ok(success),
                    Err(e) => Err(map_corruption_error(e)),
                }
            },
            RecoveryStrategy::RevertToCheckpoint { checkpoint_height } => {
                match handler.revert_to_checkpoint(*checkpoint_height).await {
                    Ok((success, _, _, _)) => Ok(success),
                    Err(e) => Err(map_corruption_error(e)),
                }
            },
            RecoveryStrategy::RebuildChainState { start_height, end_height } => {
                match handler.rebuild_chain_state(*start_height, *end_height).await {
                    Ok((success, _, _, _)) => Ok(success),
                    Err(e) => Err(map_corruption_error(e)),
                }
            },
        }
    }
    
    /// Create a UTXO key from transaction hash and output index
    fn create_utxo_key(tx_hash: &[u8; 32], index: u32) -> Vec<u8> {
        let mut key = Vec::with_capacity(36);
        key.extend_from_slice(tx_hash);
        key.extend_from_slice(&index.to_be_bytes());
        key
    }
}

/// Map corruption error to storage error
fn map_corruption_error(err: impl std::error::Error) -> StorageError {
    StorageError::DatabaseError(err.to_string())
}