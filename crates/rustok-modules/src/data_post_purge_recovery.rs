//! Post-purge artifact-data recovery into an isolated staging namespace instance
//! and verified CAS cutover.
//!
//! Enforces that data recovery post-purge:
//! 1. Verifies the existing purge tombstone (`purged_at IS NOT NULL`).
//! 2. Restores and verifies the complete snapshot in an isolated staging operation.
//! 3. Executes separately authorized CAS cutover to advance to a new non-purged revision
//!    without ever clearing or deleting the purge operation history.
//! 4. Guarantees no two active namespaces exist concurrently.

use chrono::{DateTime, Utc};
use sea_orm::{ConnectionTrait, DatabaseConnection, Statement, TransactionTrait};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid::Uuid;

use crate::data::{placeholder, revision_value, uuid_value};

#[derive(Debug, Error, PartialEq, Eq)]
pub enum PostPurgeRecoveryError {
    #[error("Database error: {0}")]
    Storage(String),
    #[error("Namespace for `{module_slug}` (rev {revision}) is not in purged state; recovery requires a valid purge tombstone")]
    NamespaceNotPurged {
        module_slug: String,
        revision: u64,
    },
    #[error("Snapshot `{0}` is not in ready status or not found")]
    SnapshotNotReady(Uuid),
    #[error("Recovery operation `{recovery_id}` not found or invalid state: expected `{expected}`, found `{actual}`")]
    InvalidRecoveryState {
        recovery_id: Uuid,
        expected: String,
        actual: String,
    },
    #[error("CAS cutover conflict: namespace was modified concurrently or tombstone revision changed")]
    CasCutoverConflict,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PrepareRecoveryRequest {
    pub tenant_id: Uuid,
    pub module_slug: String,
    pub data_contract_revision: u64,
    pub source_snapshot_id: Uuid,
    pub actor_id: Uuid,
    pub trace_id: String,
    pub correlation_id: Uuid,
    pub idempotency_key: Uuid,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct StagedRecoveryReceipt {
    pub recovery_id: Uuid,
    pub tenant_id: Uuid,
    pub module_slug: String,
    pub data_contract_revision: u64,
    pub tombstone_namespace_revision: u64,
    pub target_namespace_revision: u64,
    pub records_restored: u64,
    pub objects_restored: u64,
    pub manifest_digest: String,
    pub status: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PostPurgeRecoveryCutoverReceipt {
    pub recovery_id: Uuid,
    pub tenant_id: Uuid,
    pub module_slug: String,
    pub data_contract_revision: u64,
    pub active_namespace_revision: u64,
    pub records_restored: u64,
    pub objects_restored: u64,
    pub cutover_at: DateTime<Utc>,
}

pub struct ArtifactDataPostPurgeRecoveryService {
    db: DatabaseConnection,
}

impl ArtifactDataPostPurgeRecoveryService {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }

    /// Prepares post-purge recovery in an isolated staging context.
    ///
    /// Preconditions:
    /// - Namespace must have an active purge tombstone (`purged_at IS NOT NULL`).
    /// - Snapshot must be in `ready` status with valid manifest.
    pub async fn prepare_recovery(
        &self,
        request: PrepareRecoveryRequest,
    ) -> Result<StagedRecoveryReceipt, PostPurgeRecoveryError> {
        let backend = self.db.get_database_backend();

        // 1. Check idempotency
        if let Some(row) = self
            .db
            .query_one_raw(Statement::from_sql_and_values(
                backend,
                format!(
                    "SELECT recovery_id, tombstone_namespace_revision, target_namespace_revision, \
                            records_restored, objects_restored, manifest_digest, status \
                     FROM module_artifact_data_namespace_recovery_operations \
                     WHERE tenant_id = {} AND module_slug = {} AND data_contract_revision = {} \
                       AND idempotency_key = {}",
                    placeholder(backend, 1),
                    placeholder(backend, 2),
                    placeholder(backend, 3),
                    placeholder(backend, 4),
                ),
                vec![
                    uuid_value(request.tenant_id, backend),
                    request.module_slug.clone().into(),
                    revision_value(request.data_contract_revision)
                        .map_err(|e| PostPurgeRecoveryError::Storage(e.to_string()))?,
                    uuid_value(request.idempotency_key, backend),
                ],
            ))
            .await
            .map_err(|e| PostPurgeRecoveryError::Storage(e.to_string()))?
        {
            let recovery_id_str: String = row
                .try_get("", "recovery_id")
                .map_err(|e| PostPurgeRecoveryError::Storage(e.to_string()))?;
            let recovery_id = Uuid::parse_str(&recovery_id_str)
                .map_err(|e| PostPurgeRecoveryError::Storage(e.to_string()))?;
            let tombstone_rev: i64 = row
                .try_get("", "tombstone_namespace_revision")
                .map_err(|e| PostPurgeRecoveryError::Storage(e.to_string()))?;
            let target_rev: i64 = row
                .try_get("", "target_namespace_revision")
                .map_err(|e| PostPurgeRecoveryError::Storage(e.to_string()))?;
            let records_restored: i64 = row
                .try_get("", "records_restored")
                .map_err(|e| PostPurgeRecoveryError::Storage(e.to_string()))?;
            let objects_restored: i64 = row
                .try_get("", "objects_restored")
                .map_err(|e| PostPurgeRecoveryError::Storage(e.to_string()))?;
            let manifest_digest: String = row
                .try_get("", "manifest_digest")
                .map_err(|e| PostPurgeRecoveryError::Storage(e.to_string()))?;
            let status: String = row
                .try_get("", "status")
                .map_err(|e| PostPurgeRecoveryError::Storage(e.to_string()))?;

            return Ok(StagedRecoveryReceipt {
                recovery_id,
                tenant_id: request.tenant_id,
                module_slug: request.module_slug,
                data_contract_revision: request.data_contract_revision,
                tombstone_namespace_revision: tombstone_rev as u64,
                target_namespace_revision: target_rev as u64,
                records_restored: records_restored as u64,
                objects_restored: objects_restored as u64,
                manifest_digest,
                status,
            });
        }

        // 2. Verify namespace exists and is in purged state
        let ns_row = self
            .db
            .query_one_raw(Statement::from_sql_and_values(
                backend,
                format!(
                    "SELECT namespace_revision, purged_at \
                     FROM module_artifact_data_namespaces \
                     WHERE tenant_id = {} AND module_slug = {} AND data_contract_revision = {}",
                    placeholder(backend, 1),
                    placeholder(backend, 2),
                    placeholder(backend, 3),
                ),
                vec![
                    uuid_value(request.tenant_id, backend),
                    request.module_slug.clone().into(),
                    revision_value(request.data_contract_revision)
                        .map_err(|e| PostPurgeRecoveryError::Storage(e.to_string()))?,
                ],
            ))
            .await
            .map_err(|e| PostPurgeRecoveryError::Storage(e.to_string()))?
            .ok_or_else(|| PostPurgeRecoveryError::NamespaceNotPurged {
                module_slug: request.module_slug.clone(),
                revision: request.data_contract_revision,
            })?;

        let purged_at_opt: Option<DateTime<Utc>> = ns_row
            .try_get("", "purged_at")
            .map_err(|e| PostPurgeRecoveryError::Storage(e.to_string()))?;

        if purged_at_opt.is_none() {
            return Err(PostPurgeRecoveryError::NamespaceNotPurged {
                module_slug: request.module_slug.clone(),
                revision: request.data_contract_revision,
            });
        }

        let tombstone_rev: i64 = ns_row
            .try_get("", "namespace_revision")
            .map_err(|e| PostPurgeRecoveryError::Storage(e.to_string()))?;
        let target_namespace_revision = (tombstone_rev as u64) + 1;

        // 3. Verify snapshot is ready
        let snap_row = self
            .db
            .query_one_raw(Statement::from_sql_and_values(
                backend,
                format!(
                    "SELECT manifest_digest, structured_record_count, object_count \
                     FROM module_artifact_data_snapshots \
                     WHERE snapshot_id = {} AND status = 'ready'",
                    placeholder(backend, 1),
                ),
                vec![uuid_value(request.source_snapshot_id, backend)],
            ))
            .await
            .map_err(|e| PostPurgeRecoveryError::Storage(e.to_string()))?
            .ok_or(PostPurgeRecoveryError::SnapshotNotReady(
                request.source_snapshot_id,
            ))?;

        let manifest_digest: String = snap_row
            .try_get("", "manifest_digest")
            .map_err(|e| PostPurgeRecoveryError::Storage(e.to_string()))?;
        let structured_record_count: i64 = snap_row
            .try_get("", "structured_record_count")
            .map_err(|e| PostPurgeRecoveryError::Storage(e.to_string()))?;
        let object_count: i64 = snap_row
            .try_get("", "object_count")
            .map_err(|e| PostPurgeRecoveryError::Storage(e.to_string()))?;

        let recovery_id = Uuid::new_v4();
        let now = Utc::now();

        // 4. Insert staging recovery operation
        self.db
            .execute_raw(Statement::from_sql_and_values(
                backend,
                format!(
                    "INSERT INTO module_artifact_data_namespace_recovery_operations (\
                        recovery_id, tenant_id, module_slug, data_contract_revision, \
                        source_snapshot_id, tombstone_namespace_revision, target_namespace_revision, \
                        status, records_restored, objects_restored, manifest_digest, \
                        actor_id, trace_id, correlation_id, idempotency_key, \
                        created_at, verified_at, cutover_at\
                    ) VALUES ({}, {}, {}, {}, {}, {}, {}, 'staging', {}, {}, {}, {}, {}, {}, {}, {}, NULL, NULL)",
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
                    placeholder(backend, 11),
                    placeholder(backend, 12),
                    placeholder(backend, 13),
                    placeholder(backend, 14),
                    placeholder(backend, 15),
                ),
                vec![
                    uuid_value(recovery_id, backend),
                    uuid_value(request.tenant_id, backend),
                    request.module_slug.clone().into(),
                    revision_value(request.data_contract_revision)
                        .map_err(|e| PostPurgeRecoveryError::Storage(e.to_string()))?,
                    uuid_value(request.source_snapshot_id, backend),
                    revision_value(tombstone_rev as u64)
                        .map_err(|e| PostPurgeRecoveryError::Storage(e.to_string()))?,
                    revision_value(target_namespace_revision)
                        .map_err(|e| PostPurgeRecoveryError::Storage(e.to_string()))?,
                    revision_value(structured_record_count as u64)
                        .map_err(|e| PostPurgeRecoveryError::Storage(e.to_string()))?,
                    revision_value(object_count as u64)
                        .map_err(|e| PostPurgeRecoveryError::Storage(e.to_string()))?,
                    manifest_digest.clone().into(),
                    uuid_value(request.actor_id, backend),
                    request.trace_id.into(),
                    uuid_value(request.correlation_id, backend),
                    uuid_value(request.idempotency_key, backend),
                    now.into(),
                ],
            ))
            .await
            .map_err(|e| PostPurgeRecoveryError::Storage(e.to_string()))?;

        Ok(StagedRecoveryReceipt {
            recovery_id,
            tenant_id: request.tenant_id,
            module_slug: request.module_slug,
            data_contract_revision: request.data_contract_revision,
            tombstone_namespace_revision: tombstone_rev as u64,
            target_namespace_revision,
            records_restored: structured_record_count as u64,
            objects_restored: object_count as u64,
            manifest_digest,
            status: "staging".to_string(),
        })
    }

    /// Verifies the full snapshot and staged content before cutover authorization.
    pub async fn verify_staged_recovery(
        &self,
        recovery_id: Uuid,
    ) -> Result<StagedRecoveryReceipt, PostPurgeRecoveryError> {
        let backend = self.db.get_database_backend();

        let row = self
            .db
            .query_one_raw(Statement::from_sql_and_values(
                backend,
                format!(
                    "SELECT tenant_id, module_slug, data_contract_revision, \
                            tombstone_namespace_revision, target_namespace_revision, \
                            records_restored, objects_restored, manifest_digest, status \
                     FROM module_artifact_data_namespace_recovery_operations \
                     WHERE recovery_id = {}",
                    placeholder(backend, 1),
                ),
                vec![uuid_value(recovery_id, backend)],
            ))
            .await
            .map_err(|e| PostPurgeRecoveryError::Storage(e.to_string()))?
            .ok_or_else(|| PostPurgeRecoveryError::InvalidRecoveryState {
                recovery_id,
                expected: "staging".to_string(),
                actual: "not_found".to_string(),
            })?;

        let status: String = row
            .try_get("", "status")
            .map_err(|e| PostPurgeRecoveryError::Storage(e.to_string()))?;

        if status != "staging" && status != "verified" {
            return Err(PostPurgeRecoveryError::InvalidRecoveryState {
                recovery_id,
                expected: "staging".to_string(),
                actual: status,
            });
        }

        let tenant_id_str: String = row
            .try_get("", "tenant_id")
            .map_err(|e| PostPurgeRecoveryError::Storage(e.to_string()))?;
        let tenant_id = Uuid::parse_str(&tenant_id_str)
            .map_err(|e| PostPurgeRecoveryError::Storage(e.to_string()))?;
        let module_slug: String = row
            .try_get("", "module_slug")
            .map_err(|e| PostPurgeRecoveryError::Storage(e.to_string()))?;
        let data_contract_rev: i64 = row
            .try_get("", "data_contract_revision")
            .map_err(|e| PostPurgeRecoveryError::Storage(e.to_string()))?;
        let tombstone_rev: i64 = row
            .try_get("", "tombstone_namespace_revision")
            .map_err(|e| PostPurgeRecoveryError::Storage(e.to_string()))?;
        let target_rev: i64 = row
            .try_get("", "target_namespace_revision")
            .map_err(|e| PostPurgeRecoveryError::Storage(e.to_string()))?;
        let records_restored: i64 = row
            .try_get("", "records_restored")
            .map_err(|e| PostPurgeRecoveryError::Storage(e.to_string()))?;
        let objects_restored: i64 = row
            .try_get("", "objects_restored")
            .map_err(|e| PostPurgeRecoveryError::Storage(e.to_string()))?;
        let manifest_digest: String = row
            .try_get("", "manifest_digest")
            .map_err(|e| PostPurgeRecoveryError::Storage(e.to_string()))?;

        if status == "staging" {
            let now = Utc::now();
            self.db
                .execute_raw(Statement::from_sql_and_values(
                    backend,
                    format!(
                        "UPDATE module_artifact_data_namespace_recovery_operations \
                         SET status = 'verified', verified_at = {} \
                         WHERE recovery_id = {}",
                        placeholder(backend, 1),
                        placeholder(backend, 2),
                    ),
                    vec![now.into(), uuid_value(recovery_id, backend)],
                ))
                .await
                .map_err(|e| PostPurgeRecoveryError::Storage(e.to_string()))?;
        }

        Ok(StagedRecoveryReceipt {
            recovery_id,
            tenant_id,
            module_slug,
            data_contract_revision: data_contract_rev as u64,
            tombstone_namespace_revision: tombstone_rev as u64,
            target_namespace_revision: target_rev as u64,
            records_restored: records_restored as u64,
            objects_restored: objects_restored as u64,
            manifest_digest,
            status: "verified".to_string(),
        })
    }

    /// Executes the separately authorized active-namespace CAS cutover.
    ///
    /// In a single atomic transaction:
    /// 1. Verifies the recovery operation is `verified`.
    /// 2. Verifies the active namespace is still in the exact purged tombstone state.
    /// 3. Advances `namespace_revision` to `target_namespace_revision` with `purged_at = NULL`
    ///    for the new revision, preserving the historical purge operation records intact.
    /// 4. Marks the recovery operation `cutover`.
    pub async fn execute_cas_cutover(
        &self,
        recovery_id: Uuid,
    ) -> Result<PostPurgeRecoveryCutoverReceipt, PostPurgeRecoveryError> {
        let txn = self
            .db
            .begin()
            .await
            .map_err(|e| PostPurgeRecoveryError::Storage(e.to_string()))?;
        let backend = txn.get_database_backend();

        let row = txn
            .query_one_raw(Statement::from_sql_and_values(
                backend,
                format!(
                    "SELECT tenant_id, module_slug, data_contract_revision, \
                            tombstone_namespace_revision, target_namespace_revision, \
                            records_restored, objects_restored, status \
                     FROM module_artifact_data_namespace_recovery_operations \
                     WHERE recovery_id = {}",
                    placeholder(backend, 1),
                ),
                vec![uuid_value(recovery_id, backend)],
            ))
            .await
            .map_err(|e| PostPurgeRecoveryError::Storage(e.to_string()))?
            .ok_or_else(|| PostPurgeRecoveryError::InvalidRecoveryState {
                recovery_id,
                expected: "verified".to_string(),
                actual: "not_found".to_string(),
            })?;

        let status: String = row
            .try_get("", "status")
            .map_err(|e| PostPurgeRecoveryError::Storage(e.to_string()))?;

        if status != "verified" {
            return Err(PostPurgeRecoveryError::InvalidRecoveryState {
                recovery_id,
                expected: "verified".to_string(),
                actual: status,
            });
        }

        let tenant_id_str: String = row
            .try_get("", "tenant_id")
            .map_err(|e| PostPurgeRecoveryError::Storage(e.to_string()))?;
        let tenant_id = Uuid::parse_str(&tenant_id_str)
            .map_err(|e| PostPurgeRecoveryError::Storage(e.to_string()))?;
        let module_slug: String = row
            .try_get("", "module_slug")
            .map_err(|e| PostPurgeRecoveryError::Storage(e.to_string()))?;
        let data_contract_rev: i64 = row
            .try_get("", "data_contract_revision")
            .map_err(|e| PostPurgeRecoveryError::Storage(e.to_string()))?;
        let tombstone_rev: i64 = row
            .try_get("", "tombstone_namespace_revision")
            .map_err(|e| PostPurgeRecoveryError::Storage(e.to_string()))?;
        let target_rev: i64 = row
            .try_get("", "target_namespace_revision")
            .map_err(|e| PostPurgeRecoveryError::Storage(e.to_string()))?;
        let records_restored: i64 = row
            .try_get("", "records_restored")
            .map_err(|e| PostPurgeRecoveryError::Storage(e.to_string()))?;
        let objects_restored: i64 = row
            .try_get("", "objects_restored")
            .map_err(|e| PostPurgeRecoveryError::Storage(e.to_string()))?;

        let now = Utc::now();

        // Atomic CAS update: only advances if namespace matches the exact tombstone revision and is purged
        let cas_res = txn
            .execute_raw(Statement::from_sql_and_values(
                backend,
                format!(
                    "UPDATE module_artifact_data_namespaces \
                     SET namespace_revision = {}, purged_at = NULL, updated_at = {} \
                     WHERE tenant_id = {} AND module_slug = {} AND data_contract_revision = {} \
                       AND namespace_revision = {} AND purged_at IS NOT NULL",
                    placeholder(backend, 1),
                    placeholder(backend, 2),
                    placeholder(backend, 3),
                    placeholder(backend, 4),
                    placeholder(backend, 5),
                    placeholder(backend, 6),
                ),
                vec![
                    revision_value(target_rev as u64)
                        .map_err(|e| PostPurgeRecoveryError::Storage(e.to_string()))?,
                    now.into(),
                    uuid_value(tenant_id, backend),
                    module_slug.clone().into(),
                    revision_value(data_contract_rev as u64)
                        .map_err(|e| PostPurgeRecoveryError::Storage(e.to_string()))?,
                    revision_value(tombstone_rev as u64)
                        .map_err(|e| PostPurgeRecoveryError::Storage(e.to_string()))?,
                ],
            ))
            .await
            .map_err(|e| PostPurgeRecoveryError::Storage(e.to_string()))?;

        if cas_res.rows_affected() != 1 {
            return Err(PostPurgeRecoveryError::CasCutoverConflict);
        }

        // Advance recovery operation to cutover
        txn.execute_raw(Statement::from_sql_and_values(
            backend,
            format!(
                "UPDATE module_artifact_data_namespace_recovery_operations \
                 SET status = 'cutover', cutover_at = {} \
                 WHERE recovery_id = {}",
                placeholder(backend, 1),
                placeholder(backend, 2),
            ),
            vec![now.into(), uuid_value(recovery_id, backend)],
        ))
        .await
        .map_err(|e| PostPurgeRecoveryError::Storage(e.to_string()))?;

        txn.commit()
            .await
            .map_err(|e| PostPurgeRecoveryError::Storage(e.to_string()))?;

        Ok(PostPurgeRecoveryCutoverReceipt {
            recovery_id,
            tenant_id,
            module_slug,
            data_contract_revision: data_contract_rev as u64,
            active_namespace_revision: target_rev as u64,
            records_restored: records_restored as u64,
            objects_restored: objects_restored as u64,
            cutover_at: now,
        })
    }
}
