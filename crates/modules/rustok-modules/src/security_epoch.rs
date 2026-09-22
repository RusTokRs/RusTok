//! Monotonic global security epochs for non-blocking quarantine and instant revocation.
//!
//! Provides platform-wide preemption: when an artifact is quarantined or revoked,
//! the global security epoch is monotonically advanced. All in-flight execution leases,
//! tenant state transitions, and rollouts must validate against the current epoch before
//! committing.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Monotonically increasing platform security epoch.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct GlobalSecurityEpoch(pub u64);

impl GlobalSecurityEpoch {
    pub const INITIAL: Self = Self(1);

    pub fn next(self) -> Self {
        Self(self.0 + 1)
    }

    pub fn value(self) -> u64 {
        self.0
    }
}

#[derive(Debug, Error, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SecurityEpochConflictError {
    #[error(
        "Security epoch conflict: operation epoch {expected:?} is stale (current epoch: {current:?}, reason: {latest_reason})"
    )]
    EpochStale {
        expected: GlobalSecurityEpoch,
        current: GlobalSecurityEpoch,
        latest_reason: String,
    },
}

/// Historical record of a security epoch advancement.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SecurityEpochRecord {
    pub epoch: GlobalSecurityEpoch,
    pub advanced_at: DateTime<Utc>,
    pub reason: String,
}

/// Registry managing platform-wide security epoch validation and monotonic advancement.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SecurityEpochRegistry {
    current: GlobalSecurityEpoch,
    history: Vec<SecurityEpochRecord>,
}

impl Default for SecurityEpochRegistry {
    fn default() -> Self {
        Self {
            current: GlobalSecurityEpoch::INITIAL,
            history: vec![SecurityEpochRecord {
                epoch: GlobalSecurityEpoch::INITIAL,
                advanced_at: Utc::now(),
                reason: "Initial security epoch bootstrap".to_string(),
            }],
        }
    }
}

impl SecurityEpochRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn current_epoch(&self) -> GlobalSecurityEpoch {
        self.current
    }

    pub fn latest_reason(&self) -> &str {
        self.history
            .last()
            .map(|r| r.reason.as_str())
            .unwrap_or("unknown")
    }

    pub fn history(&self) -> &[SecurityEpochRecord] {
        &self.history
    }

    /// Monotonically advances the security epoch upon quarantine, revocation, or critical breach.
    pub fn advance_epoch(&mut self, reason: impl Into<String>) -> GlobalSecurityEpoch {
        let next = self.current.next();
        let record = SecurityEpochRecord {
            epoch: next,
            advanced_at: Utc::now(),
            reason: reason.into(),
        };
        self.current = next;
        self.history.push(record);
        self.current
    }

    /// Validates that an in-flight operation's epoch matches the current active security epoch.
    ///
    /// Fails closed if any security revocation or quarantine occurred while the operation was in-flight.
    pub fn validate_epoch(
        &self,
        expected: GlobalSecurityEpoch,
    ) -> Result<(), SecurityEpochConflictError> {
        if expected == self.current {
            Ok(())
        } else {
            Err(SecurityEpochConflictError::EpochStale {
                expected,
                current: self.current,
                latest_reason: self.latest_reason().to_string(),
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_security_epoch_lifecycle() {
        let mut registry = SecurityEpochRegistry::new();
        assert_eq!(registry.current_epoch(), GlobalSecurityEpoch(1));

        // In-flight operation claims epoch 1
        let in_flight_epoch = registry.current_epoch();
        assert!(registry.validate_epoch(in_flight_epoch).is_ok());

        // Security quarantine event occurs
        let new_epoch =
            registry.advance_epoch("Artifact 'auth-jwt' quarantined due to CVE-2026-001");
        assert_eq!(new_epoch, GlobalSecurityEpoch(2));
        assert_eq!(registry.current_epoch(), GlobalSecurityEpoch(2));

        // In-flight operation tries to commit with stale epoch 1 -> MUST FAIL CLOSED!
        let result = registry.validate_epoch(in_flight_epoch);
        assert!(matches!(
            result,
            Err(SecurityEpochConflictError::EpochStale {
                expected: GlobalSecurityEpoch(1),
                current: GlobalSecurityEpoch(2),
                ..
            })
        ));

        // New operation with current epoch 2 succeeds
        assert!(registry.validate_epoch(new_epoch).is_ok());
    }
}
