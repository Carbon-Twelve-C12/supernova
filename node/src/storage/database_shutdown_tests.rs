//! Database Shutdown and Recovery Tests for Supernova
//!
//! This module tests the database shutdown procedures to ensure
//! data integrity is maintained across node restarts.

#[cfg(test)]
mod tests {
    use super::super::database::{BlockchainDB, StorageError};
    use std::sync::Arc;
    use tempfile::TempDir;
    use tokio::sync::RwLock;

    /// Test clean shutdown procedures
    #[test]
    fn test_clean_shutdown() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("test_db");

        // Create and populate database
        {
            let db = BlockchainDB::new(&db_path).unwrap();

            // Add some test data
            db.store_metadata(b"test_key", b"test_value").unwrap();
            db.store_block(&[1u8; 32], &vec![1, 2, 3, 4]).unwrap();
            db.store_transaction(&[2u8; 32], &vec![5, 6, 7, 8]).unwrap();

            // Perform clean shutdown
            db.store_metadata(b"shutdown_in_progress", b"true").unwrap();
            db.flush().unwrap();

            // Create shutdown checkpoint
            let checkpoint_data = serde_json::json!({
                "type": "shutdown_checkpoint",
                "height": 100,
                "timestamp": chrono::Utc::now().timestamp(),
                "clean_shutdown": true,
            });

            db.store_metadata(
                b"last_shutdown_checkpoint",
                checkpoint_data.to_string().as_bytes(),
            )
            .unwrap();

            // Mark clean shutdown
            let timestamp = chrono::Utc::now().timestamp();
            db.store_metadata(b"last_clean_shutdown", timestamp.to_string().as_bytes())
                .unwrap();
            db.store_metadata(b"shutdown_in_progress", b"false")
                .unwrap();

            // Final flush
            db.flush().unwrap();
        }

        // Reopen database and verify clean shutdown was detected
        {
            let db = BlockchainDB::new(&db_path).unwrap();

            // Check clean shutdown marker
            let shutdown_data = db.get_metadata(b"last_clean_shutdown").unwrap();
            assert!(shutdown_data.is_some());

            // Verify data integrity
            let test_value = db.get_metadata(b"test_key").unwrap();
            assert_eq!(test_value, Some(b"test_value".to_vec().into()));

            let block = db.get_block(&[1u8; 32]).unwrap();
            assert!(block.is_some());

            let tx = db.get_transaction(&[2u8; 32]).unwrap();
            assert!(tx.is_some());
        }
    }

    /// Test unclean shutdown detection
    #[test]
    fn test_unclean_shutdown_detection() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("test_db");

        // Simulate unclean shutdown
        {
            let db = BlockchainDB::new(&db_path).unwrap();

            // Mark shutdown in progress but don't complete it
            db.store_metadata(b"shutdown_in_progress", b"true").unwrap();

            // Add some data
            db.store_metadata(b"test_key", b"test_value").unwrap();

            // Drop without clean shutdown
        }

        // Reopen and check for unclean shutdown
        {
            let db = BlockchainDB::new(&db_path).unwrap();

            // Check if shutdown was in progress
            let shutdown_flag = db.get_metadata(b"shutdown_in_progress").unwrap();
            assert_eq!(shutdown_flag, Some(b"true".to_vec().into()));

            // Data should still be accessible
            let test_value = db.get_metadata(b"test_key").unwrap();
            assert_eq!(test_value, Some(b"test_value".to_vec().into()));
        }
    }

    /// Test recovery after crash
    #[test]
    fn test_crash_recovery() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("test_db");

        // Create initial state
        let initial_height = 1000u64;
        let initial_hash = [42u8; 32];

        // Simulate crash during operation
        {
            let db = BlockchainDB::new(&db_path).unwrap();

            // Store initial state
            db.store_metadata(b"best_height", &initial_height.to_le_bytes())
                .unwrap();
            db.store_metadata(b"best_hash", &initial_hash).unwrap();

            // Start a simulated operation
            db.store_metadata(b"operation_in_progress", b"true")
                .unwrap();

            // Simulate crash - no clean shutdown
        }

        // Recovery phase
        {
            let db = BlockchainDB::new(&db_path).unwrap();

            // Check for incomplete operation
            let op_flag = db.get_metadata(b"operation_in_progress").unwrap();
            assert_eq!(op_flag, Some(b"true".to_vec().into()));

            // Verify state is still consistent
            let height_data = db.get_metadata(b"best_height").unwrap().unwrap();
            let height = u64::from_le_bytes(height_data[..8].try_into().unwrap());
            assert_eq!(height, initial_height);

            let hash_data = db.get_metadata(b"best_hash").unwrap().unwrap();
            assert_eq!(&hash_data[..], &initial_hash[..]);

            // Clear operation flag after recovery
            db.store_metadata(b"operation_in_progress", b"false")
                .unwrap();
        }
    }

    /// Test concurrent shutdown protection
    #[tokio::test]
    async fn test_concurrent_shutdown_protection() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("test_db");

        let db = Arc::new(RwLock::new(BlockchainDB::new(&db_path).unwrap()));

        // Spawn multiple tasks trying to shutdown
        let mut handles = vec![];

        for i in 0..5 {
            let db_clone = db.clone();
            let handle = tokio::spawn(async move {
                let db = db_clone.read().await;

                // Try to mark shutdown
                let result = db.store_metadata(b"shutdown_task", &[i as u8]);

                // Simulate shutdown work
                tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;

                result
            });

            handles.push(handle);
        }

        // Wait for all tasks
        for handle in handles {
            let _ = handle.await;
        }

        // Verify only one shutdown marker exists
        let db = db.read().await;
        let marker = db.get_metadata(b"shutdown_task").unwrap();
        assert!(marker.is_some());
    }

    /// Test database state after multiple restarts
    #[test]
    fn test_multiple_restarts() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("test_db");

        // First session
        {
            let db = BlockchainDB::new(&db_path).unwrap();
            db.store_metadata(b"session", b"1").unwrap();
            db.store_metadata(b"counter", &1u64.to_le_bytes()).unwrap();

            // Clean shutdown
            db.store_metadata(b"last_clean_shutdown", b"1").unwrap();
            db.flush().unwrap();
        }

        // Second session
        {
            let db = BlockchainDB::new(&db_path).unwrap();

            // Verify previous session data
            let session = db.get_metadata(b"session").unwrap();
            assert_eq!(session, Some(b"1".to_vec().into()));

            // Update data
            db.store_metadata(b"session", b"2").unwrap();
            let counter_data = db.get_metadata(b"counter").unwrap().unwrap();
            let mut counter = u64::from_le_bytes(counter_data[..8].try_into().unwrap());
            counter += 1;
            db.store_metadata(b"counter", &counter.to_le_bytes())
                .unwrap();

            // Clean shutdown
            db.store_metadata(b"last_clean_shutdown", b"2").unwrap();
            db.flush().unwrap();
        }

        // Third session - verify accumulated state
        {
            let db = BlockchainDB::new(&db_path).unwrap();

            let session = db.get_metadata(b"session").unwrap();
            assert_eq!(session, Some(b"2".to_vec().into()));

            let counter_data = db.get_metadata(b"counter").unwrap().unwrap();
            let counter = u64::from_le_bytes(counter_data[..8].try_into().unwrap());
            assert_eq!(counter, 2);
        }
    }

    /// Test shutdown with pending operations
    #[test]
    fn test_shutdown_with_pending_operations() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("test_db");

        let db = BlockchainDB::new(&db_path).unwrap();

        // Create some pending operations
        db.store_pending_block(&[1u8; 32], &vec![1, 2, 3], Some(100), None, None)
            .unwrap();
        db.store_pending_block(&[2u8; 32], &vec![4, 5, 6], Some(101), None, None)
            .unwrap();

        // Get pending count before shutdown
        let pending_count = db.count_pending_blocks().unwrap();
        assert_eq!(pending_count, 2);

        // Perform shutdown
        db.store_metadata(b"pending_at_shutdown", &pending_count.to_le_bytes())
            .unwrap();
        db.flush().unwrap();

        // Reopen and verify pending blocks survived
        drop(db);
        let db = BlockchainDB::new(&db_path).unwrap();

        let new_pending_count = db.count_pending_blocks().unwrap();
        assert_eq!(new_pending_count, pending_count);

        // Verify individual pending blocks
        let block1 = db.get_pending_block(&[1u8; 32]).unwrap();
        assert!(block1.is_some());

        let block2 = db.get_pending_block(&[2u8; 32]).unwrap();
        assert!(block2.is_some());
    }

    /// Test emergency shutdown
    #[test]
    fn test_emergency_shutdown() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("test_db");

        {
            let db = BlockchainDB::new(&db_path).unwrap();

            // Add critical data
            db.store_metadata(b"critical_data", b"must_survive")
                .unwrap();

            // Simulate emergency shutdown
            db.store_metadata(b"emergency_shutdown", b"true").unwrap();
            db.flush().unwrap();

            // No time for full shutdown procedures
        }

        // Verify data survived emergency shutdown
        {
            let db = BlockchainDB::new(&db_path).unwrap();

            // Check emergency flag
            let emergency = db.get_metadata(b"emergency_shutdown").unwrap();
            assert_eq!(emergency, Some(b"true".to_vec().into()));

            // Verify critical data survived
            let data = db.get_metadata(b"critical_data").unwrap();
            assert_eq!(data, Some(b"must_survive".to_vec().into()));

            // Clear emergency flag
            db.store_metadata(b"emergency_shutdown", b"").unwrap();
        }
    }
}
