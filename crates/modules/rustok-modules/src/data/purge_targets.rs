//! Artifact data purge target resolution, candidate counts, and collision checks.

use sea_orm::{
    ConnectionTrait, DatabaseConnection, DatabaseTransaction, DbBackend, QueryResult,
    Statement, TransactionTrait, Value as SqlValue,
};
use uuid::Uuid;

use super::*;
use super::constants::*;
use super::error::*;
use super::helpers::*;
use super::traits::*;
use super::types::*;

pub(crate) async fn load_artifact_data_purge_target<C: ConnectionTrait>(
    connection: &C,
    tenant_id: Uuid,
    installation_id: Uuid,
    lock: bool,
) -> Result<ArtifactDataPurgeTarget, ArtifactDataError> {
    if lock {
        lock_artifact_data_installation_on(connection, tenant_id, installation_id).await?;
    }
    query_artifact_data_purge_target(connection, tenant_id, installation_id, lock).await
}

pub(crate) fn data_installation_scope_from_row(
    row: &sea_orm::QueryResult,
    backend: DbBackend,
) -> Result<ModuleInstallationScope, ArtifactDataError> {
    match row
        .try_get::<String>("", "scope_kind")
        .map_err(storage_error)?
        .as_str()
    {
        "platform" => Ok(ModuleInstallationScope::Platform),
        "tenant" => Ok(ModuleInstallationScope::Tenant {
            tenant_id: uuid_from_row(row, "tenant_id", backend)?,
        }),
        _ => Err(ArtifactDataError::PurgePrecondition),
    }
}

/// Lock immutable installation identity before deriving mutable namespace facts.
pub(crate) async fn lock_artifact_data_installation_on<C: ConnectionTrait>(
    connection: &C,
    tenant_id: Uuid,
    installation_id: Uuid,
) -> Result<(), ArtifactDataError> {
    let backend = connection.get_database_backend();
    let row = connection
        .query_one_raw(Statement::from_sql_and_values(
            backend,
            format!(
                "SELECT scope_kind, tenant_id, slug FROM module_artifact_installations
             WHERE installation_id = {}
               AND ((scope_kind = 'platform' AND tenant_id IS NULL)
                    OR (scope_kind = 'tenant' AND tenant_id = {}))",
                placeholder(backend, 1),
                placeholder(backend, 2),
            ),
            vec![
                uuid_value(installation_id, backend),
                uuid_value(tenant_id, backend),
            ],
        ))
        .await
        .map_err(storage_error)?
        .ok_or(ArtifactDataError::PurgePrecondition)?;
    let scope = data_installation_scope_from_row(&row, backend)?;
    let slug: String = row.try_get("", "slug").map_err(storage_error)?;
    crate::installation::acquire_artifact_activation_lock(connection, &scope, &slug)
        .await
        .map_err(|error| ArtifactDataError::Storage(error.to_string()))
}

pub(crate) async fn query_artifact_data_purge_target<C: ConnectionTrait>(
    connection: &C,
    tenant_id: Uuid,
    installation_id: Uuid,
    lock: bool,
) -> Result<ArtifactDataPurgeTarget, ArtifactDataError> {
    let backend = connection.get_database_backend();
    let row = connection
        .query_one_raw(Statement::from_sql_and_values(
            backend,
            format!(
                "SELECT installation.scope_kind, installation.tenant_id, installation.slug, installation.data_owner_id, \
                        reference.namespace_instance_id, namespace.data_contract_digest AS stored_contract_digest, \
                        installation.capability_grant_revision, admission.revision AS installation_revision, admission.status, \
                        CAST(installation.descriptor AS TEXT) AS descriptor, \
                        EXISTS (SELECT 1 FROM module_artifact_uninstall_operations uninstall \
                                WHERE uninstall.installation_id = installation.installation_id) AS uninstalled \
                 FROM module_artifact_installations installation \
                 JOIN module_artifact_admissions admission \
                   ON admission.installation_id = installation.installation_id \
                 JOIN module_artifact_data_owner_references reference \
                   ON reference.tenant_id = {} AND reference.data_owner_id = installation.data_owner_id \
                 JOIN module_artifact_data_namespaces namespace \
                   ON namespace.tenant_id = reference.tenant_id AND namespace.data_owner_id = reference.data_owner_id \
                  AND namespace.namespace_instance_id = reference.namespace_instance_id \
                 WHERE installation.installation_id = {} \
                   AND ((installation.scope_kind = 'platform' AND installation.tenant_id IS NULL) \
                        OR (installation.scope_kind = 'tenant' AND installation.tenant_id = {})){}",
                placeholder(backend, 2),
                placeholder(backend, 1),
                placeholder(backend, 2),
                if lock { namespace_lock_clause(backend) } else { "" },
            ),
            vec![
                uuid_value(installation_id, backend),
                uuid_value(tenant_id, backend),
            ],
        ))
        .await
        .map_err(storage_error)?
        .ok_or(ArtifactDataError::PurgePrecondition)?;
    let installation_scope = data_installation_scope_from_row(&row, backend)?;
    let descriptor: crate::ModuleArtifactDescriptor = serde_json::from_str(
        &row.try_get::<String>("", "descriptor")
            .map_err(storage_error)?,
    )
    .map_err(|_| ArtifactDataError::DataContractUnavailable)?;
    descriptor
        .validate()
        .map_err(|_| ArtifactDataError::DataContractUnavailable)?;
    let contract = descriptor
        .persistence_contract
        .as_ref()
        .ok_or(ArtifactDataError::DataContractUnavailable)?;
    let capability_grant_revision = u64::try_from(
        row.try_get::<i64>("", "capability_grant_revision")
            .map_err(storage_error)?,
    )
    .ok()
    .filter(|revision| *revision > 0)
    .ok_or(ArtifactDataError::PurgePrecondition)?;
    let data_contract_digest = crate::promotion::digest_json(contract)
        .map_err(|error| ArtifactDataError::Storage(error.to_string()))?;
    if row
        .try_get::<String>("", "stored_contract_digest")
        .map_err(storage_error)?
        != data_contract_digest
    {
        return Err(ArtifactDataError::DataContractUnavailable);
    }
    let scope = ArtifactDataScope {
        tenant_id,
        data_owner_id: uuid_from_row(&row, "data_owner_id", backend)?,
        namespace_instance_id: uuid_from_row(&row, "namespace_instance_id", backend)?,
        data_contract_digest,
        module_slug: row.try_get("", "slug").map_err(storage_error)?,
        data_contract_revision: contract.revision,
        policy_revision: capability_grant_revision,
    };
    scope.validate()?;
    let installation_revision = u64::try_from(
        row.try_get::<i64>("", "installation_revision")
            .map_err(storage_error)?,
    )
    .ok()
    .filter(|revision| *revision > 0)
    .ok_or(ArtifactDataError::PurgePrecondition)?;
    let uninstalled = match backend {
        DbBackend::Postgres => row.try_get("", "uninstalled").map_err(storage_error)?,
        _ => {
            row.try_get::<i64>("", "uninstalled")
                .map_err(storage_error)?
                != 0
        }
    };
    Ok(ArtifactDataPurgeTarget {
        authorization: ArtifactDataPurgeAuthorizationContext {
            installation_id,
            data_owner_id: uuid_from_row(&row, "data_owner_id", backend)?,
            installation_revision,
            scope,
        },
        installation_scope,
        admission_status: row.try_get("", "status").map_err(storage_error)?,
        uninstalled,
    })
}

pub(crate) async fn ensure_artifact_data_purge_target_is_retired<C: ConnectionTrait>(
    connection: &C,
    target: &ArtifactDataPurgeTarget,
) -> Result<(), ArtifactDataError> {
    if !target.is_retired() {
        return Err(ArtifactDataError::PurgePrecondition);
    }
    ensure_no_active_artifact_data_scope_collision(connection, target).await
}

pub(crate) async fn load_retired_artifact_data_authorization_on<C: ConnectionTrait>(
    connection: &C,
    tenant_id: Uuid,
    installation_id: Uuid,
) -> Result<ArtifactDataPurgeAuthorizationContext, ArtifactDataError> {
    let target =
        load_artifact_data_purge_target(connection, tenant_id, installation_id, true).await?;
    ensure_artifact_data_purge_target_is_retired(connection, &target).await?;
    Ok(target.authorization)
}

/// Serving installations collide only with this exact tenant owner instance.
/// An unrelated installation with the same display slug is a separate boundary.
pub(crate) async fn ensure_no_active_artifact_data_scope_collision<C: ConnectionTrait>(
    connection: &C,
    target: &ArtifactDataPurgeTarget,
) -> Result<(), ArtifactDataError> {
    let backend = connection.get_database_backend();
    let scope = &target.authorization.scope;
    let active = connection
        .query_one_raw(Statement::from_sql_and_values(
            backend,
            format!(
                "SELECT 1
                 FROM module_artifact_installations installation
                 JOIN module_artifact_admissions admission
                   ON admission.installation_id = installation.installation_id
                 JOIN module_artifact_data_owner_references reference
                   ON reference.data_owner_id = installation.data_owner_id AND reference.tenant_id = {}
                 WHERE installation.data_owner_id = {} AND reference.namespace_instance_id = {}
                   AND admission.status = 'active'
                   AND ((installation.scope_kind = 'platform' AND installation.tenant_id IS NULL)
                        OR (installation.scope_kind = 'tenant' AND installation.tenant_id = {}))
                 LIMIT 1",
                placeholder(backend, 1),
                placeholder(backend, 2),
                placeholder(backend, 3),
                placeholder(backend, 1),
            ),
            vec![
                uuid_value(scope.tenant_id, backend),
                uuid_value(scope.data_owner_id, backend),
                uuid_value(scope.namespace_instance_id, backend),
            ],
        ))
        .await
        .map_err(storage_error)?;
    active
        .is_none()
        .then_some(())
        .ok_or(ArtifactDataError::PurgePrecondition)
}

pub(crate) async fn count_artifact_data_purge_records<C: ConnectionTrait>(
    connection: &C,
    scope: &ArtifactDataScope,
    backend: DbBackend,
) -> Result<u64, ArtifactDataError> {
    let structured =
        namespace_record_count(connection, "module_artifact_data", scope, backend).await?;
    let objects =
        namespace_record_count(connection, "module_artifact_data_objects", scope, backend).await?;
    structured
        .checked_add(objects)
        .ok_or(ArtifactDataError::PurgePrecondition)
}
