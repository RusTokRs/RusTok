//! Garbage collection service for expired artifact data objects.

use sea_orm::{
    ConnectionTrait, DatabaseConnection, DatabaseTransaction, DbBackend, Statement,
    TransactionTrait, Value as SqlValue,
};
use uuid::Uuid;

use super::*;
use super::constants::*;
use super::error::*;
use super::helpers::*;
use super::objects_persistence::*;
use super::types::*;

/// Deletes only object bytes that an owner transaction has explicitly marked
/// unreferenced and an external retention snapshot permits. It is tenant-scoped
/// so PostgreSQL RLS remains active during candidate discovery and completion.
#[derive(Clone)]
pub struct SeaOrmArtifactDataObjectGcService {
    db: DatabaseConnection,
    storage: StorageRuntime,
}

impl SeaOrmArtifactDataObjectGcService {
    pub fn new(db: DatabaseConnection, storage: StorageRuntime) -> Self {
        Self { db, storage }
    }

    pub async fn sweep_tenant(
        &self,
        tenant_id: Uuid,
        limit: u32,
        retention: &dyn ArtifactDataObjectRetentionPolicy,
    ) -> Result<ArtifactDataObjectGcResult, ArtifactDataError> {
        if tenant_id.is_nil() || limit == 0 || limit > MAX_ARTIFACT_OBJECT_GC_BATCH_SIZE {
            return Err(ArtifactDataError::InvalidObject);
        }
        let transaction = self.db.begin().await.map_err(storage_error)?;
        configure_tenant_scope(&transaction, tenant_id).await?;
        let backend = transaction.get_database_backend();
        let candidates = transaction
            .query_all_raw(Statement::from_sql_and_values(
                backend,
                format!(
                    "SELECT candidate.candidate_id, candidate.tenant_id, candidate.data_owner_id,
                            candidate.namespace_instance_id, candidate.policy_revision, candidate.storage_key,
                            namespace.module_slug, namespace.data_contract_revision, namespace.data_contract_digest
                     FROM module_artifact_data_object_gc_candidates candidate
                     JOIN module_artifact_data_namespaces namespace
                       ON namespace.tenant_id = candidate.tenant_id AND namespace.data_owner_id = candidate.data_owner_id
                      AND namespace.namespace_instance_id = candidate.namespace_instance_id
                     WHERE candidate.tenant_id = {} ORDER BY candidate.queued_at ASC, candidate.candidate_id ASC LIMIT {}",
                    placeholder(backend, 1),
                    placeholder(backend, 2),
                ),
                vec![uuid_value(tenant_id, backend), i64::from(limit).into()],
            ))
            .await
            .map_err(storage_error)?;
        transaction.commit().await.map_err(storage_error)?;

        let mut result = ArtifactDataObjectGcResult::default();
        for row in candidates {
            let candidate = artifact_data_object_gc_candidate_from_row(row, backend)?;
            if !retention
                .may_delete(&candidate.scope, &candidate.storage_key)
                .await?
            {
                result.retained = result
                    .retained
                    .checked_add(1)
                    .ok_or(ArtifactDataError::Storage("GC result overflow".to_string()))?;
                continue;
            }
            let transaction = self.db.begin().await.map_err(storage_error)?;
            configure_tenant_scope(&transaction, tenant_id).await?;
            let backend = transaction.get_database_backend();
            transaction
                .query_one_raw(Statement::from_sql_and_values(
                    backend,
                    format!(
                        "SELECT namespace_revision FROM module_artifact_data_namespaces
                         WHERE tenant_id = {} AND data_owner_id = {} AND namespace_instance_id = {}{}",
                        placeholder(backend, 1),
                        placeholder(backend, 2),
                        placeholder(backend, 3),
                        namespace_lock_clause(backend),
                    ),
                    namespace_values(&candidate.scope, backend)?,
                ))
                .await
                .map_err(storage_error)?;
            let snapshot_reference = transaction
                .query_one_raw(Statement::from_sql_and_values(
                    backend,
                    format!(
                        "SELECT COUNT(*) AS reference_count
                         FROM module_artifact_data_snapshot_objects snapshot_object
                         JOIN module_artifact_data_snapshots snapshot
                           ON snapshot.snapshot_id = snapshot_object.snapshot_id
                         WHERE snapshot_object.tenant_id = {}
                           AND snapshot_object.source_storage_key = {}
                           AND snapshot.status = 'staging'",
                        placeholder(backend, 1),
                        placeholder(backend, 2),
                    ),
                    vec![
                        uuid_value(tenant_id, backend),
                        candidate.storage_key.clone().into(),
                    ],
                ))
                .await
                .map_err(storage_error)?
                .ok_or_else(|| {
                    ArtifactDataError::Storage(
                        "snapshot reference query returned no row".to_string(),
                    )
                })?;
            let reference_count: i64 = snapshot_reference
                .try_get("", "reference_count")
                .map_err(storage_error)?;
            if reference_count != 0 {
                transaction.commit().await.map_err(storage_error)?;
                result.retained = result
                    .retained
                    .checked_add(1)
                    .ok_or(ArtifactDataError::Storage("GC result overflow".to_string()))?;
                continue;
            }
            self.storage
                .objects
                .delete(&Path::from(candidate.storage_key.as_str()))
                .await
                .map_err(storage_error)?;
            transaction
                .execute_raw(Statement::from_sql_and_values(
                    backend,
                    format!(
                        "DELETE FROM module_artifact_data_object_gc_candidates
                         WHERE tenant_id = {} AND candidate_id = {} AND storage_key = {}",
                        placeholder(backend, 1),
                        placeholder(backend, 2),
                        placeholder(backend, 3),
                    ),
                    vec![
                        uuid_value(tenant_id, backend),
                        uuid_value(candidate.candidate_id, backend),
                        candidate.storage_key.into(),
                    ],
                ))
                .await
                .map_err(storage_error)?;
            transaction.commit().await.map_err(storage_error)?;
            result.deleted = result
                .deleted
                .checked_add(1)
                .ok_or(ArtifactDataError::Storage("GC result overflow".to_string()))?;
        }
        Ok(result)
    }
}

pub(crate) struct ArtifactDataObjectGcCandidate {
    pub(crate) candidate_id: Uuid,
    pub(crate) scope: ArtifactDataScope,
    pub(crate) storage_key: String,
}

pub(crate) fn artifact_data_object_gc_candidate_from_row(
    row: sea_orm::QueryResult,
    backend: DbBackend,
) -> Result<ArtifactDataObjectGcCandidate, ArtifactDataError> {
    let data_contract_revision: i64 = row
        .try_get("", "data_contract_revision")
        .map_err(storage_error)?;
    let policy_revision: i64 = row.try_get("", "policy_revision").map_err(storage_error)?;
    let scope = ArtifactDataScope {
        tenant_id: uuid_from_row(&row, "tenant_id", backend)?,
        data_owner_id: uuid_from_row(&row, "data_owner_id", backend)?,
        namespace_instance_id: uuid_from_row(&row, "namespace_instance_id", backend)?,
        data_contract_digest: row
            .try_get("", "data_contract_digest")
            .map_err(storage_error)?,
        module_slug: row.try_get("", "module_slug").map_err(storage_error)?,
        data_contract_revision: u64::try_from(data_contract_revision)
            .map_err(|_| ArtifactDataError::InvalidObject)?,
        policy_revision: u64::try_from(policy_revision)
            .map_err(|_| ArtifactDataError::InvalidObject)?,
    };
    scope.validate()?;
    Ok(ArtifactDataObjectGcCandidate {
        candidate_id: uuid_from_row(&row, "candidate_id", backend)?,
        scope,
        storage_key: row.try_get("", "storage_key").map_err(storage_error)?,
    })
}
