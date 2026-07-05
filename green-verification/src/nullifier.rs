//! L2 nullifier set — double-claim rejection (ADR-0002 §6).
//!
//! In the real system these sets are **consensus state**, persisted and pruned
//! by vintage-window expiry like the UTXO set. This prototype keeps them
//! in-memory only; persistence is deferred.
//!
//! TODO(phase-3): back these sets with persistent consensus storage and
//! vintage-window pruning.

use std::collections::HashSet;

use serde::{Deserialize, Serialize};

use crate::error::GreenError;

/// Tracks already-claimed EAC retirement ids and retired offset serials.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct NullifierSet {
    /// EAC retirement ids that have already funded a rebate.
    pub used_eac_ids: HashSet<[u8; 32]>,
    /// Offset serials that have already been retired.
    pub used_offset_serials: HashSet<[u8; 32]>,
}

impl NullifierSet {
    /// Create an empty nullifier set.
    pub fn new() -> Self {
        Self::default()
    }

    /// Reject a reused EAC id, otherwise record it as used.
    ///
    /// On duplicate, the set is left unchanged and [`GreenError::EacAlreadyUsed`]
    /// is returned (reject-then-nothing, never partial insert).
    pub fn check_and_insert_eac(&mut self, id: [u8; 32]) -> Result<(), GreenError> {
        if self.used_eac_ids.contains(&id) {
            return Err(GreenError::EacAlreadyUsed);
        }
        self.used_eac_ids.insert(id);
        Ok(())
    }

    /// Reject a reused offset serial, otherwise record it as used.
    pub fn check_and_insert_offset(&mut self, serial: [u8; 32]) -> Result<(), GreenError> {
        if self.used_offset_serials.contains(&serial) {
            return Err(GreenError::OffsetSerialAlreadyUsed);
        }
        self.used_offset_serials.insert(serial);
        Ok(())
    }

    /// Whether an EAC id has already been used.
    pub fn contains_eac(&self, id: &[u8; 32]) -> bool {
        self.used_eac_ids.contains(id)
    }

    /// Whether an offset serial has already been used.
    pub fn contains_offset(&self, serial: &[u8; 32]) -> bool {
        self.used_offset_serials.contains(serial)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn eac_reuse_is_rejected() {
        let mut set = NullifierSet::new();
        let id = [7u8; 32];
        assert!(set.check_and_insert_eac(id).is_ok());
        assert_eq!(
            set.check_and_insert_eac(id),
            Err(GreenError::EacAlreadyUsed)
        );
        assert!(set.contains_eac(&id));
    }

    #[test]
    fn offset_reuse_is_rejected() {
        let mut set = NullifierSet::new();
        let serial = [9u8; 32];
        assert!(set.check_and_insert_offset(serial).is_ok());
        assert_eq!(
            set.check_and_insert_offset(serial),
            Err(GreenError::OffsetSerialAlreadyUsed)
        );
        assert!(set.contains_offset(&serial));
    }
}
