//! Low-level database operations for structured artifact data records.

use sea_orm::{
    ConnectionTrait, DatabaseConnection, DatabaseTransaction, QueryResult, Statement,
    TransactionTrait, Value as SqlValue,
};
use serde_json::Value;
use uuid::Uuid;

use super::constants::*;
use super::error::*;
use super::helpers::*;
use super::types::*;
use super::validation::*;
use super::*;

pub(crate) async fn find_artifact_data_delete_operation<C: ConnectionTrait>(
    connection: &C,
    scope: &ArtifactDataScope,
    request: &ArtifactDataDeleteRequest,
) -> Result<Option<ArtifactDataDeleteResult>, ArtifactDataError> {
    let backend =
        SupportedBackend::new(connection.get_database_backend()).map_err(storage_error)?;
    let row = connection
        .query_one_raw(Statement::from_sql_and_values(
            backend.db_backend(),
            format!(
                "SELECT data_key, expected_revision, deleted_revision
                 FROM module_artifact_data_delete_operations
                 WHERE tenant_id = {} AND data_owner_id = {} AND namespace_instance_id = {}
                 AND policy_revision = {} AND idempotency_key = {}",
                backend.placeholder(1),
                backend.placeholder(2),
                backend.placeholder(3),
                backend.placeholder(4),
                backend.placeholder(5),
            ),
            vec![
                backend.uuid_value(scope.tenant_id),
                backend.uuid_value(scope.data_owner_id),
                backend.uuid_value(scope.namespace_instance_id),
                revision_value(scope.policy_revision)?,
                backend.uuid_value(request.idempotency_key),
            ],
        ))
        .await
        .map_err(storage_error)?;
    let Some(row) = row else {
        return Ok(None);
    };
    let key: String = row.try_get("", "data_key").map_err(storage_error)?;
    let expected_revision: i64 = row
        .try_get("", "expected_revision")
        .map_err(storage_error)?;
    let deleted_revision: i64 = row.try_get("", "deleted_revision").map_err(storage_error)?;
    let expected_revision =
        u64::try_from(expected_revision).map_err(|_| ArtifactDataError::IdempotencyConflict)?;
    let deleted_revision =
        u64::try_from(deleted_revision).map_err(|_| ArtifactDataError::IdempotencyConflict)?;
    if key != request.key
        || expected_revision != request.expected_revision
        || deleted_revision != request.expected_revision
    {
        return Err(ArtifactDataError::IdempotencyConflict);
    }
    Ok(Some(ArtifactDataDeleteResult {
        key,
        deleted_revision,
    }))
}

pub(crate) async fn persist_artifact_data_delete_operation<C: ConnectionTrait>(
    connection: &C,
    scope: &ArtifactDataScope,
    request: &ArtifactDataDeleteRequest,
) -> Result<(), ArtifactDataError> {
    let backend =
        SupportedBackend::new(connection.get_database_backend()).map_err(storage_error)?;
    connection
        .execute_raw(Statement::from_sql_and_values(
            backend.db_backend(),
            format!(
                "INSERT INTO module_artifact_data_delete_operations
                 (tenant_id, data_owner_id, namespace_instance_id, policy_revision,
                  idempotency_key, data_key, expected_revision, deleted_revision, completed_at)
                 VALUES ({}, {}, {}, {}, {}, {}, {}, {}, {})",
                backend.placeholder(1),
                backend.placeholder(2),
                backend.placeholder(3),
                backend.placeholder(4),
                backend.placeholder(5),
                backend.placeholder(6),
                backend.placeholder(7),
                backend.placeholder(8),
                backend.now_sql(),
            ),
            vec![
                backend.uuid_value(scope.tenant_id),
                backend.uuid_value(scope.data_owner_id),
                backend.uuid_value(scope.namespace_instance_id),
                revision_value(scope.policy_revision)?,
                backend.uuid_value(request.idempotency_key),
                request.key.clone().into(),
                revision_value(request.expected_revision)?,
                revision_value(request.expected_revision)?,
            ],
        ))
        .await
        .map_err(storage_error)?;
    Ok(())
}

pub(crate) async fn persist_artifact_data_write(
    transaction: &DatabaseTransaction,
    scope: &ArtifactDataScope,
    write: ArtifactDataWrite,
    indexes: &[ArtifactDataIndexField],
    index_contract_digest: Option<&str>,
    quota: ArtifactDataQuota,
) -> Result<ArtifactDataRecord, ArtifactDataError> {
    let quota = quota.validate()?;
    let backend =
        SupportedBackend::new(transaction.get_database_backend()).map_err(storage_error)?;
    ensure_active_namespace(transaction, scope, backend).await?;
    if let Some(index_contract_digest) = index_contract_digest {
        validate_artifact_data_index_contract(
            transaction,
            scope,
            backend,
            index_contract_digest,
            true,
        )
        .await?;
    }
    if let Some(row) = transaction
        .query_one_raw(Statement::from_sql_and_values(
            backend.db_backend(),
            format!(
                "SELECT data_key, value, revision, expected_revision FROM module_artifact_data_operations
                 WHERE tenant_id = {} AND data_owner_id = {} AND namespace_instance_id = {}
                 AND policy_revision = {} AND idempotency_key = {}",
                backend.placeholder(1),
                backend.placeholder(2),
                backend.placeholder(3),
                backend.placeholder(4),
                backend.placeholder(5),
            ),
            vec![
                backend.uuid_value(scope.tenant_id),
                backend.uuid_value(scope.data_owner_id),
                backend.uuid_value(scope.namespace_instance_id),
                revision_value(scope.policy_revision)?,
                backend.uuid_value(write.idempotency_key),
            ],
        ))
        .await
        .map_err(storage_error)?
    {
        let expected_revision: Option<i64> = row
            .try_get("", "expected_revision")
            .map_err(storage_error)?;
        let record = record_from_row(row)?;
        if record.key != write.key
            || record.value != write.value
            || expected_revision
                .map(u64::try_from)
                .transpose()
                .map_err(|_| ArtifactDataError::IdempotencyConflict)?
                != write.expected_revision
        {
            return Err(ArtifactDataError::IdempotencyConflict);
        }
        return Ok(record);
    }

    let value_size_bytes = artifact_data_value_size(&write.value)?;
    let current = transaction
        .query_one_raw(Statement::from_sql_and_values(
            backend.db_backend(),
            format!(
                "SELECT data_key, value, value_size_bytes, revision FROM module_artifact_data
                 WHERE tenant_id = {} AND data_owner_id = {} AND namespace_instance_id = {} AND data_key = {}",
                backend.placeholder(1),
                backend.placeholder(2),
                backend.placeholder(3),
                backend.placeholder(4),
            ),
            scope_values(scope, backend, &write.key)?,
        ))
        .await
        .map_err(storage_error)?;
    let current_size_bytes = current
        .as_ref()
        .map(|row| {
            row.try_get::<i64>("", "value_size_bytes")
                .map_err(storage_error)
                .and_then(nonnegative_usage)
        })
        .transpose()?;
    enforce_structured_data_quota(
        transaction,
        scope,
        quota,
        current_size_bytes,
        value_size_bytes,
    )
    .await?;
    let revision = if let Some(row) = current {
        let current = record_from_row(row)?;
        if write.create_only || write.expected_revision != Some(current.revision) {
            return Err(ArtifactDataError::RevisionConflict);
        }
        let next_revision = current
            .revision
            .checked_add(1)
            .ok_or(ArtifactDataError::RevisionConflict)?;
        let result = transaction
            .execute_raw(Statement::from_sql_and_values(
                backend.db_backend(),
                format!(
                    "UPDATE module_artifact_data SET value = {}, value_size_bytes = {}, revision = {}, updated_at = {}
                     WHERE tenant_id = {} AND data_owner_id = {} AND namespace_instance_id = {}
                     AND data_key = {} AND revision = {}",
                    backend.placeholder(1),
                    backend.placeholder(2),
                    backend.placeholder(3),
                    backend.now_sql(),
                    backend.placeholder(4),
                    backend.placeholder(5),
                    backend.placeholder(6),
                    backend.placeholder(7),
                    backend.placeholder(8),
                ),
                vec![
                    SqlValue::Json(Some(Box::new(write.value.clone()))),
                    revision_value(value_size_bytes)?,
                    revision_value(next_revision)?,
                    backend.uuid_value(scope.tenant_id),
                    backend.uuid_value(scope.data_owner_id),
                    backend.uuid_value(scope.namespace_instance_id),
                    write.key.clone().into(),
                    revision_value(current.revision)?,
                ],
            ))
            .await
            .map_err(storage_error)?;
        if result.rows_affected() != 1 {
            return Err(ArtifactDataError::RevisionConflict);
        }
        next_revision
    } else {
        if write.expected_revision.is_some() {
            return Err(ArtifactDataError::RevisionConflict);
        }
        let result = transaction
            .execute_raw(Statement::from_sql_and_values(
                backend.db_backend(),
                format!(
                    "INSERT INTO module_artifact_data
                     (tenant_id, data_owner_id, namespace_instance_id, data_key, value, value_size_bytes, revision, updated_at)
                     VALUES ({}, {}, {}, {}, {}, {}, 1, {}) ON CONFLICT DO NOTHING",
                    backend.placeholder(1),
                    backend.placeholder(2),
                    backend.placeholder(3),
                    backend.placeholder(4),
                    backend.placeholder(5),
                    backend.placeholder(6),
                    backend.now_sql(),
                ),
                vec![
                    backend.uuid_value(scope.tenant_id),
                    backend.uuid_value(scope.data_owner_id),
                    backend.uuid_value(scope.namespace_instance_id),
                    write.key.clone().into(),
                    SqlValue::Json(Some(Box::new(write.value.clone()))),
                    revision_value(value_size_bytes)?,
                ],
            ))
            .await
            .map_err(storage_error)?;
        if result.rows_affected() != 1 {
            return Err(ArtifactDataError::RevisionConflict);
        }
        1
    };
    let record = ArtifactDataRecord {
        key: write.key,
        value: write.value,
        revision,
    };
    synchronize_artifact_data_indexes(transaction, scope, &record, indexes).await?;
    transaction
        .execute_raw(Statement::from_sql_and_values(
            backend.db_backend(),
            format!(
                "INSERT INTO module_artifact_data_operations
                 (tenant_id, data_owner_id, namespace_instance_id, policy_revision, idempotency_key, data_key, value, expected_revision, revision, completed_at)
                 VALUES ({}, {}, {}, {}, {}, {}, {}, {}, {}, {})",
                backend.placeholder(1),
                backend.placeholder(2),
                backend.placeholder(3),
                backend.placeholder(4),
                backend.placeholder(5),
                backend.placeholder(6),
                backend.placeholder(7),
                backend.placeholder(8),
                backend.placeholder(9),
                backend.now_sql(),
            ),
            vec![
                backend.uuid_value(scope.tenant_id),
                backend.uuid_value(scope.data_owner_id),
                backend.uuid_value(scope.namespace_instance_id),
                revision_value(scope.policy_revision)?,
                backend.uuid_value(write.idempotency_key),
                record.key.clone().into(),
                SqlValue::Json(Some(Box::new(record.value.clone()))),
                optional_revision_value(write.expected_revision)?,
                revision_value(record.revision)?,
            ],
        ))
        .await
        .map_err(storage_error)?;
    Ok(record)
}

pub(crate) async fn enforce_structured_data_quota<C: ConnectionTrait>(
    connection: &C,
    scope: &ArtifactDataScope,
    quota: ArtifactDataQuota,
    current_size_bytes: Option<u64>,
    requested_size_bytes: u64,
) -> Result<(), ArtifactDataError> {
    let backend =
        SupportedBackend::new(connection.get_database_backend()).map_err(storage_error)?;
    let row = connection
        .query_one_raw(Statement::from_sql_and_values(
            backend.db_backend(),
            format!(
                "SELECT COUNT(*) AS record_count, COALESCE(SUM(value_size_bytes), 0) AS total_bytes
                 FROM module_artifact_data
                 WHERE tenant_id = {} AND data_owner_id = {} AND namespace_instance_id = {}",
                backend.placeholder(1),
                backend.placeholder(2),
                backend.placeholder(3),
            ),
            namespace_values(scope, backend)?,
        ))
        .await
        .map_err(storage_error)?
        .ok_or_else(|| {
            ArtifactDataError::Storage("structured data usage was not computed".to_string())
        })?;
    let record_count = nonnegative_usage(row.try_get("", "record_count").map_err(storage_error)?)?;
    let total_bytes = nonnegative_usage(row.try_get("", "total_bytes").map_err(storage_error)?)?;
    let projected_records = record_count
        .checked_add(u64::from(current_size_bytes.is_none()))
        .ok_or_else(|| ArtifactDataError::Storage("record quota overflow".to_string()))?;
    let projected_bytes = total_bytes
        .checked_sub(current_size_bytes.unwrap_or(0))
        .and_then(|bytes| bytes.checked_add(requested_size_bytes))
        .ok_or_else(|| ArtifactDataError::Storage("structured byte quota overflow".to_string()))?;
    enforce_quota(
        "structured_records",
        quota.max_structured_records,
        projected_records,
    )?;
    enforce_quota(
        "structured_bytes",
        quota.max_structured_bytes,
        projected_bytes,
    )
}

pub(crate) async fn synchronize_artifact_data_indexes(
    transaction: &DatabaseTransaction,
    scope: &ArtifactDataScope,
    record: &ArtifactDataRecord,
    indexes: &[ArtifactDataIndexField],
) -> Result<(), ArtifactDataError> {
    delete_artifact_data_indexes(transaction, scope, &record.key).await?;
    let backend =
        SupportedBackend::new(transaction.get_database_backend()).map_err(storage_error)?;
    for index in indexes {
        let Some(value) = record.value.pointer(&index.json_pointer) else {
            continue;
        };
        if !artifact_data_index_value_matches(value, index.value_type) {
            return Err(ArtifactDataError::InvalidIndexQuery);
        }
        let index_value = canonical_index_value(value)?;
        if index_value.len() > MAX_ARTIFACT_DATA_INDEX_VALUE_BYTES {
            return Err(ArtifactDataError::InvalidIndexQuery);
        }
        transaction
            .execute_raw(Statement::from_sql_and_values(
                backend.db_backend(),
                format!(
                    "INSERT INTO module_artifact_data_indexes
                     (tenant_id, data_owner_id, namespace_instance_id, index_name, index_value, data_key)
                     VALUES ({}, {}, {}, {}, {}, {})",
                    backend.placeholder(1),
                    backend.placeholder(2),
                    backend.placeholder(3),
                    backend.placeholder(4),
                    backend.placeholder(5),
                    backend.placeholder(6),
                ),
                vec![
                    backend.uuid_value(scope.tenant_id),
                    backend.uuid_value(scope.data_owner_id),
                    backend.uuid_value(scope.namespace_instance_id),
                    index.name.clone().into(),
                    index_value.into(),
                    record.key.clone().into(),
                ],
            ))
            .await
            .map_err(storage_error)?;
    }
    Ok(())
}

pub(crate) async fn delete_artifact_data_indexes<C: ConnectionTrait>(
    connection: &C,
    scope: &ArtifactDataScope,
    key: &str,
) -> Result<(), ArtifactDataError> {
    let backend =
        SupportedBackend::new(connection.get_database_backend()).map_err(storage_error)?;
    connection
        .execute_raw(Statement::from_sql_and_values(
            backend.db_backend(),
            format!(
                "DELETE FROM module_artifact_data_indexes
                 WHERE tenant_id = {} AND data_owner_id = {} AND namespace_instance_id = {}
                   AND data_key = {}",
                backend.placeholder(1),
                backend.placeholder(2),
                backend.placeholder(3),
                backend.placeholder(4),
            ),
            scope_values(scope, backend, key)?,
        ))
        .await
        .map_err(storage_error)?;
    Ok(())
}

pub(crate) fn index_contract_digest(indexes: &[ArtifactDataIndexField]) -> Option<String> {
    (!indexes.is_empty()).then(|| {
        // Canonical (order-independent) contract identity: the same index
        // set declared in a different order is the same contract.
        let mut canonical = indexes.to_vec();
        canonical.sort_by(|left, right| {
            left.name
                .cmp(&right.name)
                .then_with(|| left.json_pointer.cmp(&right.json_pointer))
                .then_with(|| (left.value_type as u8).cmp(&(right.value_type as u8)))
        });
        let encoded = serde_json::to_vec(&canonical)
            .expect("artifact data index declarations are always serializable");
        crate::installation::sha256_digest(&encoded)
    })
}

pub(crate) fn artifact_data_index_value_matches(
    value: &Value,
    value_type: ArtifactDataIndexValueType,
) -> bool {
    matches!(
        (value, value_type),
        (Value::String(_), ArtifactDataIndexValueType::String)
            | (Value::Number(_), ArtifactDataIndexValueType::Number)
            | (Value::Bool(_), ArtifactDataIndexValueType::Boolean)
    )
}
