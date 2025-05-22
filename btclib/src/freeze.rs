// This file is used to mark and disable problematic code sections during development
// When FREEZE_MODE is true, certain problematic components will be skipped from compilation

/// When true, problematic code sections marked with the freeze macro will be disabled
pub const FREEZE_MODE: bool = true;

/// Macro to conditionally compile code based on freeze status
#[macro_export]
macro_rules! freeze_skip {
    ($($tokens:tt)*) => {
        #[cfg(not(feature = "freeze"))]
        {
            $($tokens)*
        }
    };
}

/// Macro to generate empty implementations when in freeze mode
#[macro_export]
macro_rules! freeze_stub {
    ($type:ident, $func:ident, $ret:ty) => {
        #[cfg(feature = "freeze")]
        impl $type {
            pub fn $func(&self) -> $ret {
                Default::default()
            }
        }
    };
}

// Freeze functionality for the Supernova blockchain
// This module allows temporarily disabling parts of the code during compilation

use std::fmt;

/// Status of a feature
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FreezeStatus {
    /// Feature is active
    Active,
    /// Feature is frozen (disabled)
    Frozen,
    /// Feature is in development
    InDevelopment,
    /// Feature is deprecated
    Deprecated,
}

impl fmt::Display for FreezeStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FreezeStatus::Active => write!(f, "Active"),
            FreezeStatus::Frozen => write!(f, "Frozen"),
            FreezeStatus::InDevelopment => write!(f, "In Development"),
            FreezeStatus::Deprecated => write!(f, "Deprecated"),
        }
    }
}

/// A feature that can be frozen
#[derive(Debug, Clone)]
pub struct FreezableFeature {
    /// Name of the feature
    pub name: String,
    /// Current status of the feature
    pub status: FreezeStatus,
    /// Description of the feature
    pub description: String,
}

impl FreezableFeature {
    /// Create a new freezable feature
    pub fn new(name: &str, status: FreezeStatus, description: &str) -> Self {
        Self {
            name: name.to_string(),
            status,
            description: description.to_string(),
        }
    }
    
    /// Check if the feature is active
    pub fn is_active(&self) -> bool {
        self.status == FreezeStatus::Active
    }
    
    /// Check if the feature is frozen
    pub fn is_frozen(&self) -> bool {
        self.status == FreezeStatus::Frozen
    }
}

/// Registry of freezable features
#[derive(Debug, Clone, Default)]
pub struct FreezeRegistry {
    /// List of features
    features: Vec<FreezableFeature>,
}

impl FreezeRegistry {
    /// Create a new empty registry
    pub fn new() -> Self {
        Self {
            features: Vec::new(),
        }
    }
    
    /// Register a new feature
    pub fn register(&mut self, feature: FreezableFeature) {
        self.features.push(feature);
    }
    
    /// Get a feature by name
    pub fn get(&self, name: &str) -> Option<&FreezableFeature> {
        self.features.iter().find(|f| f.name == name)
    }
    
    /// Check if a feature is active
    pub fn is_active(&self, name: &str) -> bool {
        self.get(name).map_or(false, |f| f.is_active())
    }
    
    /// Get all features
    pub fn all_features(&self) -> &[FreezableFeature] {
        &self.features
    }
    
    /// Get all active features
    pub fn active_features(&self) -> Vec<&FreezableFeature> {
        self.features.iter().filter(|f| f.is_active()).collect()
    }
    
    /// Get all frozen features
    pub fn frozen_features(&self) -> Vec<&FreezableFeature> {
        self.features.iter().filter(|f| f.is_frozen()).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_freeze_status() {
        assert_eq!(FreezeStatus::Active.to_string(), "Active");
        assert_eq!(FreezeStatus::Frozen.to_string(), "Frozen");
        assert_eq!(FreezeStatus::InDevelopment.to_string(), "In Development");
        assert_eq!(FreezeStatus::Deprecated.to_string(), "Deprecated");
    }
    
    #[test]
    fn test_freezable_feature() {
        let feature = FreezableFeature::new(
            "test_feature",
            FreezeStatus::Active,
            "Test feature description"
        );
        
        assert_eq!(feature.name, "test_feature");
        assert_eq!(feature.status, FreezeStatus::Active);
        assert_eq!(feature.description, "Test feature description");
        assert!(feature.is_active());
        assert!(!feature.is_frozen());
    }
    
    #[test]
    fn test_freeze_registry() {
        let mut registry = FreezeRegistry::new();
        
        registry.register(FreezableFeature::new(
            "feature1",
            FreezeStatus::Active,
            "Feature 1"
        ));
        
        registry.register(FreezableFeature::new(
            "feature2",
            FreezeStatus::Frozen,
            "Feature 2"
        ));
        
        assert!(registry.is_active("feature1"));
        assert!(!registry.is_active("feature2"));
        assert!(!registry.is_active("nonexistent"));
        
        assert_eq!(registry.all_features().len(), 2);
        assert_eq!(registry.active_features().len(), 1);
        assert_eq!(registry.frozen_features().len(), 1);
    }
} 