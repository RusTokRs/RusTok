//! Durable snapshot and restore copy reservations.
//!
//! Bytes are published only after a reservation. Exact replay retains the
//! owner-selected key; reconciliation requires actual metadata and byte evidence.
//! Missing parents are unresolved, never age-authorized orphan collection.

use chrono::Utc;
use rustok_storage::StorageRuntime;
use sea_orm::{ConnectionTrait, DatabaseConnection, DbBackend, Statement, TransactionTrait};
use serde::{Deserialize, Serialize};
use std::time::Duration;
use thiserror::Error;
use uuid::Uuid;

use crate::{
    ArtifactDataObject, ArtifactDataScope,
    data::{configure_tenant_scope, placeholder, revision_value, uuid_from_row, uuid_value},
    promotion::digest_json,
};

#[derive(Debug, Error, PartialEq, Eq)]
pub enum SnapshotIntentError {
    #[error("Snapshot copy storage error: {0}")]
    Storage(String),
    #[error("Snapshot copy command is invalid")]
    InvalidCommand,
    #[error("Copy intent {0} has an invalid state")]
    InvalidIntentState(Uuid),
    #[error("Snapshot copy identity was reused with different evidence")]
    IdempotencyConflict,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SnapshotCopyKind {
    Snapshot,
    Restore,
}

impl SnapshotCopyKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Snapshot => "snapshot",
            Self::Restore => "restore",
        }
    }
}

#[derive(Serialize)]
pub(crate) struct SnapshotCopyRequest<'a> {
    pub scope: &'a ArtifactDataScope,
    pub snapshot_id: Uuid,
    pub operation_id: Uuid,
    pub operation_kind: SnapshotCopyKind,
    pub operation_request_digest: &'a str,
    pub object: &'a ArtifactDataObject,
    pub source_storage_key: &'a str,
}

pub(crate) struct SnapshotCopyIntent {
    pub intent_id: Uuid,
    pub tenant_id: Uuid,
    pub data_owner_id: Uuid,
    pub namespace_instance_id: Uuid,
    pub snapshot_id: Uuid,
    pub operation_id: Uuid,
    pub operation_kind: SnapshotCopyKind,
    pub operation_request_digest: String,
    pub object_name: String,
    pub target_storage_key: String,
    pub digest_sha256: String,
    pub size_bytes: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReconciledSnapshotIntentsReceipt {
    pub total_scanned: u64,
    pub committed_resumed: u64,
    pub retained_unresolved: u64,
}

pub struct ArtifactDataSnapshotIntentService {
    db: DatabaseConnection,
    storage: StorageRuntime,
}

impl ArtifactDataSnapshotIntentService {
    pub fn new(db: DatabaseConnection, storage: StorageRuntime) -> Self {
        Self { db, storage }
    }

    pub(crate) async fn reserve_intent(
        &self,
        request: SnapshotCopyRequest<'_>,
        candidate_target_key: &str,
    ) -> Result<SnapshotCopyIntent, SnapshotIntentError> {
        request.scope.validate().map_err(storage_error)?;
        if request.snapshot_id.is_nil()
            || request.operation_id.is_nil()
            || request.source_storage_key.is_empty()
            || candidate_target_key.is_empty()
            || !crate::promotion::valid_digest(request.operation_request_digest)
            || !crate::promotion::valid_digest(&request.object.digest_sha256)
            || request.object.name.is_empty()
        {
            return Err(SnapshotIntentError::InvalidCommand);
        }
        let request_digest = digest_json(&request).map_err(storage_error)?;
        let transaction = self.db.begin().await.map_err(storage_error)?;
        configure_tenant_scope(&transaction, request.scope.tenant_id)
            .await
            .map_err(storage_error)?;
        let backend = transaction.get_database_backend();
        let intent_id = Uuid::new_v4();
        transaction.execute_raw(Statement::from_sql_and_values(backend, format!(
            "INSERT INTO module_artifact_data_snapshot_copy_intents
             (intent_id, tenant_id, data_owner_id, namespace_instance_id, snapshot_id, operation_id,
              operation_kind, operation_request_digest, request_digest, object_name,
              source_storage_key, target_storage_key, digest_sha256, size_bytes, status, created_at)
             VALUES ({}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {}, 'intent', {})
             ON CONFLICT (tenant_id, data_owner_id, namespace_instance_id, operation_id, operation_kind, object_name)
             DO NOTHING",
            placeholder(backend,1), placeholder(backend,2), placeholder(backend,3),
            placeholder(backend,4), placeholder(backend,5), placeholder(backend,6),
            placeholder(backend,7), placeholder(backend,8), placeholder(backend,9),
            placeholder(backend,10), placeholder(backend,11), placeholder(backend,12),
            placeholder(backend,13), placeholder(backend,14), placeholder(backend,15),
        ), vec![
            uuid_value(intent_id,backend), uuid_value(request.scope.tenant_id,backend),
            uuid_value(request.scope.data_owner_id,backend), uuid_value(request.scope.namespace_instance_id,backend),
            uuid_value(request.snapshot_id,backend), uuid_value(request.operation_id,backend),
            request.operation_kind.as_str().into(), request.operation_request_digest.to_owned().into(),
            request_digest.clone().into(), request.object.name.clone().into(),
            request.source_storage_key.to_owned().into(), candidate_target_key.to_owned().into(),
            request.object.digest_sha256.clone().into(), revision_value(request.object.size_bytes).map_err(storage_error)?,
            Utc::now().into(),
        ])).await.map_err(storage_error)?;
        let row =
            transaction
                .query_one_raw(Statement::from_sql_and_values(
                    backend,
                    format!(
            "SELECT * FROM module_artifact_data_snapshot_copy_intents WHERE tenant_id = {}
             AND data_owner_id = {} AND namespace_instance_id = {} AND operation_id = {}
             AND operation_kind = {} AND object_name = {}",
            placeholder(backend,1), placeholder(backend,2), placeholder(backend,3),
            placeholder(backend,4), placeholder(backend,5), placeholder(backend,6),
        ),
                    vec![
                        uuid_value(request.scope.tenant_id, backend),
                        uuid_value(request.scope.data_owner_id, backend),
                        uuid_value(request.scope.namespace_instance_id, backend),
                        uuid_value(request.operation_id, backend),
                        request.operation_kind.as_str().into(),
                        request.object.name.clone().into(),
                    ],
                ))
                .await
                .map_err(storage_error)?
                .ok_or(SnapshotIntentError::InvalidIntentState(intent_id))?;
        if row
            .try_get::<String>("", "request_digest")
            .map_err(storage_error)?
            != request_digest
        {
            return Err(SnapshotIntentError::IdempotencyConflict);
        }
        let intent = intent_from_row(&row, backend)?;
        transaction.commit().await.map_err(storage_error)?;
        Ok(intent)
    }

    pub(crate) async fn record_staging_receipt(
        &self,
        intent: &SnapshotCopyIntent,
    ) -> Result<(), SnapshotIntentError> {
        self.verify_bytes(intent).await?;
        let transaction = self.db.begin().await.map_err(storage_error)?;
        configure_tenant_scope(&transaction, intent.tenant_id)
            .await
            .map_err(storage_error)?;
        let backend = transaction.get_database_backend();
        let result = transaction.execute_raw(Statement::from_sql_and_values(backend, format!(
            "UPDATE module_artifact_data_snapshot_copy_intents SET status = CASE
               WHEN status = 'intent' THEN 'staging' ELSE status END
             WHERE tenant_id = {} AND intent_id = {} AND status IN ('intent', 'staging', 'committed')",
            placeholder(backend,1), placeholder(backend,2),
        ), vec![uuid_value(intent.tenant_id,backend), uuid_value(intent.intent_id,backend)]))
            .await.map_err(storage_error)?;
        if result.rows_affected() != 1 {
            return Err(SnapshotIntentError::InvalidIntentState(intent.intent_id));
        }
        transaction.commit().await.map_err(storage_error)?;
        Ok(())
    }

    pub(crate) async fn verify_bytes(
        &self,
        intent: &SnapshotCopyIntent,
    ) -> Result<(), SnapshotIntentError> {
        use object_store::{ObjectStoreExt, path::Path};
        use sha2::{Digest, Sha256};
        let bytes = self
            .storage
            .objects
            .get(&Path::from(intent.target_storage_key.as_str()))
            .await
            .map_err(storage_error)?
            .bytes()
            .await
            .map_err(storage_error)?;
        let size = u64::try_from(bytes.len()).map_err(storage_error)?;
        if size != intent.size_bytes
            || format!("sha256:{}", hex::encode(Sha256::digest(&bytes))) != intent.digest_sha256
        {
            return Err(SnapshotIntentError::Storage(
                "Published copy bytes do not match the intent".into(),
            ));
        }
        Ok(())
    }

    /// Resume only metadata-bound, byte-verified receipts. Missing parents and
    /// uncertain publication remain retained for an authorized recovery action.
    pub async fn reconcile_stale_intents(
        &self,
        tenant_id: Uuid,
        grace_period: Duration,
    ) -> Result<ReconciledSnapshotIntentsReceipt, SnapshotIntentError> {
        if tenant_id.is_nil() {
            return Err(SnapshotIntentError::InvalidCommand);
        }
        let threshold = Utc::now()
            .checked_sub_signed(chrono::Duration::from_std(grace_period).map_err(storage_error)?)
            .ok_or(SnapshotIntentError::InvalidCommand)?;
        let transaction = self.db.begin().await.map_err(storage_error)?;
        configure_tenant_scope(&transaction, tenant_id)
            .await
            .map_err(storage_error)?;
        let backend = transaction.get_database_backend();
        let rows = transaction
            .query_all_raw(Statement::from_sql_and_values(
                backend,
                format!(
                    "SELECT * FROM module_artifact_data_snapshot_copy_intents WHERE tenant_id = {}
             AND status IN ('intent','staging') AND created_at < {}
             ORDER BY created_at, intent_id LIMIT 100",
                    placeholder(backend, 1),
                    placeholder(backend, 2),
                ),
                vec![uuid_value(tenant_id, backend), threshold.into()],
            ))
            .await
            .map_err(storage_error)?;
        let mut receipt = ReconciledSnapshotIntentsReceipt {
            total_scanned: u64::try_from(rows.len()).map_err(storage_error)?,
            committed_resumed: 0,
            retained_unresolved: 0,
        };
        for row in rows {
            let intent = intent_from_row(&row, backend)?;
            if !intent_metadata_committed(&transaction, &intent).await? {
                receipt.retained_unresolved += 1;
                continue;
            }
            self.verify_bytes(&intent).await?;
            mark_copy_intent_committed_on(&transaction, &intent).await?;
            receipt.committed_resumed += 1;
        }
        transaction.commit().await.map_err(storage_error)?;
        Ok(receipt)
    }
}

async fn intent_metadata_committed<C: ConnectionTrait>(
    db: &C,
    intent: &SnapshotCopyIntent,
) -> Result<bool, SnapshotIntentError> {
    let backend = db.get_database_backend();
    let sql = match intent.operation_kind {
        SnapshotCopyKind::Snapshot =>
            "SELECT 1 FROM module_artifact_data_snapshot_objects object
             JOIN module_artifact_data_snapshots snapshot ON snapshot.tenant_id = object.tenant_id
              AND snapshot.snapshot_id = object.snapshot_id
             WHERE object.tenant_id = {1} AND object.snapshot_id = {2} AND object.object_name = {3}
              AND object.snapshot_storage_key = {4} AND object.digest_sha256 = {5} AND object.size_bytes = {6}
              AND snapshot.data_owner_id = {7} AND snapshot.namespace_instance_id = {8}
              AND snapshot.request_digest = {9} AND snapshot.status IN ('staging', 'ready')",
        SnapshotCopyKind::Restore =>
            "SELECT 1 FROM module_artifact_data_objects object
             JOIN module_artifact_data_restore_operations operation ON operation.tenant_id = object.tenant_id
              AND operation.data_owner_id = object.data_owner_id
              AND operation.namespace_instance_id = object.namespace_instance_id
             WHERE object.tenant_id = {1} AND operation.snapshot_id = {2} AND object.object_name = {3}
              AND object.storage_key = {4} AND object.digest_sha256 = {5} AND object.size_bytes = {6}
              AND object.data_owner_id = {7} AND object.namespace_instance_id = {8}
              AND operation.request_digest = {9} AND operation.idempotency_key = {10}",
    };
    let mut sql = sql.to_owned();
    for index in 1..=10 {
        sql = sql.replace(&format!("{{{index}}}"), &placeholder(backend, index));
    }
    let mut values = vec![
        uuid_value(intent.tenant_id, backend),
        uuid_value(intent.snapshot_id, backend),
        intent.object_name.clone().into(),
        intent.target_storage_key.clone().into(),
        intent.digest_sha256.clone().into(),
        revision_value(intent.size_bytes).map_err(storage_error)?,
        uuid_value(intent.data_owner_id, backend),
        uuid_value(intent.namespace_instance_id, backend),
        intent.operation_request_digest.clone().into(),
    ];
    if intent.operation_kind == SnapshotCopyKind::Restore {
        values.push(uuid_value(intent.operation_id, backend));
    }
    Ok(db
        .query_one_raw(Statement::from_sql_and_values(backend, sql, values))
        .await
        .map_err(storage_error)?
        .is_some())
}

fn intent_from_row(
    row: &sea_orm::QueryResult,
    backend: DbBackend,
) -> Result<SnapshotCopyIntent, SnapshotIntentError> {
    let size: i64 = row.try_get("", "size_bytes").map_err(storage_error)?;
    let kind: String = row.try_get("", "operation_kind").map_err(storage_error)?;
    let status: String = row.try_get("", "status").map_err(storage_error)?;
    if !matches!(status.as_str(), "intent" | "staging" | "committed") {
        return Err(SnapshotIntentError::InvalidCommand);
    }
    Ok(SnapshotCopyIntent {
        intent_id: uuid_from_row(row, "intent_id", backend).map_err(storage_error)?,
        tenant_id: uuid_from_row(row, "tenant_id", backend).map_err(storage_error)?,
        data_owner_id: uuid_from_row(row, "data_owner_id", backend).map_err(storage_error)?,
        namespace_instance_id: uuid_from_row(row, "namespace_instance_id", backend)
            .map_err(storage_error)?,
        snapshot_id: uuid_from_row(row, "snapshot_id", backend).map_err(storage_error)?,
        operation_id: uuid_from_row(row, "operation_id", backend).map_err(storage_error)?,
        operation_kind: match kind.as_str() {
            "snapshot" => SnapshotCopyKind::Snapshot,
            "restore" => SnapshotCopyKind::Restore,
            _ => return Err(SnapshotIntentError::InvalidCommand),
        },
        operation_request_digest: row
            .try_get("", "operation_request_digest")
            .map_err(storage_error)?,
        object_name: row.try_get("", "object_name").map_err(storage_error)?,
        target_storage_key: row
            .try_get("", "target_storage_key")
            .map_err(storage_error)?,
        digest_sha256: row.try_get("", "digest_sha256").map_err(storage_error)?,
        size_bytes: u64::try_from(size).map_err(storage_error)?,
    })
}

pub(crate) async fn mark_copy_intent_committed_on<C: ConnectionTrait>(
    db: &C,
    intent: &SnapshotCopyIntent,
) -> Result<(), SnapshotIntentError> {
    if !intent_metadata_committed(db, intent).await? {
        return Err(SnapshotIntentError::InvalidIntentState(intent.intent_id));
    }
    let backend = db.get_database_backend();
    let updated = db
        .execute_raw(Statement::from_sql_and_values(
            backend,
            format!(
                "UPDATE module_artifact_data_snapshot_copy_intents SET status = 'committed',
         committed_at = COALESCE(committed_at, {}) WHERE tenant_id = {} AND intent_id = {}
         AND target_storage_key = {} AND status IN ('staging', 'committed')",
                placeholder(backend, 1),
                placeholder(backend, 2),
                placeholder(backend, 3),
                placeholder(backend, 4),
            ),
            vec![
                Utc::now().into(),
                uuid_value(intent.tenant_id, backend),
                uuid_value(intent.intent_id, backend),
                intent.target_storage_key.clone().into(),
            ],
        ))
        .await
        .map_err(storage_error)?;
    if updated.rows_affected() != 1 {
        return Err(SnapshotIntentError::InvalidIntentState(intent.intent_id));
    }
    Ok(())
}

fn storage_error(error: impl std::fmt::Display) -> SnapshotIntentError {
    SnapshotIntentError::Storage(error.to_string())
}
