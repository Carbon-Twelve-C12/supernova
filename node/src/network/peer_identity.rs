// Persistent Peer Identity Management
// Ensures nodes maintain stable peer IDs across restarts

use libp2p::identity::Keypair;
use std::path::Path;
use thiserror::Error;
use tracing::{info, warn};

#[derive(Error, Debug)]
pub enum IdentityError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    
    #[error("Failed to decode keypair: {0}")]
    DecodeError(String),
    
    #[error("Failed to encode keypair: {0}")]
    EncodeError(String),
}

/// Load or generate persistent peer keypair
///
/// This function ensures the node maintains a stable peer ID across restarts
/// by saving the keypair to disk on first run and loading it on subsequent runs.
///
/// # Arguments
/// * `data_dir` - Directory to store the peer_id.key file
///
/// # Returns
/// * `Keypair` - The persistent ed25519 keypair for this node
///
/// # Security
/// - File is created with 0600 permissions (owner read/write only)
/// - Uses libp2p's protobuf encoding for keypair serialization
/// - Validates keypair can be decoded before returning
pub fn load_or_generate_keypair(data_dir: &Path) -> Result<Keypair, IdentityError> {
    let keypair_path = data_dir.join("peer_id.key");
    
    // Try to load existing keypair
    if keypair_path.exists() {
        match load_keypair_from_file(&keypair_path) {
            Ok(keypair) => {
                let peer_id = libp2p::PeerId::from(keypair.public());
                info!("✓ Loaded persistent peer ID from {:?}", keypair_path);
                info!("  Peer ID: {}", peer_id);
                return Ok(keypair);
            }
            Err(e) => {
                warn!("Failed to load keypair from {:?}: {}", keypair_path, e);
                warn!("Generating new keypair...");
            }
        }
    }
    
    // Generate new keypair
    let keypair = Keypair::generate_ed25519();
    let peer_id = libp2p::PeerId::from(keypair.public());
    
    // Save to disk
    if let Err(e) = save_keypair_to_file(&keypair, &keypair_path) {
        warn!("Failed to save keypair to {:?}: {}", keypair_path, e);
        warn!("Peer ID will not persist across restarts!");
    } else {
        info!("✓ Generated new peer ID, saved to {:?}", keypair_path);
        info!("  Peer ID: {}", peer_id);
    }
    
    Ok(keypair)
}

/// Load keypair from file
fn load_keypair_from_file(path: &Path) -> Result<Keypair, IdentityError> {
    let bytes = std::fs::read(path)?;
    
    Keypair::from_protobuf_encoding(&bytes)
        .map_err(|e| IdentityError::DecodeError(e.to_string()))
}

/// Save keypair to file with secure permissions
fn save_keypair_to_file(keypair: &Keypair, path: &Path) -> Result<(), IdentityError> {
    let bytes = keypair.to_protobuf_encoding()
        .map_err(|e| IdentityError::EncodeError(e.to_string()))?;
    
    // Create parent directory if needed
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    
    // Write keypair to file
    std::fs::write(path, &bytes)?;
    
    // Set secure permissions (Unix only)
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = std::fs::metadata(path)?.permissions();
        perms.set_mode(0o600); // Owner read/write only
        std::fs::set_permissions(path, perms)?;
    }
    
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    
    #[test]
    fn test_generate_and_load_keypair() {
        let temp_dir = TempDir::new().unwrap();
        let data_dir = temp_dir.path();
        
        // First call should generate
        let keypair1 = load_or_generate_keypair(data_dir).unwrap();
        let peer_id1 = libp2p::PeerId::from(keypair1.public());
        
        // Second call should load same keypair
        let keypair2 = load_or_generate_keypair(data_dir).unwrap();
        let peer_id2 = libp2p::PeerId::from(keypair2.public());
        
        // Peer IDs should match
        assert_eq!(peer_id1, peer_id2);
    }
    
    #[test]
    fn test_keypair_file_created() {
        let temp_dir = TempDir::new().unwrap();
        let data_dir = temp_dir.path();
        
        load_or_generate_keypair(data_dir).unwrap();
        
        // File should exist
        let keypair_path = data_dir.join("peer_id.key");
        assert!(keypair_path.exists());
    }
    
    #[cfg(unix)]
    #[test]
    fn test_secure_file_permissions() {
        use std::os::unix::fs::PermissionsExt;
        
        let temp_dir = TempDir::new().unwrap();
        let data_dir = temp_dir.path();
        
        load_or_generate_keypair(data_dir).unwrap();
        
        let keypair_path = data_dir.join("peer_id.key");
        let perms = std::fs::metadata(&keypair_path).unwrap().permissions();
        let mode = perms.mode() & 0o777;
        
        // Should be 0600 (owner read/write only)
        assert_eq!(mode, 0o600);
    }
}

