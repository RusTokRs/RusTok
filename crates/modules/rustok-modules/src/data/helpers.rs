//! SQL query builders, placeholders, tenant scoping, and namespace lock helpers.

use sea_orm::{
    ConnectionTrait, DatabaseConnection, DatabaseTransaction, QueryResult, Statement,
    TransactionTrait, Value as SqlValue,
};
use uuid::Uuid;

use super::constants::*;
use super::error::*;
use super::types::*;
use super::*;

pub(crate) async fn namespace_record_count<C: ConnectionTrait>(
    connection: &C,
    table: &'static str,
    scope: &ArtifactDataScope,
    backend: SupportedBackend,
) -> Result<u64, ArtifactDataError> {
    let row = connection
        .query_one_raw(Statement::from_sql_and_values(
            backend.db_backend(),
            format!(
                "SELECT COUNT(*) AS record_count FROM {table}
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
            ArtifactDataError::Storage("artifact data record count was unavailable".to_string())
        })?;
    let count: i64 = row.try_get("", "record_count").map_err(storage_error)?;
    u64::try_from(count).map_err(|_| ArtifactDataError::PurgePrecondition)
}

pub(crate) async fn configure_tenant_scope<C: ConnectionTrait>(
    connection: &C,
    tenant_id: Uuid,
) -> Result<(), ArtifactDataError> {
    // The GUC itself is owned by `crate::backend`; this adapter only maps
    // the error into the artifact-data taxonomy.
    crate::backend::set_tenant_scope(connection, tenant_id)
        .await
        .map_err(storage_error)
}

/// Resolve the tenant's single owner reference, initializing its first empty
/// instance only from the exact admitted serving installation allocation.
/// A retained tombstone or another contract never becomes an initial instance.
pub(crate) async fn resolve_serving_artifact_data_scope(
    db: &DatabaseConnection,
    installation: &InstalledModuleArtifact,
    mut scope: ArtifactDataScope,
) -> Result<ArtifactDataScope, ArtifactDataError> {
    scope.validate()?;
    let transaction = db.begin().await.map_err(storage_error)?;
    configure_tenant_scope(&transaction, scope.tenant_id).await?;
    crate::installation::acquire_artifact_activation_lock(
        &transaction,
        &installation.scope,
        &installation.descriptor.slug,
    )
    .await
    .map_err(|error| ArtifactDataError::Storage(error.to_string()))?;
    let backend =
        SupportedBackend::new(transaction.get_database_backend()).map_err(storage_error)?;
    let values = vec![
        backend.uuid_value(scope.tenant_id),
        backend.uuid_value(scope.data_owner_id),
        backend.uuid_value(scope.namespace_instance_id),
        scope.module_slug.clone().into(),
        revision_value(scope.data_contract_revision)?,
        scope.data_contract_digest.clone().into(),
        backend.uuid_value(installation.installation_id),
        revision_value(scope.policy_revision)?,
    ];
    transaction.execute_raw(Statement::from_sql_and_values(backend.db_backend(), format!(
        "INSERT INTO module_artifact_data_namespaces
         (tenant_id, data_owner_id, namespace_instance_id, module_slug, data_contract_revision,
          data_contract_digest, state, namespace_revision, created_at, updated_at)
         SELECT {tenant}, {owner}, {instance}, {slug}, {revision}, {digest}, 'serving', 1, {now}, {now}
         FROM module_artifact_installations installation
         JOIN module_artifact_admissions admission ON admission.installation_id = installation.installation_id
         WHERE installation.installation_id = {installation} AND installation.data_owner_id = {owner}
           AND installation.namespace_instance_id = {instance} AND installation.capability_grant_revision = {policy}
           AND admission.status = 'active'
           AND (installation.scope_kind = 'platform' OR installation.tenant_id = {tenant})
           AND NOT EXISTS (SELECT 1 FROM module_artifact_uninstall_operations uninstall
                           WHERE uninstall.installation_id = installation.installation_id)
           AND NOT EXISTS (SELECT 1 FROM module_artifact_tenant_lifecycle lifecycle
                           WHERE lifecycle.installation_id = installation.installation_id
                             AND lifecycle.tenant_id = {tenant} AND NOT lifecycle.enabled)
           AND NOT EXISTS (SELECT 1 FROM module_artifact_data_owner_references reference
                           WHERE reference.tenant_id = {tenant} AND reference.data_owner_id = {owner})
         ON CONFLICT DO NOTHING",
        tenant=backend.placeholder(1), owner=backend.placeholder(2), instance=backend.placeholder(3),
        slug=backend.placeholder(4), revision=backend.placeholder(5), digest=backend.placeholder(6),
        installation=backend.placeholder(7), policy=backend.placeholder(8), now=backend.now_sql(),
    ), values)).await.map_err(storage_error)?;
    transaction.execute_raw(Statement::from_sql_and_values(backend.db_backend(), format!(
        "INSERT INTO module_artifact_data_owner_references
         (tenant_id, data_owner_id, namespace_instance_id, reference_revision)
         SELECT tenant_id, data_owner_id, namespace_instance_id, 1 FROM module_artifact_data_namespaces
         WHERE tenant_id = {} AND data_owner_id = {} AND namespace_instance_id = {}
           AND state = 'serving' AND purged_at IS NULL
         ON CONFLICT DO NOTHING",
        backend.placeholder(1), backend.placeholder(2), backend.placeholder(3),
    ), namespace_values(&scope,backend)?)).await.map_err(storage_error)?;
    let row = transaction.query_one_raw(Statement::from_sql_and_values(backend.db_backend(), format!(
        "SELECT namespace.namespace_instance_id, namespace.data_contract_digest
         FROM module_artifact_data_owner_references reference
         JOIN module_artifact_data_namespaces namespace
           ON namespace.tenant_id = reference.tenant_id AND namespace.data_owner_id = reference.data_owner_id
          AND namespace.namespace_instance_id = reference.namespace_instance_id
         JOIN module_artifact_installations installation ON installation.installation_id = {}
           AND installation.data_owner_id = reference.data_owner_id
         JOIN module_artifact_admissions admission ON admission.installation_id = installation.installation_id
         WHERE reference.tenant_id = {} AND reference.data_owner_id = {}
           AND namespace.state = 'serving' AND namespace.purged_at IS NULL AND admission.status = 'active'
           AND installation.capability_grant_revision = {}{}",
        backend.placeholder(3), backend.placeholder(1), backend.placeholder(2), backend.placeholder(4),
        namespace_lock_clause(backend),
    ), vec![backend.uuid_value(scope.tenant_id), backend.uuid_value(scope.data_owner_id),
            backend.uuid_value(installation.installation_id), revision_value(scope.policy_revision)?]))
        .await.map_err(storage_error)?.ok_or(ArtifactDataError::PolicyDenied)?;
    let digest: String = row
        .try_get("", "data_contract_digest")
        .map_err(storage_error)?;
    if digest != scope.data_contract_digest {
        return Err(ArtifactDataError::DataContractUnavailable);
    }
    scope.namespace_instance_id = uuid_from_row(&row, "namespace_instance_id", backend)?;
    transaction.commit().await.map_err(storage_error)?;
    Ok(scope)
}

pub(crate) async fn ensure_active_namespace<C: ConnectionTrait>(
    connection: &C,
    scope: &ArtifactDataScope,
    backend: SupportedBackend,
) -> Result<(), ArtifactDataError> {
    let active = connection
        .query_one_raw(Statement::from_sql_and_values(
            backend.db_backend(),
            format!(
                "SELECT namespace_revision FROM module_artifact_data_namespaces
                 WHERE tenant_id = {} AND data_owner_id = {} AND namespace_instance_id = {}
                 AND state = 'serving' AND purged_at IS NULL
                 AND EXISTS (SELECT 1 FROM module_artifact_data_owner_references reference
                             WHERE reference.tenant_id = {} AND reference.data_owner_id = {}
                               AND reference.namespace_instance_id = {}){}",
                backend.placeholder(1),
                backend.placeholder(2),
                backend.placeholder(3),
                backend.placeholder(1),
                backend.placeholder(2),
                backend.placeholder(3),
                namespace_lock_clause(backend),
            ),
            namespace_values(scope, backend)?,
        ))
        .await
        .map_err(storage_error)?;
    if active.is_none() {
        return Err(ArtifactDataError::NamespacePurged);
    }
    Ok(())
}

/// Loads and locks an existing active namespace without creating one. Owner
/// exports must never resurrect a purged namespace or create state merely by
/// reading it.
pub(crate) async fn require_active_namespace<C: ConnectionTrait>(
    connection: &C,
    scope: &ArtifactDataScope,
    expected_namespace_revision: u64,
    backend: SupportedBackend,
) -> Result<u64, ArtifactDataError> {
    let row = connection
        .query_one_raw(Statement::from_sql_and_values(
            backend.db_backend(),
            format!(
                "SELECT namespace_revision FROM module_artifact_data_namespaces
                 WHERE tenant_id = {} AND data_owner_id = {} AND namespace_instance_id = {}
                 AND purged_at IS NULL{}",
                backend.placeholder(1),
                backend.placeholder(2),
                backend.placeholder(3),
                namespace_lock_clause(backend),
            ),
            namespace_values(scope, backend)?,
        ))
        .await
        .map_err(storage_error)?
        .ok_or(ArtifactDataError::ExportPrecondition)?;
    let namespace_revision: i64 = row
        .try_get("", "namespace_revision")
        .map_err(storage_error)?;
    let namespace_revision =
        u64::try_from(namespace_revision).map_err(|_| ArtifactDataError::ExportPrecondition)?;
    if namespace_revision != expected_namespace_revision {
        return Err(ArtifactDataError::ExportPrecondition);
    }
    Ok(namespace_revision)
}

/// Validates the exact immutable index declaration for a namespace. The first
/// indexed write binds the declaration before it persists data. An unbound
/// namespace with structured values but no binding is intentionally unavailable
/// for indexed queries: returning a partial result would be less safe than
/// requiring an owner-mediated data-contract upgrade.
pub(crate) async fn validate_artifact_data_index_contract<C: ConnectionTrait>(
    connection: &C,
    scope: &ArtifactDataScope,
    backend: SupportedBackend,
    contract_digest: &str,
    bind_if_empty: bool,
) -> Result<(), ArtifactDataError> {
    let values = namespace_values(scope, backend)?;
    let existing = connection
        .query_one_raw(Statement::from_sql_and_values(
            backend.db_backend(),
            format!(
                "SELECT contract_digest FROM module_artifact_data_index_contracts
                 WHERE tenant_id = {} AND data_owner_id = {} AND namespace_instance_id = {}",
                backend.placeholder(1),
                backend.placeholder(2),
                backend.placeholder(3),
            ),
            values.clone(),
        ))
        .await
        .map_err(storage_error)?;
    if let Some(row) = existing {
        let stored: String = row.try_get("", "contract_digest").map_err(storage_error)?;
        return (stored == contract_digest)
            .then_some(())
            .ok_or(ArtifactDataError::IndexQueryUnavailable);
    }
    let has_records = connection
        .query_one_raw(Statement::from_sql_and_values(
            backend.db_backend(),
            format!(
                "SELECT 1 FROM module_artifact_data
                 WHERE tenant_id = {} AND data_owner_id = {} AND namespace_instance_id = {}
                 LIMIT 1",
                backend.placeholder(1),
                backend.placeholder(2),
                backend.placeholder(3),
            ),
            values.clone(),
        ))
        .await
        .map_err(storage_error)?;
    if has_records.is_some() {
        return Err(ArtifactDataError::IndexQueryUnavailable);
    }
    if !bind_if_empty {
        return Ok(());
    }
    connection
        .execute_raw(Statement::from_sql_and_values(
            backend.db_backend(),
            format!(
                "INSERT INTO module_artifact_data_index_contracts
                 (tenant_id, data_owner_id, namespace_instance_id, contract_digest, bound_at)
                 VALUES ({}, {}, {}, {}, {}) ON CONFLICT DO NOTHING",
                backend.placeholder(1),
                backend.placeholder(2),
                backend.placeholder(3),
                backend.placeholder(4),
                backend.now_sql(),
            ),
            vec![
                values[0].clone(),
                values[1].clone(),
                values[2].clone(),
                contract_digest.to_owned().into(),
            ],
        ))
        .await
        .map_err(storage_error)?;
    let row = connection
        .query_one_raw(Statement::from_sql_and_values(
            backend.db_backend(),
            format!(
                "SELECT contract_digest FROM module_artifact_data_index_contracts
                 WHERE tenant_id = {} AND data_owner_id = {} AND namespace_instance_id = {}",
                backend.placeholder(1),
                backend.placeholder(2),
                backend.placeholder(3),
            ),
            values,
        ))
        .await
        .map_err(storage_error)?
        .ok_or_else(|| {
            ArtifactDataError::Storage("index contract binding was not persisted".to_string())
        })?;
    let stored: String = row.try_get("", "contract_digest").map_err(storage_error)?;
    (stored == contract_digest)
        .then_some(())
        .ok_or(ArtifactDataError::IndexQueryUnavailable)
}

pub(crate) fn validate_purge_request(
    request: &ArtifactDataPurgeRequest,
) -> Result<(), ArtifactDataError> {
    if request.installation_id.is_nil()
        || request.expected_namespace_revision == 0
        || request.context.tenant_id.is_none()
        || request.context.validate().is_err()
        || request.reason.trim().is_empty()
        || request.reason.trim() != request.reason
        || request.reason.len() > 2_000
    {
        return Err(ArtifactDataError::PurgePrecondition);
    }
    Ok(())
}

pub(crate) fn validate_export_request(
    request: &ArtifactDataExportRequest,
) -> Result<(), ArtifactDataError> {
    request.scope.validate()?;
    validate_page_request(&request.page)?;
    if request.expected_namespace_revision == 0
        || request.context.tenant_id != Some(request.scope.tenant_id)
        || request.context.validate().is_err()
        || request.reason.trim().is_empty()
        || request.reason.trim() != request.reason
        || request.reason.len() > 2_000
    {
        return Err(ArtifactDataError::ExportPrecondition);
    }
    Ok(())
}

pub(crate) fn scope_values(
    scope: &ArtifactDataScope,
    backend: SupportedBackend,
    key: &str,
) -> Result<Vec<SqlValue>, ArtifactDataError> {
    Ok(vec![
        backend.uuid_value(scope.tenant_id),
        backend.uuid_value(scope.data_owner_id),
        backend.uuid_value(scope.namespace_instance_id),
        key.to_owned().into(),
    ])
}

pub(crate) fn namespace_values(
    scope: &ArtifactDataScope,
    backend: SupportedBackend,
) -> Result<Vec<SqlValue>, ArtifactDataError> {
    Ok(vec![
        backend.uuid_value(scope.tenant_id),
        backend.uuid_value(scope.data_owner_id),
        backend.uuid_value(scope.namespace_instance_id),
    ])
}

pub(crate) fn revision_value(value: u64) -> Result<SqlValue, ArtifactDataError> {
    i64::try_from(value)
        .map(|value| value.into())
        .map_err(|_| ArtifactDataError::RevisionConflict)
}

pub(crate) fn optional_revision_value(value: Option<u64>) -> Result<SqlValue, ArtifactDataError> {
    value
        .map(revision_value)
        .transpose()
        .map(|value| match value {
            Some(value) => value,
            None => SqlValue::BigInt(None),
        })
}

pub(crate) fn uuid_from_row(
    row: &sea_orm::QueryResult,
    column: &str,
    backend: SupportedBackend,
) -> Result<Uuid, ArtifactDataError> {
    match backend {
        SupportedBackend::Postgres => row.try_get("", column).map_err(storage_error),
        SupportedBackend::Sqlite => match row.try_get::<Uuid>("", column) {
            Ok(value) => Ok(value),
            Err(_) => row
                .try_get::<String>("", column)
                .map_err(storage_error)?
                .parse()
                .map_err(storage_error),
        },
    }
}

/// SQL expression of the delivery queues' retry delay for a row's current
/// `attempt`: exponential backoff
/// `min(retry_max_seconds, retry_base_seconds * 2^min(attempt - 1, 16))`
/// seconds from now, evaluated per row. Lease expiry shares this backoff
/// with ordinary retryable failures: resetting `available_at` to now made
/// a crash-looping delivery hot-retry with zero delay until `max_attempts`.
pub(crate) fn retry_available_at_expression(
    backend: SupportedBackend,
    base_parameter: usize,
    max_parameter: usize,
) -> String {
    match backend {
        SupportedBackend::Postgres => format!(
            "NOW() + (LEAST({}, {} * (1 << LEAST(GREATEST(attempt - 1, 0), 16))) * INTERVAL '1 second')",
            backend.placeholder(max_parameter),
            backend.placeholder(base_parameter),
        ),
        SupportedBackend::Sqlite => format!(
            "datetime('now', '+' || MIN({}, {} * (1 << MIN(MAX(attempt - 1, 0), 16))) || ' seconds')",
            backend.placeholder(max_parameter),
            backend.placeholder(base_parameter),
        ),
    }
}

pub(crate) fn namespace_lock_clause(backend: SupportedBackend) -> &'static str {
    match backend {
        SupportedBackend::Postgres => " FOR UPDATE",
        SupportedBackend::Sqlite => "",
    }
}

pub(crate) fn record_from_row(
    row: sea_orm::QueryResult,
) -> Result<ArtifactDataRecord, ArtifactDataError> {
    let revision: i64 = row.try_get("", "revision").map_err(storage_error)?;
    Ok(ArtifactDataRecord {
        key: row.try_get("", "data_key").map_err(storage_error)?,
        value: row.try_get("", "value").map_err(storage_error)?,
        revision: u64::try_from(revision).map_err(|_| ArtifactDataError::RevisionConflict)?,
    })
}

pub(crate) fn nonnegative_usage(value: i64) -> Result<u64, ArtifactDataError> {
    u64::try_from(value)
        .map_err(|_| ArtifactDataError::Storage("artifact data usage is invalid".to_string()))
}

pub(crate) fn enforce_quota(
    resource: &'static str,
    limit: u64,
    attempted: u64,
) -> Result<(), ArtifactDataError> {
    if attempted > limit {
        return Err(ArtifactDataError::QuotaExceeded {
            resource,
            limit,
            attempted,
        });
    }
    Ok(())
}

pub(crate) fn storage_error(error: impl std::fmt::Display) -> ArtifactDataError {
    ArtifactDataError::Storage(error.to_string())
}
