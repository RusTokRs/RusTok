//! SQL query builders, placeholders, tenant scoping, and namespace lock helpers.

use sea_orm::{
    ConnectionTrait, DatabaseConnection, DatabaseTransaction, DbBackend, QueryResult,
    Statement, TransactionTrait, Value as SqlValue,
};
use uuid::Uuid;

use super::*;
use super::constants::*;
use super::error::*;
use super::types::*;

pub(crate) async fn namespace_record_count<C: ConnectionTrait>(
    connection: &C,
    table: &'static str,
    scope: &ArtifactDataScope,
    backend: DbBackend,
) -> Result<u64, ArtifactDataError> {
    let row = connection
        .query_one_raw(Statement::from_sql_and_values(
            backend,
            format!(
                "SELECT COUNT(*) AS record_count FROM {table}
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
            ArtifactDataError::Storage("artifact data record count was unavailable".to_string())
        })?;
    let count: i64 = row.try_get("", "record_count").map_err(storage_error)?;
    u64::try_from(count).map_err(|_| ArtifactDataError::PurgePrecondition)
}

pub(crate) async fn configure_tenant_scope<C: ConnectionTrait>(
    connection: &C,
    tenant_id: Uuid,
) -> Result<(), ArtifactDataError> {
    if connection.get_database_backend() == DbBackend::Postgres {
        connection
            .execute_raw(Statement::from_sql_and_values(
                DbBackend::Postgres,
                "SELECT set_config('rustok.tenant_id', $1, true)",
                vec![tenant_id.to_string().into()],
            ))
            .await
            .map_err(storage_error)?;
    }
    Ok(())
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
    let backend = transaction.get_database_backend();
    let values = vec![
        uuid_value(scope.tenant_id, backend),
        uuid_value(scope.data_owner_id, backend),
        uuid_value(scope.namespace_instance_id, backend),
        scope.module_slug.clone().into(),
        revision_value(scope.data_contract_revision)?,
        scope.data_contract_digest.clone().into(),
        uuid_value(installation.installation_id, backend),
        revision_value(scope.policy_revision)?,
    ];
    transaction.execute_raw(Statement::from_sql_and_values(backend, format!(
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
        tenant=placeholder(backend,1), owner=placeholder(backend,2), instance=placeholder(backend,3),
        slug=placeholder(backend,4), revision=placeholder(backend,5), digest=placeholder(backend,6),
        installation=placeholder(backend,7), policy=placeholder(backend,8), now=now_expression(backend),
    ), values)).await.map_err(storage_error)?;
    transaction.execute_raw(Statement::from_sql_and_values(backend, format!(
        "INSERT INTO module_artifact_data_owner_references
         (tenant_id, data_owner_id, namespace_instance_id, reference_revision)
         SELECT tenant_id, data_owner_id, namespace_instance_id, 1 FROM module_artifact_data_namespaces
         WHERE tenant_id = {} AND data_owner_id = {} AND namespace_instance_id = {}
           AND state = 'serving' AND purged_at IS NULL
         ON CONFLICT DO NOTHING",
        placeholder(backend,1), placeholder(backend,2), placeholder(backend,3),
    ), namespace_values(&scope,backend)?)).await.map_err(storage_error)?;
    let row = transaction.query_one_raw(Statement::from_sql_and_values(backend, format!(
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
        placeholder(backend,3), placeholder(backend,1), placeholder(backend,2), placeholder(backend,4),
        namespace_lock_clause(backend),
    ), vec![uuid_value(scope.tenant_id,backend), uuid_value(scope.data_owner_id,backend),
            uuid_value(installation.installation_id,backend), revision_value(scope.policy_revision)?]))
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
    backend: DbBackend,
) -> Result<(), ArtifactDataError> {
    let active = connection
        .query_one_raw(Statement::from_sql_and_values(
            backend,
            format!(
                "SELECT namespace_revision FROM module_artifact_data_namespaces
                 WHERE tenant_id = {} AND data_owner_id = {} AND namespace_instance_id = {}
                 AND state = 'serving' AND purged_at IS NULL
                 AND EXISTS (SELECT 1 FROM module_artifact_data_owner_references reference
                             WHERE reference.tenant_id = {} AND reference.data_owner_id = {}
                               AND reference.namespace_instance_id = {}){}",
                placeholder(backend, 1),
                placeholder(backend, 2),
                placeholder(backend, 3),
                placeholder(backend, 1),
                placeholder(backend, 2),
                placeholder(backend, 3),
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
    backend: DbBackend,
) -> Result<u64, ArtifactDataError> {
    let row = connection
        .query_one_raw(Statement::from_sql_and_values(
            backend,
            format!(
                "SELECT namespace_revision FROM module_artifact_data_namespaces
                 WHERE tenant_id = {} AND data_owner_id = {} AND namespace_instance_id = {}
                 AND purged_at IS NULL{}",
                placeholder(backend, 1),
                placeholder(backend, 2),
                placeholder(backend, 3),
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
    backend: DbBackend,
    contract_digest: &str,
    bind_if_empty: bool,
) -> Result<(), ArtifactDataError> {
    let values = namespace_values(scope, backend)?;
    let existing = connection
        .query_one_raw(Statement::from_sql_and_values(
            backend,
            format!(
                "SELECT contract_digest FROM module_artifact_data_index_contracts
                 WHERE tenant_id = {} AND data_owner_id = {} AND namespace_instance_id = {}",
                placeholder(backend, 1),
                placeholder(backend, 2),
                placeholder(backend, 3),
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
            backend,
            format!(
                "SELECT 1 FROM module_artifact_data
                 WHERE tenant_id = {} AND data_owner_id = {} AND namespace_instance_id = {}
                 LIMIT 1",
                placeholder(backend, 1),
                placeholder(backend, 2),
                placeholder(backend, 3),
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
            backend,
            format!(
                "INSERT INTO module_artifact_data_index_contracts
                 (tenant_id, data_owner_id, namespace_instance_id, contract_digest, bound_at)
                 VALUES ({}, {}, {}, {}, {}) ON CONFLICT DO NOTHING",
                placeholder(backend, 1),
                placeholder(backend, 2),
                placeholder(backend, 3),
                placeholder(backend, 4),
                now_expression(backend),
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
            backend,
            format!(
                "SELECT contract_digest FROM module_artifact_data_index_contracts
                 WHERE tenant_id = {} AND data_owner_id = {} AND namespace_instance_id = {}",
                placeholder(backend, 1),
                placeholder(backend, 2),
                placeholder(backend, 3),
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

pub(crate) fn validate_purge_request(request: &ArtifactDataPurgeRequest) -> Result<(), ArtifactDataError> {
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

pub(crate) fn validate_export_request(request: &ArtifactDataExportRequest) -> Result<(), ArtifactDataError> {
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
    backend: DbBackend,
    key: &str,
) -> Result<Vec<SqlValue>, ArtifactDataError> {
    Ok(vec![
        uuid_value(scope.tenant_id, backend),
        uuid_value(scope.data_owner_id, backend),
        uuid_value(scope.namespace_instance_id, backend),
        key.to_owned().into(),
    ])
}

pub(crate) fn namespace_values(
    scope: &ArtifactDataScope,
    backend: DbBackend,
) -> Result<Vec<SqlValue>, ArtifactDataError> {
    Ok(vec![
        uuid_value(scope.tenant_id, backend),
        uuid_value(scope.data_owner_id, backend),
        uuid_value(scope.namespace_instance_id, backend),
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

pub(crate) fn uuid_value(value: Uuid, backend: DbBackend) -> SqlValue {
    match backend {
        DbBackend::Postgres => SqlValue::Uuid(Some(value)),
        _ => value.to_string().into(),
    }
}

pub(crate) fn uuid_from_row(
    row: &sea_orm::QueryResult,
    column: &str,
    backend: DbBackend,
) -> Result<Uuid, ArtifactDataError> {
    match backend {
        DbBackend::Postgres => row.try_get("", column).map_err(storage_error),
        _ => match row.try_get::<Uuid>("", column) {
            Ok(value) => Ok(value),
            Err(_) => row
                .try_get::<String>("", column)
                .map_err(storage_error)?
                .parse()
                .map_err(storage_error),
        },
    }
}

pub(crate) fn placeholder(backend: DbBackend, index: usize) -> String {
    match backend {
        DbBackend::Postgres => format!("${index}"),
        _ => format!("?{index}"),
    }
}

pub(crate) fn now_expression(backend: DbBackend) -> &'static str {
    match backend {
        DbBackend::Postgres => "NOW()",
        _ => "datetime('now')",
    }
}

pub(crate) fn namespace_lock_clause(backend: DbBackend) -> &'static str {
    match backend {
        DbBackend::Postgres => " FOR UPDATE",
        _ => "",
    }
}

pub(crate) fn record_from_row(row: sea_orm::QueryResult) -> Result<ArtifactDataRecord, ArtifactDataError> {
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
