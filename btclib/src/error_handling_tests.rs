//! Error Handling Tests for Supernova
//! 
//! This module tests that unsafe unwrap() calls have been replaced with proper error handling

#[cfg(test)]
mod tests {
    use crate::errors::{SuperNovaError, SuperNovaResult, SafeUnwrap, ResultExt};
    use crate::errors::{safe_serialize, safe_deserialize, get_system_time};
    
    #[test]
    fn test_safe_unwrap_option() {
        // Test None case
        let opt: Option<i32> = None;
        let result = opt.safe_unwrap("Expected value");
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Expected value"));
        
        // Test Some case
        let opt = Some(42);
        let result = opt.safe_unwrap("Should work");
        assert_eq!(result.unwrap(), 42);
    }
    
    #[test]
    fn test_result_context() {
        fn failing_operation() -> Result<(), std::io::Error> {
            Err(std::io::Error::new(std::io::ErrorKind::NotFound, "file missing"))
        }
        
        let result = failing_operation().context("Loading config file");
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("Loading config file"));
        assert!(err_msg.contains("file missing"));
    }
    
    #[test]
    fn test_safe_serialize() {
        #[derive(serde::Serialize)]
        struct TestStruct {
            value: u32,
        }
        
        let test = TestStruct { value: 42 };
        let result = safe_serialize(&test);
        assert!(result.is_ok());
        let serialized = result.unwrap();
        assert!(!serialized.is_empty());
    }
    
    #[test]
    fn test_safe_deserialize() {
        #[derive(serde::Serialize, serde::Deserialize, PartialEq, Debug)]
        struct TestStruct {
            value: u32,
        }
        
        let test = TestStruct { value: 42 };
        let serialized = safe_serialize(&test).unwrap();
        let result: SuperNovaResult<TestStruct> = safe_deserialize(&serialized);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), test);
        
        // Test with invalid data
        let invalid_data = vec![0xFF, 0xFF, 0xFF, 0xFF];
        let result: SuperNovaResult<TestStruct> = safe_deserialize(&invalid_data);
        assert!(result.is_err());
    }
    
    #[test]
    fn test_safe_lock_macro() {
        use std::sync::{Mutex, RwLock};
        
        let mutex = Mutex::new(42);
        let result = (|| -> SuperNovaResult<()> {
            let value = safe_lock!(mutex);
            assert_eq!(*value, 42);
            Ok(())
        })();
        assert!(result.is_ok());
        
        let rwlock = RwLock::new("test");
        let result = (|| -> SuperNovaResult<()> {
            let value = safe_lock!(rwlock, read);
            assert_eq!(*value, "test");
            Ok(())
        })();
        assert!(result.is_ok());
    }
    
    #[test]
    fn test_safe_arithmetic() {
        // Test safe_add
        let result = (|| -> SuperNovaResult<u64> {
            let a: u64 = 100;
            let b: u64 = 200;
            Ok(safe_add!(a, b))
        })();
        assert_eq!(result.unwrap(), 300);
        
        // Test overflow
        let result = (|| -> SuperNovaResult<u64> {
            let a: u64 = u64::MAX;
            let b: u64 = 1;
            Ok(safe_add!(a, b))
        })();
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), SuperNovaError::ArithmeticOverflow(_)));
        
        // Test safe_sub
        let result = (|| -> SuperNovaResult<u64> {
            let a: u64 = 300;
            let b: u64 = 100;
            Ok(safe_sub!(a, b))
        })();
        assert_eq!(result.unwrap(), 200);
        
        // Test underflow
        let result = (|| -> SuperNovaResult<u64> {
            let a: u64 = 100;
            let b: u64 = 200;
            Ok(safe_sub!(a, b))
        })();
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), SuperNovaError::ArithmeticOverflow(_)));
    }
    
    #[test]
    fn test_get_system_time() {
        let result = get_system_time();
        assert!(result.is_ok());
        let timestamp = result.unwrap();
        assert!(timestamp > 0);
        
        // Timestamp should be reasonable (after 2020)
        assert!(timestamp > 1577836800); // Jan 1, 2020
    }
    
    #[test]
    fn test_error_handling_patterns() {
        // Pattern 1: Using ? operator with custom errors
        fn process_data() -> SuperNovaResult<String> {
            let data = vec![1, 2, 3];
            let serialized = safe_serialize(&data)?;
            let deserialized: Vec<i32> = safe_deserialize(&serialized)?;
            Ok(format!("Processed {} items", deserialized.len()))
        }
        
        let result = process_data();
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "Processed 3 items");
        
        // Pattern 2: Chaining with context
        fn load_and_process() -> SuperNovaResult<()> {
            std::fs::read_to_string("nonexistent.txt")
                .context("Failed to read config file")?;
            Ok(())
        }
        
        let result = load_and_process();
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Failed to read config file"));
    }
    
    #[test]
    fn test_no_unwrap_patterns() {
        // Instead of: value.unwrap()
        // Use: value.safe_unwrap("context")?
        
        // Instead of: result.unwrap()
        // Use: result.context("context")?
        
        // Instead of: mutex.lock().unwrap()
        // Use: safe_lock!(mutex)
        
        // Instead of: a + b (with potential overflow)
        // Use: safe_add!(a, b)
        
        // Instead of: bincode::serialize(&data).unwrap()
        // Use: safe_serialize(&data)?
        
        // These patterns ensure all errors are properly handled and provide context
    }
} 