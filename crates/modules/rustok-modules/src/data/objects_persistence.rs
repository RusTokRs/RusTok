//! Low-level database operations for stored artifact data objects.

use bytes::Bytes;
use sea_orm::{
    ConnectionTrait, DatabaseConnection, DatabaseTransaction, DbBackend, QueryResult, Statement,
    TransactionTrait, Value as SqlValue,
};
use uuid::Uuid;

use super::constants::*;
use super::error::*;
use super::helpers::*;
use super::types::*;
use super::validation::*;
use super::*;

#[derive(Clone)]
pub(crate) struct StoredArtifactDataObject {
    pub(crate) object: ArtifactDataObject,
    pub(crate) storage_key: String,
}

pub(crate) async fn find_artifact_data_object<C: ConnectionTrait>(
    connection: &C,
    scope: &ArtifactDataScope,
    name: &str,
) -> Result<Option<StoredArtifactDataObject>, ArtifactDataError> {
    let backend = connection.get_database_backend();
    connection
        .query_one_raw(Statement::from_sql_and_values(
            backend,
            format!(
                "SELECT object_name, content_type, size_bytes, digest_sha256, revision, storage_key
                 FROM module_artifact_data_objects
                 WHERE tenant_id = {} AND data_owner_id = {} AND namespace_instance_id = {} AND object_name = {}",
                placeholder(backend, 1),
                placeholder(backend, 2),
                placeholder(backend, 3),
                placeholder(backend, 4),
            ),
            scope_values(scope, backend, name)?,
        ))
        .await
        .map_err(storage_error)?
        .map(stored_artifact_data_object_from_row)
        .transpose()
}

pub(crate) async fn find_artifact_data_object_delete_operation<C: ConnectionTrait>(
    connection: &C,
    scope: &ArtifactDataScope,
    request: &ArtifactDataObjectDeleteRequest,
) -> Result<Option<ArtifactDataObjectDeleteResult>, ArtifactDataError> {
    let backend = connection.get_database_backend();
    let row = connection
        .query_one_raw(Statement::from_sql_and_values(
            backend,
            format!(
                "SELECT object_name, expected_revision, deleted_revision
                 FROM module_artifact_data_object_delete_operations
                 WHERE tenant_id = {} AND data_owner_id = {} AND namespace_instance_id = {}
                 AND policy_revision = {} AND idempotency_key = {}",
                placeholder(backend, 1),
                placeholder(backend, 2),
                placeholder(backend, 3),
                placeholder(backend, 4),
                placeholder(backend, 5),
            ),
            vec![
                uuid_value(scope.tenant_id, backend),
                uuid_value(scope.data_owner_id, backend),
                uuid_value(scope.namespace_instance_id, backend),
                revision_value(scope.policy_revision)?,
                uuid_value(request.idempotency_key, backend),
            ],
        ))
        .await
        .map_err(storage_error)?;
    let Some(row) = row else {
        return Ok(None);
    };
    let name: String = row.try_get("", "object_name").map_err(storage_error)?;
    let expected_revision: i64 = row
        .try_get("", "expected_revision")
        .map_err(storage_error)?;
    let deleted_revision: i64 = row.try_get("", "deleted_revision").map_err(storage_error)?;
    let expected_revision =
        u64::try_from(expected_revision).map_err(|_| ArtifactDataError::IdempotencyConflict)?;
    let deleted_revision =
        u64::try_from(deleted_revision).map_err(|_| ArtifactDataError::IdempotencyConflict)?;
    if name != request.name
        || expected_revision != request.expected_revision
        || deleted_revision != request.expected_revision
    {
        return Err(ArtifactDataError::IdempotencyConflict);
    }
    Ok(Some(ArtifactDataObjectDeleteResult {
        name,
        deleted_revision,
    }))
}

pub(crate) async fn persist_artifact_data_object_delete_operation<C: ConnectionTrait>(
    connection: &C,
    scope: &ArtifactDataScope,
    request: &ArtifactDataObjectDeleteRequest,
) -> Result<(), ArtifactDataError> {
    let backend = connection.get_database_backend();
    connection
        .execute_raw(Statement::from_sql_and_values(
            backend,
            format!(
                "INSERT INTO module_artifact_data_object_delete_operations
                 (tenant_id, data_owner_id, namespace_instance_id, policy_revision, idempotency_key,
                  object_name, expected_revision, deleted_revision, completed_at)
                 VALUES ({}, {}, {}, {}, {}, {}, {}, {}, {})",
                placeholder(backend, 1),
                placeholder(backend, 2),
                placeholder(backend, 3),
                placeholder(backend, 4),
                placeholder(backend, 5),
                placeholder(backend, 6),
                placeholder(backend, 7),
                placeholder(backend, 8),
                now_expression(backend),
            ),
            vec![
                uuid_value(scope.tenant_id, backend),
                uuid_value(scope.data_owner_id, backend),
                uuid_value(scope.namespace_instance_id, backend),
                revision_value(scope.policy_revision)?,
                uuid_value(request.idempotency_key, backend),
                request.name.clone().into(),
                revision_value(request.expected_revision)?,
                revision_value(request.expected_revision)?,
            ],
        ))
        .await
        .map_err(storage_error)?;
    Ok(())
}

pub(crate) async fn queue_artifact_data_object_gc_candidate<C: ConnectionTrait>(
    connection: &C,
    infrastructure: &ControlPlaneInfrastructure,
    scope: &ArtifactDataScope,
    storage_key: &str,
) -> Result<(), ArtifactDataError> {
    let backend = connection.get_database_backend();
    connection
        .execute_raw(Statement::from_sql_and_values(
            backend,
            format!(
                "INSERT INTO module_artifact_data_object_gc_candidates
                 (candidate_id, tenant_id, data_owner_id, namespace_instance_id, policy_revision, storage_key, queued_at)
                 VALUES ({}, {}, {}, {}, {}, {}, {}) ON CONFLICT (storage_key) DO NOTHING",
                placeholder(backend, 1),
                placeholder(backend, 2),
                placeholder(backend, 3),
                placeholder(backend, 4),
                placeholder(backend, 5),
                placeholder(backend, 6),
                now_expression(backend),
            ),
            vec![
                uuid_value(infrastructure.new_id(), backend),
                uuid_value(scope.tenant_id, backend),
                uuid_value(scope.data_owner_id, backend),
                uuid_value(scope.namespace_instance_id, backend),
                revision_value(scope.policy_revision)?,
                storage_key.to_owned().into(),
            ],
        ))
        .await
        .map_err(storage_error)?;
    Ok(())
}

pub(crate) async fn persist_artifact_data_object(
    transaction: &DatabaseTransaction,
    infrastructure: &ControlPlaneInfrastructure,
    scope: &ArtifactDataScope,
    upload: &ArtifactDataObjectUpload,
    requested: &ArtifactDataObject,
    storage_key: &str,
    quota: ArtifactDataQuota,
) -> Result<StoredArtifactDataObject, ArtifactDataError> {
    let quota = quota.validate()?;
    let backend = transaction.get_database_backend();
    ensure_active_namespace(transaction, scope, backend).await?;
    if let Some((existing, expected_revision)) =
        find_artifact_data_object_operation(transaction, scope, upload.idempotency_key).await?
    {
        validate_object_operation(&existing, upload, requested, expected_revision)?;
        return Ok(existing);
    }
    let current = find_artifact_data_object(transaction, scope, &requested.name).await?;
    enforce_object_data_quota(
        transaction,
        scope,
        quota,
        current.as_ref().map(|stored| stored.object.size_bytes),
        requested.size_bytes,
    )
    .await?;
    let revision = match current {
        Some(current) => {
            if upload.expected_revision != Some(current.object.revision) {
                return Err(ArtifactDataError::RevisionConflict);
            }
            let revision = current
                .object
                .revision
                .checked_add(1)
                .ok_or(ArtifactDataError::RevisionConflict)?;
            let result = transaction
                .execute_raw(Statement::from_sql_and_values(
                    backend,
                    format!(
                        "UPDATE module_artifact_data_objects
                         SET storage_key = {}, content_type = {}, size_bytes = {}, digest_sha256 = {}, revision = {}, updated_at = {}
                         WHERE tenant_id = {} AND data_owner_id = {} AND namespace_instance_id = {} AND object_name = {} AND revision = {}",
                        placeholder(backend, 1), placeholder(backend, 2), placeholder(backend, 3),
                        placeholder(backend, 4), placeholder(backend, 5), now_expression(backend),
                        placeholder(backend, 6), placeholder(backend, 7), placeholder(backend, 8),
                        placeholder(backend, 9), placeholder(backend, 10),
                    ),
                    vec![
                        storage_key.to_owned().into(), requested.content_type.clone().into(),
                        revision_value(requested.size_bytes)?, requested.digest_sha256.clone().into(),
                        revision_value(revision)?, uuid_value(scope.tenant_id, backend),
                        uuid_value(scope.data_owner_id, backend), uuid_value(scope.namespace_instance_id, backend),
                        requested.name.clone().into(), revision_value(current.object.revision)?,
                    ],
                ))
                .await
                .map_err(storage_error)?;
            if result.rows_affected() != 1 {
                return Err(ArtifactDataError::RevisionConflict);
            }
            if current.storage_key != storage_key {
                queue_artifact_data_object_gc_candidate(
                    transaction,
                    infrastructure,
                    scope,
                    &current.storage_key,
                )
                .await?;
            }
            revision
        }
        None => {
            if upload.expected_revision.is_some() {
                return Err(ArtifactDataError::RevisionConflict);
            }
            let result = transaction
                .execute_raw(Statement::from_sql_and_values(
                    backend,
                    format!(
                        "INSERT INTO module_artifact_data_objects
                         (tenant_id, data_owner_id, namespace_instance_id, object_name, storage_key, content_type, size_bytes, digest_sha256, revision, created_at, updated_at)
                         VALUES ({}, {}, {}, {}, {}, {}, {}, {}, 1, {}, {}) ON CONFLICT DO NOTHING",
                        placeholder(backend, 1), placeholder(backend, 2), placeholder(backend, 3),
                        placeholder(backend, 4), placeholder(backend, 5), placeholder(backend, 6),
                        placeholder(backend, 7), placeholder(backend, 8), now_expression(backend), now_expression(backend),
                    ),
                    vec![
                        uuid_value(scope.tenant_id, backend), uuid_value(scope.data_owner_id, backend),
                        uuid_value(scope.namespace_instance_id, backend), requested.name.clone().into(),
                        storage_key.to_owned().into(), requested.content_type.clone().into(),
                        revision_value(requested.size_bytes)?, requested.digest_sha256.clone().into(),
                    ],
                ))
                .await
                .map_err(storage_error)?;
            if result.rows_affected() != 1 {
                return Err(ArtifactDataError::RevisionConflict);
            }
            1
        }
    };
    let stored = StoredArtifactDataObject {
        object: ArtifactDataObject {
            name: requested.name.clone(),
            content_type: requested.content_type.clone(),
            size_bytes: requested.size_bytes,
            digest_sha256: requested.digest_sha256.clone(),
            revision,
        },
        storage_key: storage_key.to_owned(),
    };
    transaction
        .execute_raw(Statement::from_sql_and_values(
            backend,
            format!(
                "INSERT INTO module_artifact_data_object_operations
                 (tenant_id, data_owner_id, namespace_instance_id, policy_revision, idempotency_key, object_name, storage_key, content_type, size_bytes, digest_sha256, expected_revision, revision, completed_at)
                 VALUES ({}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {})",
                placeholder(backend, 1), placeholder(backend, 2), placeholder(backend, 3), placeholder(backend, 4),
                placeholder(backend, 5), placeholder(backend, 6), placeholder(backend, 7), placeholder(backend, 8),
                placeholder(backend, 9), placeholder(backend, 10), placeholder(backend, 11), placeholder(backend, 12),
                now_expression(backend),
            ),
            vec![
                uuid_value(scope.tenant_id, backend), uuid_value(scope.data_owner_id, backend),
                uuid_value(scope.namespace_instance_id, backend), revision_value(scope.policy_revision)?,
                uuid_value(upload.idempotency_key, backend),
                stored.object.name.clone().into(), stored.storage_key.clone().into(),
                stored.object.content_type.clone().into(), revision_value(stored.object.size_bytes)?,
                stored.object.digest_sha256.clone().into(), optional_revision_value(upload.expected_revision)?,
                revision_value(stored.object.revision)?,
            ],
        ))
        .await
        .map_err(storage_error)?;
    Ok(stored)
}

pub(crate) async fn enforce_object_data_quota<C: ConnectionTrait>(
    connection: &C,
    scope: &ArtifactDataScope,
    quota: ArtifactDataQuota,
    current_size_bytes: Option<u64>,
    requested_size_bytes: u64,
) -> Result<(), ArtifactDataError> {
    let backend = connection.get_database_backend();
    let row = connection
        .query_one_raw(Statement::from_sql_and_values(
            backend,
            format!(
                "SELECT COUNT(*) AS object_count, COALESCE(SUM(size_bytes), 0) AS total_bytes
                 FROM module_artifact_data_objects
                 WHERE tenant_id = {} AND data_owner_id = {} AND namespace_instance_id = {}",
                placeholder(backend, 1),
                placeholder(backend, 2),
                placeholder(backend, 3),
            ),
            namespace_values(scope, backend)?,
        ))
        .await
        .map_err(storage_error)?
        .ok_or_else(|| {
            ArtifactDataError::Storage("artifact object usage was not computed".to_string())
        })?;
    let object_count = nonnegative_usage(row.try_get("", "object_count").map_err(storage_error)?)?;
    let total_bytes = nonnegative_usage(row.try_get("", "total_bytes").map_err(storage_error)?)?;
    let projected_objects = object_count
        .checked_add(u64::from(current_size_bytes.is_none()))
        .ok_or_else(|| ArtifactDataError::Storage("object quota overflow".to_string()))?;
    let projected_bytes = total_bytes
        .checked_sub(current_size_bytes.unwrap_or(0))
        .and_then(|bytes| bytes.checked_add(requested_size_bytes))
        .ok_or_else(|| ArtifactDataError::Storage("object byte quota overflow".to_string()))?;
    enforce_quota("objects", quota.max_objects, projected_objects)?;
    enforce_quota("object_bytes", quota.max_object_bytes, projected_bytes)
}

pub(crate) async fn find_artifact_data_object_operation<C: ConnectionTrait>(
    connection: &C,
    scope: &ArtifactDataScope,
    idempotency_key: Uuid,
) -> Result<Option<(StoredArtifactDataObject, Option<u64>)>, ArtifactDataError> {
    let backend = connection.get_database_backend();
    connection
        .query_one_raw(Statement::from_sql_and_values(
            backend,
            format!(
                "SELECT object_name, content_type, size_bytes, digest_sha256, revision, storage_key, expected_revision
                 FROM module_artifact_data_object_operations
                 WHERE tenant_id = {} AND data_owner_id = {} AND namespace_instance_id = {}
                 AND policy_revision = {} AND idempotency_key = {}",
                placeholder(backend, 1), placeholder(backend, 2), placeholder(backend, 3),
                placeholder(backend, 4), placeholder(backend, 5),
            ),
            vec![
                uuid_value(scope.tenant_id, backend), uuid_value(scope.data_owner_id, backend),
                uuid_value(scope.namespace_instance_id, backend), revision_value(scope.policy_revision)?,
                uuid_value(idempotency_key, backend),
            ],
        ))
        .await
        .map_err(storage_error)?
        .map(|row| {
            let expected_revision: Option<i64> = row
                .try_get("", "expected_revision")
                .map_err(storage_error)?;
            let expected_revision = expected_revision
                .map(u64::try_from)
                .transpose()
                .map_err(|_| ArtifactDataError::IdempotencyConflict)?;
            Ok((stored_artifact_data_object_from_row(row)?, expected_revision))
        })
        .transpose()
}

pub(crate) fn validate_object_operation(
    stored: &StoredArtifactDataObject,
    upload: &ArtifactDataObjectUpload,
    requested: &ArtifactDataObject,
    expected_revision: Option<u64>,
) -> Result<(), ArtifactDataError> {
    if stored.object.name != requested.name
        || stored.object.content_type != requested.content_type
        || stored.object.size_bytes != requested.size_bytes
        || stored.object.digest_sha256 != requested.digest_sha256
        || expected_revision != upload.expected_revision
    {
        return Err(ArtifactDataError::IdempotencyConflict);
    }
    Ok(())
}

pub(crate) fn stored_artifact_data_object_from_row(
    row: sea_orm::QueryResult,
) -> Result<StoredArtifactDataObject, ArtifactDataError> {
    let revision: i64 = row.try_get("", "revision").map_err(storage_error)?;
    let size_bytes: i64 = row.try_get("", "size_bytes").map_err(storage_error)?;
    let object = ArtifactDataObject {
        name: row.try_get("", "object_name").map_err(storage_error)?,
        content_type: row.try_get("", "content_type").map_err(storage_error)?,
        size_bytes: u64::try_from(size_bytes).map_err(|_| ArtifactDataError::ObjectIntegrity)?,
        digest_sha256: row.try_get("", "digest_sha256").map_err(storage_error)?,
        revision: u64::try_from(revision).map_err(|_| ArtifactDataError::ObjectIntegrity)?,
    };
    object.validate()?;
    Ok(StoredArtifactDataObject {
        object,
        storage_key: row.try_get("", "storage_key").map_err(storage_error)?,
    })
}
