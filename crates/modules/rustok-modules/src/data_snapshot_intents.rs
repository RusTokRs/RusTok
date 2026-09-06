//! Durable per-copy snapshot/restore intents and staging receipts.
//!
//! Tracks object transfer intents across storage and database transactions so that
//! crashes after object publication but before metadata commit resume exactly or
//! collect proven orphan objects safely.

use std::time::Duration as StdDuration;

use chrono::{DateTime, Utc};
use sea_orm::{ConnectionTrait, DatabaseConnection, Statement};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid::Uuid;

use crate::data::{placeholder, revision_value, uuid_value};

#[derive(Debug, Error, PartialEq, Eq)]
pub enum SnapshotIntentError {
    #[error("Database error: {0}")]
    Storage(String),
    #[error("Intent `{0}` not found or already in terminal status")]
    InvalidIntentState(Uuid),
    #[error("Object digest conflict for `{object_name}`: expected `{expected}`, found `{actual}`")]
    DigestConflict {
        object_name: String,
        expected: String,
        actual: String,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SnapshotCopyKind {
    Snapshot,
    Restore,
}

impl SnapshotCopyKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Snapshot => "snapshot",
            Self::Restore => "restore",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SnapshotCopyIntent {
    pub intent_id: Uuid,
    pub tenant_id: Uuid,
    pub snapshot_id: Uuid,
    pub operation_kind: SnapshotCopyKind,
    pub object_name: String,
    pub source_storage_key: String,
    pub target_storage_key: String,
    pub digest_sha256: String,
    pub size_bytes: u64,
    pub status: String,
    pub created_at: DateTime<Utc>,
    pub committed_at: Option<DateTime<Utc>>,
    pub collected_at: Option<DateTime<Utc>>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReconciledSnapshotIntentsReceipt {
    pub total_scanned: u64,
    pub committed_resumed: u64,
    pub orphans_collected: u64,
}

pub struct ArtifactDataSnapshotIntentService {
    db: DatabaseConnection,
}

impl ArtifactDataSnapshotIntentService {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }

    /// Reserves a durable copy intent before publishing bytes to the storage backend.
    pub async fn reserve_intent(
        &self,
        tenant_id: Uuid,
        snapshot_id: Uuid,
        operation_kind: SnapshotCopyKind,
        object_name: &str,
        source_storage_key: &str,
        target_storage_key: &str,
        digest_sha256: &str,
        size_bytes: u64,
    ) -> Result<Uuid, SnapshotIntentError> {
        let backend = self.db.get_database_backend();
        let intent_id = Uuid::new_v4();
        let now = Utc::now();

        self.db
            .execute_raw(Statement::from_sql_and_values(
                backend,
                format!(
                    "INSERT INTO module_artifact_data_snapshot_copy_intents (\
                        intent_id, tenant_id, snapshot_id, operation_kind, object_name, \
                        source_storage_key, target_storage_key, digest_sha256, size_bytes, \
                        status, created_at, committed_at, collected_at\
                    ) VALUES ({}, {}, {}, {}, {}, {}, {}, {}, {}, 'intent', {}, NULL, NULL)\
                    ON CONFLICT (tenant_id, snapshot_id, operation_kind, object_name) DO UPDATE SET \
                        status = 'intent', target_storage_key = EXCLUDED.target_storage_key",
                    placeholder(backend, 1),
                    placeholder(backend, 2),
                    placeholder(backend, 3),
                    placeholder(backend, 4),
                    placeholder(backend, 5),
                    placeholder(backend, 6),
                    placeholder(backend, 7),
                    placeholder(backend, 8),
                    placeholder(backend, 9),
                    placeholder(backend, 10),
                ),
                vec![
                    uuid_value(intent_id, backend),
                    uuid_value(tenant_id, backend),
                    uuid_value(snapshot_id, backend),
                    operation_kind.as_str().into(),
                    object_name.to_string().into(),
                    source_storage_key.to_string().into(),
                    target_storage_key.to_string().into(),
                    digest_sha256.to_string().into(),
                    revision_value(size_bytes)
                        .map_err(|e| SnapshotIntentError::Storage(e.to_string()))?,
                    now.into(),
                ],
            ))
            .await
            .map_err(|e| SnapshotIntentError::Storage(e.to_string()))?;

        Ok(intent_id)
    }

    /// Records that the object bytes were published to storage and are in staging.
    pub async fn record_staging_receipt(&self, intent_id: Uuid) -> Result<(), SnapshotIntentError> {
        let backend = self.db.get_database_backend();
        let res = self
            .db
            .execute_raw(Statement::from_sql_and_values(
                backend,
                format!(
                    "UPDATE module_artifact_data_snapshot_copy_intents \
                     SET status = 'staging' \
                     WHERE intent_id = {} AND status = 'intent'",
                    placeholder(backend, 1),
                ),
                vec![uuid_value(intent_id, backend)],
            ))
            .await
            .map_err(|e| SnapshotIntentError::Storage(e.to_string()))?;

        if res.rows_affected() == 0 {
            return Err(SnapshotIntentError::InvalidIntentState(intent_id));
        }
        Ok(())
    }

    /// Commits the intent after metadata has been durably written to the database.
    pub async fn commit_intent(&self, intent_id: Uuid) -> Result<(), SnapshotIntentError> {
        let backend = self.db.get_database_backend();
        let now = Utc::now();
        let res = self
            .db
            .execute_raw(Statement::from_sql_and_values(
                backend,
                format!(
                    "UPDATE module_artifact_data_snapshot_copy_intents \
                     SET status = 'committed', committed_at = {} \
                     WHERE intent_id = {} AND status IN ('intent', 'staging')",
                    placeholder(backend, 1),
                    placeholder(backend, 2),
                ),
                vec![now.into(), uuid_value(intent_id, backend)],
            ))
            .await
            .map_err(|e| SnapshotIntentError::Storage(e.to_string()))?;

        if res.rows_affected() == 0 {
            return Err(SnapshotIntentError::InvalidIntentState(intent_id));
        }
        Ok(())
    }

    /// Reconciles stale uncommitted intents older than `grace_period`.
    ///
    /// If the parent snapshot or restore is finalized, resumes the commit.
    /// Otherwise marks as collected orphan.
    pub async fn reconcile_stale_intents(
        &self,
        tenant_id: Uuid,
        grace_period: StdDuration,
    ) -> Result<ReconciledSnapshotIntentsReceipt, SnapshotIntentError> {
        let backend = self.db.get_database_backend();
        let threshold = Utc::now() - chrono::Duration::from_std(grace_period)
            .unwrap_or_else(|_| chrono::Duration::seconds(300));

        let rows = self
            .db
            .query_all_raw(Statement::from_sql_and_values(
                backend,
                format!(
                    "SELECT intent_id, snapshot_id, status \
                     FROM module_artifact_data_snapshot_copy_intents \
                     WHERE tenant_id = {} AND status IN ('intent', 'staging') \
                       AND created_at < {}",
                    placeholder(backend, 1),
                    placeholder(backend, 2),
                ),
                vec![uuid_value(tenant_id, backend), threshold.into()],
            ))
            .await
            .map_err(|e| SnapshotIntentError::Storage(e.to_string()))?;

        let total_scanned = rows.len() as u64;
        let mut committed_resumed = 0u64;
        let mut orphans_collected = 0u64;

        for row in rows {
            let intent_id_str: String = row
                .try_get("", "intent_id")
                .map_err(|e| SnapshotIntentError::Storage(e.to_string()))?;
            let intent_id = Uuid::parse_str(&intent_id_str)
                .map_err(|e| SnapshotIntentError::Storage(e.to_string()))?;

            let snapshot_id_str: String = row
                .try_get("", "snapshot_id")
                .map_err(|e| SnapshotIntentError::Storage(e.to_string()))?;
            let snapshot_id = Uuid::parse_str(&snapshot_id_str)
                .map_err(|e| SnapshotIntentError::Storage(e.to_string()))?;

            // Check if parent snapshot reached 'ready' status
            let parent_ready = self
                .db
                .query_one_raw(Statement::from_sql_and_values(
                    backend,
                    format!(
                        "SELECT 1 FROM module_artifact_data_snapshots \
                         WHERE snapshot_id = {} AND status = 'ready'",
                        placeholder(backend, 1),
                    ),
                    vec![uuid_value(snapshot_id, backend)],
                ))
                .await
                .map_err(|e| SnapshotIntentError::Storage(e.to_string()))?
                .is_some();

            let now = Utc::now();
            if parent_ready {
                // Resume commit
                self.db
                    .execute_raw(Statement::from_sql_and_values(
                        backend,
                        format!(
                            "UPDATE module_artifact_data_snapshot_copy_intents \
                             SET status = 'committed', committed_at = {} \
                             WHERE intent_id = {}",
                            placeholder(backend, 1),
                            placeholder(backend, 2),
                        ),
                        vec![now.into(), uuid_value(intent_id, backend)],
                    ))
                    .await
                    .map_err(|e| SnapshotIntentError::Storage(e.to_string()))?;
                committed_resumed += 1;
            } else {
                // Collect proven orphan
                self.db
                    .execute_raw(Statement::from_sql_and_values(
                        backend,
                        format!(
                            "UPDATE module_artifact_data_snapshot_copy_intents \
                             SET status = 'collected', collected_at = {} \
                             WHERE intent_id = {}",
                            placeholder(backend, 1),
                            placeholder(backend, 2),
                        ),
                        vec![now.into(), uuid_value(intent_id, backend)],
                    ))
                    .await
                    .map_err(|e| SnapshotIntentError::Storage(e.to_string()))?;
                orphans_collected += 1;
            }
        }

        Ok(ReconciledSnapshotIntentsReceipt {
            total_scanned,
            committed_resumed,
            orphans_collected,
        })
    }
}
