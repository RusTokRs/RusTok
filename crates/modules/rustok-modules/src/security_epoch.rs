//! Monotonic global security epochs for non-blocking quarantine and instant revocation.
//!
//! Provides platform-wide preemption: when an artifact is quarantined or revoked,
//! the global security epoch is monotonically advanced. All in-flight execution leases,
//! tenant state transitions, and rollouts must validate against the current epoch before
//! committing.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::backend::SupportedBackend;

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

    /// Builds a validation snapshot from a durable current epoch (loaded via
    /// [`SeaOrmSecurityEpochStore`]). The full advancement history lives in
    /// the database; the snapshot only needs the current value and the latest
    /// reason for fail-closed validation and diagnostics.
    pub fn from_current(
        current: GlobalSecurityEpoch,
        latest_reason: impl Into<String>,
        advanced_at: DateTime<Utc>,
    ) -> Self {
        Self {
            current,
            history: vec![SecurityEpochRecord {
                epoch: current,
                advanced_at,
                reason: latest_reason.into(),
            }],
        }
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

/// Errors from the durable security-epoch store.
#[derive(Debug, Error)]
pub enum SecurityEpochStoreError {
    #[error("security epoch persistence failed: {0}")]
    Persistence(String),
    #[error("stored security epoch must be >= 1, found {0}")]
    InvalidStoredEpoch(i64),
    #[error("security epoch row is missing (run the module security epoch migration)")]
    MissingRow,
    #[error("security epoch advancement matched no row")]
    AdvanceMatchedNoRow,
}

impl From<sea_orm::DbErr> for SecurityEpochStoreError {
    fn from(error: sea_orm::DbErr) -> Self {
        Self::Persistence(error.to_string())
    }
}

/// Durable security epoch backed by the `module_security_epoch` singleton row
/// (audit remediation D9/H-7).
///
/// Until this store existed, every process constructed an in-memory registry
/// permanently stuck at epoch 1, so quarantine/revocation never preempted
/// in-flight transitions and checkpoints always recorded epoch 1. All epoch
/// reads and advancements now go through this single durable owner;
/// [`SecurityEpochRegistry`] remains only as an in-memory validation
/// snapshot built from the durable value.
pub struct SeaOrmSecurityEpochStore;

impl SeaOrmSecurityEpochStore {
    /// Loads the durable current epoch and the reason for its latest
    /// advancement.
    pub async fn load_record<C>(db: &C) -> Result<SecurityEpochRecord, SecurityEpochStoreError>
    where
        C: sea_orm::ConnectionTrait,
    {
        use sea_orm::Statement;

        let backend = SupportedBackend::new(db.get_database_backend())
            .map_err(|e| SecurityEpochStoreError::Persistence(e.to_string()))?;
        let row = db
            .query_one_raw(Statement::from_string(
                backend.db_backend(),
                "SELECT epoch, last_reason, updated_at FROM module_security_epoch \
                 WHERE singleton_id = 1",
            ))
            .await?
            .ok_or(SecurityEpochStoreError::MissingRow)?;
        let epoch: i64 = row.try_get("", "epoch")?;
        let epoch = u64::try_from(epoch)
            .ok()
            .filter(|value| *value >= 1)
            .ok_or(SecurityEpochStoreError::InvalidStoredEpoch(epoch))?;
        let last_reason: String = row.try_get("", "last_reason")?;
        let advanced_at = match backend {
            SupportedBackend::Postgres => {
                let value: chrono::DateTime<chrono::FixedOffset> = row.try_get("", "updated_at")?;
                value.with_timezone(&Utc)
            }
            SupportedBackend::Sqlite => {
                let value: String = row.try_get("", "updated_at")?;
                crate::backend::parse_sqlite_datetime(&value).map_err(|error| {
                    SecurityEpochStoreError::Persistence(format!(
                        "stored security epoch timestamp is invalid: {error}"
                    ))
                })?
            }
        };
        Ok(SecurityEpochRecord {
            epoch: GlobalSecurityEpoch(epoch),
            advanced_at,
            reason: last_reason,
        })
    }

    /// Loads the durable current epoch.
    pub async fn current<C>(db: &C) -> Result<GlobalSecurityEpoch, SecurityEpochStoreError>
    where
        C: sea_orm::ConnectionTrait,
    {
        Ok(Self::load_record(db).await?.epoch)
    }

    /// Loads a validation snapshot from the durable epoch.
    pub async fn load_registry<C>(db: &C) -> Result<SecurityEpochRegistry, SecurityEpochStoreError>
    where
        C: sea_orm::ConnectionTrait,
    {
        let record = Self::load_record(db).await?;
        Ok(SecurityEpochRegistry::from_current(
            record.epoch,
            record.reason,
            record.advanced_at,
        ))
    }

    /// Durably advances the epoch by one and records `reason`. Callers run
    /// this in the same transaction as the quarantine/revocation write so
    /// preemption is atomic with the security decision.
    pub async fn advance<C>(
        db: &C,
        reason: &str,
        now: DateTime<Utc>,
    ) -> Result<GlobalSecurityEpoch, SecurityEpochStoreError>
    where
        C: sea_orm::ConnectionTrait,
    {
        use sea_orm::Statement;

        let backend = SupportedBackend::new(db.get_database_backend())
            .map_err(|e| SecurityEpochStoreError::Persistence(e.to_string()))?;
        let updated_at: sea_orm::Value = backend.datetime_value(now);
        let p1 = backend.placeholder(1);
        let p2 = backend.placeholder(2);
        let result = db
            .execute_raw(Statement::from_sql_and_values(
                backend.db_backend(),
                format!(
                    "UPDATE module_security_epoch \
                     SET epoch = epoch + 1, last_reason = {p1}, updated_at = {p2} \
                     WHERE singleton_id = 1"
                ),
                [reason.to_string().into(), updated_at],
            ))
            .await?;
        if result.rows_affected() != 1 {
            return Err(SecurityEpochStoreError::AdvanceMatchedNoRow);
        }
        Self::current(db).await
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
