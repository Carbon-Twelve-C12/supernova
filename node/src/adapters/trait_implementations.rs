//! Trait Implementations for btclib/node Integration
//! 
//! This module provides systematic trait implementations to resolve
//! trait bound errors between btclib and node layers.

use serde::{Serialize, Deserialize};
use libp2p::PeerId;
use std::hash::{Hash, Hasher};
use crate::metrics::performance::MetricType;

/// Implement Serialize for PeerId
#[derive(Clone)]
pub struct SerializablePeerId(pub PeerId);

impl Serialize for SerializablePeerId {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        self.0.to_string().serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for SerializablePeerId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        PeerId::from_bytes(s.as_bytes())
            .map(SerializablePeerId)
            .map_err(serde::de::Error::custom)
    }
}

/// Wrapper for PeerId with Serialize/Deserialize
pub fn serialize_peer_id<S>(peer_id: &PeerId, serializer: S) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    peer_id.to_string().serialize(serializer)
}

pub fn deserialize_peer_id<'de, D>(deserializer: D) -> Result<PeerId, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let s = String::deserialize(deserializer)?;
    PeerId::from_bytes(s.as_bytes())
        .map_err(serde::de::Error::custom)
}

/// Implement missing traits for MetricType
impl Eq for MetricType {}

impl Hash for MetricType {
    fn hash<H: Hasher>(&self, state: &mut H) {
        match self {
            MetricType::NetworkLatency => 0.hash(state),
            MetricType::BlockProcessing => 1.hash(state),
            MetricType::TransactionValidation => 2.hash(state),
            MetricType::StorageOperation => 3.hash(state),
            MetricType::TransactionProcessing => 4.hash(state),
            MetricType::BlockValidation => 5.hash(state),
            MetricType::DatabaseRead => 6.hash(state),
            MetricType::DatabaseWrite => 7.hash(state),
            MetricType::Network => 8.hash(state),
            MetricType::PeerConnection => 9.hash(state),
            MetricType::Mempool => 10.hash(state),
            MetricType::Synchronization => 11.hash(state),
            MetricType::ApiRequest => 12.hash(state),
            MetricType::Lightning => 13.hash(state),
            MetricType::Custom(s) => {
                14.hash(state);
                s.hash(state);
            }
        }
    }
}

/// Conversion traits for numeric types
pub trait SafeNumericConversion {
    fn to_f64_safe(&self) -> f64;
    fn to_usize_safe(&self) -> usize;
}

impl SafeNumericConversion for usize {
    fn to_f64_safe(&self) -> f64 {
        *self as f64
    }
    
    fn to_usize_safe(&self) -> usize {
        *self
    }
}

impl SafeNumericConversion for u64 {
    fn to_f64_safe(&self) -> f64 {
        *self as f64
    }
    
    fn to_usize_safe(&self) -> usize {
        *self as usize
    }
}

impl SafeNumericConversion for u32 {
    fn to_f64_safe(&self) -> f64 {
        *self as f64
    }
    
    fn to_usize_safe(&self) -> usize {
        *self as usize
    }
}

/// Helper trait for IVec conversions
pub trait IVecConversion {
    fn to_ivec(&self) -> sled::IVec;
    fn from_ivec(ivec: &sled::IVec) -> Self;
}

impl IVecConversion for Vec<u8> {
    fn to_ivec(&self) -> sled::IVec {
        sled::IVec::from(self.clone())
    }
    
    fn from_ivec(ivec: &sled::IVec) -> Self {
        ivec.to_vec()
    }
}

impl IVecConversion for [u8; 32] {
    fn to_ivec(&self) -> sled::IVec {
        sled::IVec::from(self.to_vec())
    }
    
    fn from_ivec(ivec: &sled::IVec) -> Self {
        let vec = ivec.to_vec();
        let mut arr = [0u8; 32];
        arr.copy_from_slice(&vec[..32.min(vec.len())]);
        arr
    }
}

/// Borrow implementation helper
pub trait BorrowHelper<T: ?Sized> {
    fn borrow_helper(&self) -> &T;
}

impl BorrowHelper<[u8; 32]> for [u8; 32] {
    fn borrow_helper(&self) -> &[u8; 32] {
        self
    }
}

impl BorrowHelper<[u8]> for [u8; 32] {
    fn borrow_helper(&self) -> &[u8] {
        &self[..]
    }
}

/// Wallet conversion trait placeholder
pub trait WalletConversion {
    fn to_lightning_wallet(&self) -> Result<btclib::lightning::wallet::LightningWallet, String>;
}

impl WalletConversion for Arc<()> {
    fn to_lightning_wallet(&self) -> Result<btclib::lightning::wallet::LightningWallet, String> {
        // Placeholder implementation - would need actual wallet conversion
        Err("Wallet conversion not implemented".to_string())
    }
}

/// UTXO transaction serialization helpers
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SerializableUtxoTransaction {
    pub tx_hash: [u8; 32],
    pub output_index: u32,
    pub value: u64,
    pub script: Vec<u8>,
}

/// Future serialization wrapper
pub struct SerializableFuture<T>(pub T);

impl<T> Serialize for SerializableFuture<T> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        // Futures can't be serialized - use a placeholder
        "future".serialize(serializer)
    }
}

/// Helper function to convert between numeric types safely
pub fn safe_numeric_cast<F, T>(value: F) -> T
where
    F: SafeNumericConversion,
    T: From<usize>,
{
    T::from(value.to_usize_safe())
}

/// Extension trait for Result type to handle conversions
pub trait ResultConversionExt<T, E> {
    fn convert_err<F>(self) -> Result<T, F>
    where
        E: Into<F>;
}

impl<T, E> ResultConversionExt<T, E> for Result<T, E> {
    fn convert_err<F>(self) -> Result<T, F>
    where
        E: Into<F>,
    {
        self.map_err(Into::into)
    }
}

use std::sync::Arc;

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_numeric_conversion() {
        let usize_val: usize = 42;
        assert_eq!(usize_val.to_f64_safe(), 42.0);
        
        let u64_val: u64 = 100;
        assert_eq!(u64_val.to_f64_safe(), 100.0);
    }
    
    #[test]
    fn test_ivec_conversion() {
        let vec = vec![1, 2, 3, 4];
        let ivec = vec.to_ivec();
        let back: Vec<u8> = Vec::from_ivec(&ivec);
        assert_eq!(vec, back);
    }
} 