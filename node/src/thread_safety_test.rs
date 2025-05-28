#[cfg(test)]
mod thread_safety_tests {
    use super::super::node::Node;
    use super::super::thread_safety_fix::{ThreadSafeNode, NodeApiFacade};
    use std::sync::Arc;
    
    #[test]
    fn test_node_is_send() {
        fn assert_send<T: Send>() {}
        assert_send::<Node>();
    }
    
    #[test]
    fn test_node_is_sync() {
        fn assert_sync<T: Sync>() {}
        assert_sync::<Node>();
    }
    
    #[test]
    fn test_arc_node_is_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<Arc<Node>>();
    }
    
    #[test]
    fn test_node_can_be_shared_across_threads() {
        use std::thread;
        use crate::config::NodeConfig;
        
        let node = Arc::new(Node::new(NodeConfig::default()).unwrap());
        
        let node_clone = node.clone();
        let handle = thread::spawn(move || {
            // Access node in another thread
            let _ = node_clone.get_info();
        });
        
        handle.join().unwrap();
    }
    
    #[test]
    fn test_thread_safe_node_wrapper() {
        use crate::config::NodeConfig;
        
        let node = Node::new(NodeConfig::default()).unwrap();
        let safe_node = ThreadSafeNode::new(node);
        
        // Test that ThreadSafeNode is Send + Sync
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<ThreadSafeNode>();
    }
    
    #[test]
    fn test_node_api_facade_is_thread_safe() {
        use crate::config::NodeConfig;
        
        let node = Node::new(NodeConfig::default()).unwrap();
        let facade = NodeApiFacade::from_node(&node);
        
        // Test that facade is Send + Sync
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<NodeApiFacade>();
        
        // Test cloning
        let facade2 = facade.clone();
        assert!(facade2.get_info().is_ok());
    }
    
    #[tokio::test]
    async fn test_api_server_with_arc_node() {
        use crate::config::NodeConfig;
        use crate::api::ApiServer;
        
        let node = Arc::new(Node::new(NodeConfig::default()).unwrap());
        
        // This should compile without thread safety issues
        let _api_server = ApiServer::new(node.clone(), "127.0.0.1", 8080);
        
        // Test that we can access the node from multiple tasks
        let node2 = node.clone();
        let handle1 = tokio::spawn(async move {
            let _ = node2.get_info();
        });
        
        let node3 = node.clone();
        let handle2 = tokio::spawn(async move {
            let _ = node3.get_status();
        });
        
        handle1.await.unwrap();
        handle2.await.unwrap();
    }
    
    #[tokio::test]
    async fn test_concurrent_node_access() {
        use crate::config::NodeConfig;
        use tokio::sync::Barrier;
        use std::sync::Arc;
        
        let node = Arc::new(Node::new(NodeConfig::default()).unwrap());
        let barrier = Arc::new(Barrier::new(10));
        
        let mut handles = vec![];
        
        // Spawn 10 concurrent tasks accessing the node
        for i in 0..10 {
            let node_clone = node.clone();
            let barrier_clone = barrier.clone();
            
            let handle = tokio::spawn(async move {
                // Wait for all tasks to be ready
                barrier_clone.wait().await;
                
                // Access node methods concurrently
                match i % 4 {
                    0 => { let _ = node_clone.get_info(); },
                    1 => { let _ = node_clone.get_status(); },
                    2 => { let _ = node_clone.get_performance_metrics(); },
                    _ => { let _ = node_clone.get_network_stats(); },
                }
            });
            
            handles.push(handle);
        }
        
        // Wait for all tasks to complete
        for handle in handles {
            handle.await.unwrap();
        }
    }
} 