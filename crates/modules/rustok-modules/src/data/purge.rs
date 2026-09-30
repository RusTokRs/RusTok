//! Artifact data purge service and preview targets.

use rustok_events::DomainEvent;
use sea_orm::{
    ConnectionTrait, DatabaseConnection, DatabaseTransaction, DbBackend, Statement,
    TransactionTrait, Value as SqlValue,
};
use uuid::Uuid;

use super::*;
use super::constants::*;
use super::error::*;
use super::helpers::*;
use super::purge_targets::*;
use super::traits::*;
use super::types::*;

/// Owner service for irreversible namespace deletion. Its authorization port
/// keeps retention and installation lifecycle policy outside guest-controlled
/// capability calls while the data owner keeps mutation, audit and outbox facts
/// in one transaction.
#[derive(Clone)]
pub struct SeaOrmArtifactDataPurgeService<A> {
    db: DatabaseConnection,
    authorizer: A,
    infrastructure: ControlPlaneInfrastructure,
}

impl<A> SeaOrmArtifactDataPurgeService<A>
where
    A: ArtifactDataPurgeAuthorizer,
{
    pub fn new(db: DatabaseConnection, authorizer: A) -> Self {
        let infrastructure = ControlPlaneInfrastructure::for_database(db.clone());
        Self::with_infrastructure(db, authorizer, infrastructure)
    }

    pub fn with_infrastructure(
        db: DatabaseConnection,
        authorizer: A,
        infrastructure: ControlPlaneInfrastructure,
    ) -> Self {
        Self {
            db,
            authorizer,
            infrastructure,
        }
    }

    pub async fn purge(
        &self,
        request: ArtifactDataPurgeRequest,
    ) -> Result<ArtifactDataPurgeResult, ArtifactDataError> {
        validate_purge_request(&request)?;
        let tenant_id = request
            .context
            .tenant_id
            .ok_or(ArtifactDataError::PurgePrecondition)?;

        // An exact terminal receipt is replayable after a later lifecycle
        // transition. Check it before requiring that the historical target is
        // still purge-eligible.
        let transaction = self.db.begin().await.map_err(storage_error)?;
        configure_tenant_scope(&transaction, tenant_id).await?;
        let backend = transaction.get_database_backend();
        if let Some(existing) = find_artifact_data_purge_operation(
            &transaction,
            tenant_id,
            request.installation_id,
            &request,
        )
        .await?
        {
            transaction.commit().await.map_err(storage_error)?;
            return Ok(existing);
        }

        lock_artifact_data_installation_on(&transaction, tenant_id, request.installation_id)
            .await?;
        // Another command may have committed while this operation waited for
        // the lifecycle lock. Replay before reading its mutable target facts.
        if let Some(existing) = find_artifact_data_purge_operation(
            &transaction,
            tenant_id,
            request.installation_id,
            &request,
        )
        .await?
        {
            transaction.commit().await.map_err(storage_error)?;
            return Ok(existing);
        }
        let locked_target = query_artifact_data_purge_target(
            &transaction,
            tenant_id,
            request.installation_id,
            true,
        )
        .await?;
        ensure_artifact_data_purge_target_is_retired(&transaction, &locked_target).await?;
        let scope = &locked_target.authorization.scope;

        let namespace = transaction
            .query_one_raw(Statement::from_sql_and_values(
                backend,
                format!(
                    "SELECT namespace_revision, CASE WHEN purged_at IS NULL THEN 0 ELSE 1 END AS is_purged
                     FROM module_artifact_data_namespaces
                     WHERE tenant_id = {} AND data_owner_id = {} AND namespace_instance_id = {}{}",
                    placeholder(backend, 1),
                    placeholder(backend, 2),
                    placeholder(backend, 3),
                    namespace_lock_clause(backend),
                ),
                namespace_values(scope, backend)?,
            ))
            .await
            .map_err(storage_error)?
            .ok_or(ArtifactDataError::PurgePrecondition)?;
        let current_revision: i64 = namespace
            .try_get("", "namespace_revision")
            .map_err(storage_error)?;
        let already_purged = namespace
            .try_get::<i64>("", "is_purged")
            .map_err(storage_error)?
            != 0;
        if already_purged
            || u64::try_from(current_revision).ok() != Some(request.expected_namespace_revision)
        {
            return Err(ArtifactDataError::PurgePrecondition);
        }
        self.authorizer
            .authorize_purge_on(&transaction, &request, &locked_target.authorization)
            .await?;
        ensure_namespace_not_migration_held_on(
            &transaction,
            scope.tenant_id,
            scope.data_owner_id,
            scope.namespace_instance_id,
            None,
        )
        .await
        .map_err(|error| match error {
            ArtifactDataError::Storage(message) => ArtifactDataError::Storage(message),
            _ => ArtifactDataError::PurgePrecondition,
        })?;
        transaction
            .execute_raw(Statement::from_sql_and_values(
                backend,
                format!(
                    "DELETE FROM module_artifact_data_index_contracts
                     WHERE tenant_id = {} AND data_owner_id = {} AND namespace_instance_id = {}",
                    placeholder(backend, 1),
                    placeholder(backend, 2),
                    placeholder(backend, 3),
                ),
                namespace_values(scope, backend)?,
            ))
            .await
            .map_err(storage_error)?;
        transaction
            .execute_raw(Statement::from_sql_and_values(
                backend,
                format!(
                    "DELETE FROM module_artifact_data_indexes
                     WHERE tenant_id = {} AND data_owner_id = {} AND namespace_instance_id = {}",
                    placeholder(backend, 1),
                    placeholder(backend, 2),
                    placeholder(backend, 3),
                ),
                namespace_values(scope, backend)?,
            ))
            .await
            .map_err(storage_error)?;
        let structured_records = transaction
            .execute_raw(Statement::from_sql_and_values(
                backend,
                format!(
                    "DELETE FROM module_artifact_data
                     WHERE tenant_id = {} AND data_owner_id = {} AND namespace_instance_id = {}",
                    placeholder(backend, 1),
                    placeholder(backend, 2),
                    placeholder(backend, 3),
                ),
                namespace_values(scope, backend)?,
            ))
            .await
            .map_err(storage_error)?
            .rows_affected();
        transaction
            .execute_raw(Statement::from_sql_and_values(
                backend,
                format!(
                    "DELETE FROM module_artifact_data_operations
                     WHERE tenant_id = {} AND data_owner_id = {} AND namespace_instance_id = {}",
                    placeholder(backend, 1),
                    placeholder(backend, 2),
                    placeholder(backend, 3),
                ),
                namespace_values(scope, backend)?,
            ))
            .await
            .map_err(storage_error)?;
        transaction
            .execute_raw(Statement::from_sql_and_values(
                backend,
                format!(
                    "DELETE FROM module_artifact_data_delete_operations
                     WHERE tenant_id = {} AND data_owner_id = {} AND namespace_instance_id = {}",
                    placeholder(backend, 1),
                    placeholder(backend, 2),
                    placeholder(backend, 3),
                ),
                namespace_values(scope, backend)?,
            ))
            .await
            .map_err(storage_error)?;
        let object_storage_keys = transaction
            .query_all_raw(Statement::from_sql_and_values(
                backend,
                format!(
                    "SELECT storage_key FROM module_artifact_data_objects
                     WHERE tenant_id = {} AND data_owner_id = {} AND namespace_instance_id = {}",
                    placeholder(backend, 1),
                    placeholder(backend, 2),
                    placeholder(backend, 3),
                ),
                namespace_values(scope, backend)?,
            ))
            .await
            .map_err(storage_error)?;
        for row in object_storage_keys {
            let storage_key: String = row.try_get("", "storage_key").map_err(storage_error)?;
            queue_artifact_data_object_gc_candidate(
                &transaction,
                &self.infrastructure,
                scope,
                &storage_key,
            )
            .await?;
        }
        let object_records = transaction
            .execute_raw(Statement::from_sql_and_values(
                backend,
                format!(
                    "DELETE FROM module_artifact_data_objects
                     WHERE tenant_id = {} AND data_owner_id = {} AND namespace_instance_id = {}",
                    placeholder(backend, 1),
                    placeholder(backend, 2),
                    placeholder(backend, 3),
                ),
                namespace_values(scope, backend)?,
            ))
            .await
            .map_err(storage_error)?
            .rows_affected();
        transaction
            .execute_raw(Statement::from_sql_and_values(
                backend,
                format!(
                    "DELETE FROM module_artifact_data_object_operations
                     WHERE tenant_id = {} AND data_owner_id = {} AND namespace_instance_id = {}",
                    placeholder(backend, 1),
                    placeholder(backend, 2),
                    placeholder(backend, 3),
                ),
                namespace_values(scope, backend)?,
            ))
            .await
            .map_err(storage_error)?;
        transaction
            .execute_raw(Statement::from_sql_and_values(
                backend,
                format!(
                    "DELETE FROM module_artifact_data_object_delete_operations
                     WHERE tenant_id = {} AND data_owner_id = {} AND namespace_instance_id = {}",
                    placeholder(backend, 1),
                    placeholder(backend, 2),
                    placeholder(backend, 3),
                ),
                namespace_values(scope, backend)?,
            ))
            .await
            .map_err(storage_error)?;
        let next_revision = u64::try_from(current_revision)
            .ok()
            .and_then(|value| value.checked_add(1))
            .ok_or(ArtifactDataError::PurgePrecondition)?;
        let updated = transaction
            .execute_raw(Statement::from_sql_and_values(
                backend,
                format!(
                    "UPDATE module_artifact_data_namespaces
                     SET namespace_revision = {}, state = 'purged', purged_at = {}, updated_at = {}
                     WHERE tenant_id = {} AND data_owner_id = {} AND namespace_instance_id = {}
                     AND namespace_revision = {} AND purged_at IS NULL",
                    placeholder(backend, 1),
                    now_expression(backend),
                    now_expression(backend),
                    placeholder(backend, 2),
                    placeholder(backend, 3),
                    placeholder(backend, 4),
                    placeholder(backend, 5),
                ),
                vec![
                    revision_value(next_revision)?,
                    uuid_value(scope.tenant_id, backend),
                    uuid_value(scope.data_owner_id, backend),
                    uuid_value(scope.namespace_instance_id, backend),
                    revision_value(request.expected_namespace_revision)?,
                ],
            ))
            .await
            .map_err(storage_error)?;
        if updated.rows_affected() != 1 {
            return Err(ArtifactDataError::PurgePrecondition);
        }
        let purged_records = structured_records
            .checked_add(object_records)
            .and_then(|count| i64::try_from(count).ok())
            .ok_or(ArtifactDataError::PurgePrecondition)?;
        transaction
            .execute_raw(Statement::from_sql_and_values(
                backend,
                format!(
                    "INSERT INTO module_artifact_data_purge_operations
                     (tenant_id, installation_id, data_owner_id, namespace_instance_id, policy_revision, idempotency_key, expected_namespace_revision,
                      namespace_revision, actor_id, trace_id, correlation_id, reason, purged_records, completed_at)
                     VALUES ({}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {})",
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
                    now_expression(backend),
                ),
                vec![
                    uuid_value(scope.tenant_id, backend),
                    uuid_value(request.installation_id, backend),
                    uuid_value(scope.data_owner_id, backend),
                    uuid_value(scope.namespace_instance_id, backend),
                    revision_value(scope.policy_revision)?,
                    uuid_value(request.context.idempotency_key, backend),
                    revision_value(request.expected_namespace_revision)?,
                    revision_value(next_revision)?,
                    uuid_value(request.context.actor_id, backend),
                    request.context.trace_id.clone().into(),
                    uuid_value(request.context.correlation_id, backend),
                    request.reason.clone().into(),
                    purged_records.into(),
                ],
            ))
            .await
            .map_err(storage_error)?;
        self.infrastructure
            .write_event(
                &transaction,
                self.infrastructure.event_envelope_for_command(
                    &request.context,
                    DomainEvent::ModuleArtifactDataPurged {
                        tenant_id: scope.tenant_id,
                        module_slug: scope.module_slug.clone(),
                        data_contract_revision: scope.data_contract_revision,
                        namespace_revision: next_revision,
                        purged_records: u64::try_from(purged_records)
                            .map_err(|_| ArtifactDataError::PurgePrecondition)?,
                    },
                ),
            )
            .await
            .map_err(storage_error)?;
        transaction.commit().await.map_err(storage_error)?;
        Ok(ArtifactDataPurgeResult {
            namespace_revision: next_revision,
            purged_records: u64::try_from(purged_records)
                .map_err(|_| ArtifactDataError::PurgePrecondition)?,
        })
    }
}

/// Owner-owned read path for a data-purge preview. The preview intentionally
/// exposes only redacted readiness facts and cannot be used as apply input.
#[derive(Clone)]
pub struct ArtifactDataPurgePreviewService {
    db: DatabaseConnection,
}

impl ArtifactDataPurgePreviewService {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }

    pub async fn preview(
        &self,
        tenant_id: Uuid,
        installation_id: Uuid,
    ) -> Result<ArtifactDataPurgePreview, ArtifactDataError> {
        if tenant_id.is_nil() || installation_id.is_nil() {
            return Err(ArtifactDataError::PurgePrecondition);
        }
        let transaction = self.db.begin().await.map_err(storage_error)?;
        configure_tenant_scope(&transaction, tenant_id).await?;
        let target =
            load_artifact_data_purge_target(&transaction, tenant_id, installation_id, false)
                .await?;

        if !target.is_retired() {
            transaction.commit().await.map_err(storage_error)?;
            return Ok(ArtifactDataPurgePreview {
                installation_id,
                namespace_revision: 0,
                records_to_purge: 0,
                can_purge: false,
                reason: "Artifact installation must be inactive and uninstalled before data purge."
                    .to_string(),
            });
        }
        if let Err(error) =
            ensure_no_active_artifact_data_scope_collision(&transaction, &target).await
        {
            if matches!(error, ArtifactDataError::PurgePrecondition) {
                transaction.commit().await.map_err(storage_error)?;
                return Ok(ArtifactDataPurgePreview {
                    installation_id,
                    namespace_revision: 0,
                    records_to_purge: 0,
                    can_purge: false,
                    reason: "An active installation with the same artifact slug still exists in this tenant scope."
                        .to_string(),
                });
            }
            return Err(error);
        }

        let backend = transaction.get_database_backend();
        let scope = &target.authorization.scope;
        let namespace = transaction
            .query_one_raw(Statement::from_sql_and_values(
                backend,
                format!(
                    "SELECT namespace_revision, CASE WHEN purged_at IS NULL THEN 0 ELSE 1 END AS is_purged
                     FROM module_artifact_data_namespaces
                     WHERE tenant_id = {} AND data_owner_id = {} AND namespace_instance_id = {}",
                    placeholder(backend, 1),
                    placeholder(backend, 2),
                    placeholder(backend, 3),
                ),
                namespace_values(scope, backend)?,
            ))
            .await
            .map_err(storage_error)?;
        let Some(namespace) = namespace else {
            transaction.commit().await.map_err(storage_error)?;
            return Ok(ArtifactDataPurgePreview {
                installation_id,
                namespace_revision: 0,
                records_to_purge: 0,
                can_purge: false,
                reason: "No artifact data namespace exists for the retired installation."
                    .to_string(),
            });
        };
        let namespace_revision = u64::try_from(
            namespace
                .try_get::<i64>("", "namespace_revision")
                .map_err(storage_error)?,
        )
        .map_err(|_| ArtifactDataError::PurgePrecondition)?;
        let already_purged = namespace
            .try_get::<i64>("", "is_purged")
            .map_err(storage_error)?
            != 0;
        if already_purged {
            transaction.commit().await.map_err(storage_error)?;
            return Ok(ArtifactDataPurgePreview {
                installation_id,
                namespace_revision,
                records_to_purge: 0,
                can_purge: false,
                reason: "Artifact data namespace is already purged.".to_string(),
            });
        }
        let records_to_purge =
            count_artifact_data_purge_records(&transaction, scope, backend).await?;
        transaction.commit().await.map_err(storage_error)?;
        Ok(ArtifactDataPurgePreview {
            installation_id,
            namespace_revision,
            records_to_purge,
            can_purge: true,
            reason:
                "Retired artifact data namespace is eligible for a separately authorized purge."
                    .to_string(),
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ArtifactDataPurgeTarget {
    pub(crate) authorization: ArtifactDataPurgeAuthorizationContext,
    pub(crate) installation_scope: ModuleInstallationScope,
    pub(crate) admission_status: String,
    pub(crate) uninstalled: bool,
}

impl ArtifactDataPurgeTarget {
    fn is_retired(&self) -> bool {
        self.admission_status == "inactive" && self.uninstalled
    }
}

pub(crate) async fn find_artifact_data_purge_operation<C: ConnectionTrait>(
    connection: &C,
    tenant_id: Uuid,
    installation_id: Uuid,
    request: &ArtifactDataPurgeRequest,
) -> Result<Option<ArtifactDataPurgeResult>, ArtifactDataError> {
    let backend = connection.get_database_backend();
    let row = connection
        .query_one_raw(Statement::from_sql_and_values(
            backend,
            format!(
                "SELECT expected_namespace_revision, actor_id, trace_id, correlation_id, reason, namespace_revision, purged_records
                 FROM module_artifact_data_purge_operations
                 WHERE tenant_id = {} AND installation_id = {} AND idempotency_key = {}",
                placeholder(backend, 1),
                placeholder(backend, 2),
                placeholder(backend, 3),
            ),
            vec![
                uuid_value(tenant_id, backend),
                uuid_value(installation_id, backend),
                uuid_value(request.context.idempotency_key, backend),
            ],
        ))
        .await
        .map_err(storage_error)?;
    let Some(row) = row else {
        return Ok(None);
    };
    let expected_revision: i64 = row
        .try_get("", "expected_namespace_revision")
        .map_err(storage_error)?;
    let actor_id = uuid_from_row(&row, "actor_id", backend)?;
    let trace_id: String = row.try_get("", "trace_id").map_err(storage_error)?;
    let correlation_id = uuid_from_row(&row, "correlation_id", backend)?;
    let reason: String = row.try_get("", "reason").map_err(storage_error)?;
    if u64::try_from(expected_revision).ok() != Some(request.expected_namespace_revision)
        || actor_id != request.context.actor_id
        || trace_id != request.context.trace_id
        || correlation_id != request.context.correlation_id
        || reason != request.reason
    {
        return Err(ArtifactDataError::IdempotencyConflict);
    }
    let namespace_revision: i64 = row
        .try_get("", "namespace_revision")
        .map_err(storage_error)?;
    let purged_records: i64 = row.try_get("", "purged_records").map_err(storage_error)?;
    Ok(Some(ArtifactDataPurgeResult {
        namespace_revision: u64::try_from(namespace_revision)
            .ok()
            .filter(|revision| *revision > 0)
            .ok_or_else(|| {
                storage_error("Stored data purge receipt has an invalid namespace revision")
            })?,
        purged_records: u64::try_from(purged_records)
            .map_err(|_| storage_error("Stored data purge receipt has an invalid record count"))?,
    }))
}
