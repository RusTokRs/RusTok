//! RBAC-owned grants and checks for immutable artifact permission vocabulary.

use std::sync::Arc;

use async_trait::async_trait;
use rustok_events::RbacArtifactPermissionEvent;
use sea_orm::{
    ConnectionTrait, DatabaseConnection, DatabaseTransaction, DbBackend, Statement,
    TransactionTrait,
};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid::Uuid;

const MAX_PERMISSION_KEY_LENGTH: usize = 256;
const MAX_IDEMPOTENCY_KEY_LENGTH: usize = 128;

/// Explicit admitted scope selected by an operator-facing mutation.
///
/// Tenant scope always resolves against the trusted command tenant. Callers cannot
/// supply a second tenant identifier or rely on platform/tenant precedence.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ArtifactPermissionAssignmentScope {
    Platform,
    Tenant,
}

/// An explicit role grant or revocation for one admitted artifact permission.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArtifactRolePermissionAssignmentCommand {
    pub tenant_id: Uuid,
    pub role_id: Uuid,
    pub scope: ArtifactPermissionAssignmentScope,
    pub installation_id: Uuid,
    pub permission_key: String,
    pub actor_id: Uuid,
    pub granted: bool,
    pub idempotency_key: String,
}

/// Result of applying an idempotent artifact-permission operation.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArtifactRolePermissionAssignmentResult {
    /// `true` when this request changed or confirmed durable state; `false` for an exact retry.
    pub applied: bool,
}

/// Errors exposed by the RBAC owner boundary for artifact permission grants.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum ArtifactPermissionAssignmentError {
    #[error("artifact permission assignment command is invalid: {0}")]
    InvalidCommand(&'static str),
    #[error("idempotency key was already used for a different artifact permission command")]
    IdempotencyConflict,
    #[error("role does not exist in the requested tenant")]
    RoleNotFound,
    #[error("artifact permission is not registered for the requested explicit scope")]
    PermissionNotRegistered,
    #[error("artifact permission assignment storage failed: {0}")]
    Database(String),
}

/// Host-neutral publisher used by the RBAC owner while its transaction is live.
///
/// Implementations must persist the typed event through the configured durable
/// transport using the supplied owner transaction. Returning an error causes
/// RBAC to roll back both the mutation and its idempotency receipt.
#[async_trait]
pub trait ArtifactPermissionEventPublisher: Send + Sync {
    async fn publish_assignment_changed(
        &self,
        transaction: &DatabaseTransaction,
        tenant_id: Uuid,
        actor_id: Uuid,
        event: RbacArtifactPermissionEvent,
    ) -> Result<(), ArtifactPermissionAssignmentError>;
}

/// Durable RBAC owner service for explicit dynamic artifact permission grants.
///
/// This service never writes the static `role_permissions` relation. Dynamic
/// permissions remain bound to the exact immutable permission definition and
/// admitted scope. The idempotency receipt, grant/revocation, and typed event are
/// committed by one owner transaction. State no-ops commit their receipt but do
/// not publish a false change event.
#[derive(Clone)]
pub struct RbacArtifactPermissionAssignmentService {
    db: DatabaseConnection,
    event_publisher: Arc<dyn ArtifactPermissionEventPublisher>,
}

impl RbacArtifactPermissionAssignmentService {
    pub fn new(
        db: DatabaseConnection,
        event_publisher: Arc<dyn ArtifactPermissionEventPublisher>,
    ) -> Self {
        Self {
            db,
            event_publisher,
        }
    }

    pub async fn assign(
        &self,
        command: ArtifactRolePermissionAssignmentCommand,
    ) -> Result<ArtifactRolePermissionAssignmentResult, ArtifactPermissionAssignmentError> {
        validate_command(&command)?;
        ensure_supported_backend(self.db.get_database_backend())?;

        let transaction = self.db.begin().await.map_err(database_error)?;
        if !role_exists(&transaction, &command).await? {
            transaction.rollback().await.map_err(database_error)?;
            return Err(ArtifactPermissionAssignmentError::RoleNotFound);
        }
        let artifact_permission =
            resolve_artifact_permission_identity(&transaction, &command).await?;

        if let Some(existing) = find_operation(&transaction, &command).await? {
            return match_operation(existing, &artifact_permission, &command);
        }

        let operation_id = match insert_operation(&transaction, &artifact_permission, &command)
            .await?
        {
            Some(operation_id) => operation_id,
            None => {
                let existing = find_operation(&transaction, &command)
                    .await?
                    .ok_or_else(|| {
                        ArtifactPermissionAssignmentError::Database(
                            "artifact permission operation disappeared after an idempotency conflict"
                                .to_string(),
                        )
                    })?;
                return match_operation(existing, &artifact_permission, &command);
            }
        };

        let changed = if command.granted {
            grant_permission(&transaction, &artifact_permission, &command).await?
        } else {
            revoke_permission(&transaction, &artifact_permission, &command).await?
        };
        if changed
            && let Err(error) = self
                .event_publisher
                .publish_assignment_changed(
                    &transaction,
                    command.tenant_id,
                    command.actor_id,
                    assignment_event(operation_id, &artifact_permission, &command),
                )
                .await
        {
            transaction.rollback().await.map_err(database_error)?;
            return Err(error);
        }
        transaction.commit().await.map_err(database_error)?;

        Ok(ArtifactRolePermissionAssignmentResult { applied: true })
    }
}

/// Read-only authorizer for a user's role-derived artifact permission.
#[derive(Clone)]
pub struct SeaOrmArtifactPermissionAuthorizer {
    db: DatabaseConnection,
}

impl SeaOrmArtifactPermissionAuthorizer {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }

    pub async fn is_authorized(
        &self,
        tenant_id: Uuid,
        user_id: Uuid,
        installation_id: Uuid,
        permission_key: &str,
    ) -> Result<bool, ArtifactPermissionAssignmentError> {
        if tenant_id.is_nil() || user_id.is_nil() || installation_id.is_nil() {
            return Err(ArtifactPermissionAssignmentError::InvalidCommand(
                "authorization identity must be present",
            ));
        }
        validate_permission_key(permission_key)?;
        let backend = self.db.get_database_backend();
        ensure_supported_backend(backend)?;
        let tenant_scope = format!("tenant:{tenant_id}");
        let sql = placeholders(
            backend,
            "SELECT 1 FROM users u INNER JOIN user_roles ur ON ur.user_id = u.id INNER JOIN roles r ON r.id = ur.role_id INNER JOIN rbac_artifact_role_permissions arp ON arp.role_id = r.id AND arp.tenant_id = r.tenant_id INNER JOIN rbac_artifact_permission_definitions apd ON apd.id = arp.artifact_permission_id AND apd.scope_key = arp.permission_scope_key WHERE u.id = {user_id} AND u.tenant_id = {tenant_id} AND r.tenant_id = {tenant_id} AND apd.installation_id = {installation_id} AND apd.permission_key = {permission_key} AND (apd.scope_key = 'platform' OR apd.scope_key = {tenant_scope}) LIMIT 1",
        );
        Ok(self
            .db
            .query_one_raw(Statement::from_sql_and_values(
                backend,
                sql,
                vec![
                    uuid_value(backend, user_id),
                    uuid_value(backend, tenant_id),
                    uuid_value(backend, installation_id),
                    permission_key.into(),
                    tenant_scope.into(),
                ],
            ))
            .await
            .map_err(database_error)?
            .is_some())
    }
}

#[derive(Debug)]
struct ArtifactPermissionIdentity {
    id: Uuid,
    scope_key: String,
    installation_id: Uuid,
    permission_key: String,
}

#[derive(Debug)]
struct StoredOperation {
    role_id: Uuid,
    artifact_permission_id: Uuid,
    permission_scope_key: String,
    actor_id: Uuid,
    granted: bool,
}

fn validate_command(
    command: &ArtifactRolePermissionAssignmentCommand,
) -> Result<(), ArtifactPermissionAssignmentError> {
    if command.tenant_id.is_nil()
        || command.role_id.is_nil()
        || command.installation_id.is_nil()
        || command.actor_id.is_nil()
    {
        return Err(ArtifactPermissionAssignmentError::InvalidCommand(
            "tenant, role, installation, and actor identities must be present",
        ));
    }
    validate_permission_key(&command.permission_key)?;
    validate_text_token(
        &command.idempotency_key,
        MAX_IDEMPOTENCY_KEY_LENGTH,
        "idempotency key",
    )
}

fn validate_permission_key(permission_key: &str) -> Result<(), ArtifactPermissionAssignmentError> {
    validate_text_token(permission_key, MAX_PERMISSION_KEY_LENGTH, "permission key")
}

fn validate_text_token(
    value: &str,
    maximum_length: usize,
    label: &'static str,
) -> Result<(), ArtifactPermissionAssignmentError> {
    if value.is_empty()
        || value.len() > maximum_length
        || value.trim() != value
        || value.chars().any(char::is_control)
    {
        return Err(ArtifactPermissionAssignmentError::InvalidCommand(label));
    }
    Ok(())
}

fn scope_key(scope: ArtifactPermissionAssignmentScope, tenant_id: Uuid) -> String {
    match scope {
        ArtifactPermissionAssignmentScope::Platform => "platform".to_string(),
        ArtifactPermissionAssignmentScope::Tenant => format!("tenant:{tenant_id}"),
    }
}

fn uuid_value(backend: DbBackend, value: Uuid) -> sea_orm::Value {
    match backend {
        DbBackend::Postgres => value.into(),
        DbBackend::Sqlite => value.to_string().into(),
        _ => unreachable!("unsupported database backend was validated"),
    }
}

fn uuid_from_row(
    backend: DbBackend,
    row: &sea_orm::QueryResult,
    column: &str,
) -> Result<Uuid, ArtifactPermissionAssignmentError> {
    match backend {
        DbBackend::Postgres => row.try_get("", column).map_err(database_error),
        DbBackend::Sqlite => {
            let value: String = row.try_get("", column).map_err(database_error)?;
            Uuid::parse_str(&value).map_err(|error| {
                ArtifactPermissionAssignmentError::Database(format!(
                    "artifact permission UUID column `{column}` contains invalid value `{value}`: {error}"
                ))
            })
        }
        _ => unreachable!("unsupported database backend was validated"),
    }
}
fn ensure_supported_backend(backend: DbBackend) -> Result<(), ArtifactPermissionAssignmentError> {
    match backend {
        DbBackend::Postgres | DbBackend::Sqlite => Ok(()),
        backend => Err(ArtifactPermissionAssignmentError::Database(format!(
            "artifact permission assignment does not support {backend:?}"
        ))),
    }
}

async fn find_operation(
    transaction: &DatabaseTransaction,
    command: &ArtifactRolePermissionAssignmentCommand,
) -> Result<Option<StoredOperation>, ArtifactPermissionAssignmentError> {
    let backend = transaction.get_database_backend();
    let sql = placeholders(
        backend,
        "SELECT role_id, artifact_permission_id, permission_scope_key, actor_id, granted FROM rbac_artifact_role_permission_operations WHERE tenant_id = {tenant_id} AND idempotency_key = {idempotency_key} LIMIT 1",
    );
    transaction
        .query_one_raw(Statement::from_sql_and_values(
            backend,
            sql,
            vec![
                uuid_value(backend, command.tenant_id),
                command.idempotency_key.clone().into(),
            ],
        ))
        .await
        .map_err(database_error)?
        .map(|row| {
            Ok(StoredOperation {
                role_id: uuid_from_row(backend, &row, "role_id")?,
                artifact_permission_id: uuid_from_row(backend, &row, "artifact_permission_id")?,
                permission_scope_key: row
                    .try_get("", "permission_scope_key")
                    .map_err(database_error)?,
                actor_id: uuid_from_row(backend, &row, "actor_id")?,
                granted: row.try_get("", "granted").map_err(database_error)?,
            })
        })
        .transpose()
}

fn match_operation(
    existing: StoredOperation,
    artifact_permission: &ArtifactPermissionIdentity,
    command: &ArtifactRolePermissionAssignmentCommand,
) -> Result<ArtifactRolePermissionAssignmentResult, ArtifactPermissionAssignmentError> {
    if existing.role_id != command.role_id
        || existing.artifact_permission_id != artifact_permission.id
        || existing.permission_scope_key != artifact_permission.scope_key
        || existing.actor_id != command.actor_id
        || existing.granted != command.granted
    {
        return Err(ArtifactPermissionAssignmentError::IdempotencyConflict);
    }
    Ok(ArtifactRolePermissionAssignmentResult { applied: false })
}

async fn insert_operation(
    transaction: &DatabaseTransaction,
    artifact_permission: &ArtifactPermissionIdentity,
    command: &ArtifactRolePermissionAssignmentCommand,
) -> Result<Option<Uuid>, ArtifactPermissionAssignmentError> {
    let backend = transaction.get_database_backend();
    let operation_id = rustok_core::generate_id();
    let sql = placeholders(
        backend,
        "INSERT INTO rbac_artifact_role_permission_operations (id, tenant_id, idempotency_key, role_id, artifact_permission_id, permission_scope_key, actor_id, granted) VALUES ({id}, {tenant_id}, {idempotency_key}, {role_id}, {artifact_permission_id}, {permission_scope_key}, {actor_id}, {granted}) ON CONFLICT (tenant_id, idempotency_key) DO NOTHING",
    );
    let result = transaction
        .execute_raw(Statement::from_sql_and_values(
            backend,
            sql,
            vec![
                uuid_value(backend, operation_id),
                uuid_value(backend, command.tenant_id),
                command.idempotency_key.clone().into(),
                uuid_value(backend, command.role_id),
                uuid_value(backend, artifact_permission.id),
                artifact_permission.scope_key.clone().into(),
                uuid_value(backend, command.actor_id),
                command.granted.into(),
            ],
        ))
        .await
        .map_err(database_error)?;
    Ok((result.rows_affected() == 1).then_some(operation_id))
}

async fn role_exists(
    transaction: &DatabaseTransaction,
    command: &ArtifactRolePermissionAssignmentCommand,
) -> Result<bool, ArtifactPermissionAssignmentError> {
    let backend = transaction.get_database_backend();
    let sql = placeholders(
        backend,
        "SELECT 1 FROM roles WHERE id = {role_id} AND tenant_id = {tenant_id} LIMIT 1",
    );
    Ok(transaction
        .query_one_raw(Statement::from_sql_and_values(
            backend,
            sql,
            vec![
                uuid_value(backend, command.role_id),
                uuid_value(backend, command.tenant_id),
            ],
        ))
        .await
        .map_err(database_error)?
        .is_some())
}

async fn resolve_artifact_permission_identity(
    transaction: &DatabaseTransaction,
    command: &ArtifactRolePermissionAssignmentCommand,
) -> Result<ArtifactPermissionIdentity, ArtifactPermissionAssignmentError> {
    let backend = transaction.get_database_backend();
    let requested_scope_key = scope_key(command.scope, command.tenant_id);
    let sql = placeholders(
        backend,
        "SELECT id, scope_key, installation_id, permission_key FROM rbac_artifact_permission_definitions WHERE scope_key = {scope_key} AND installation_id = {installation_id} AND permission_key = {permission_key} LIMIT 1",
    );
    transaction
        .query_one_raw(Statement::from_sql_and_values(
            backend,
            sql,
            vec![
                requested_scope_key.into(),
                uuid_value(backend, command.installation_id),
                command.permission_key.clone().into(),
            ],
        ))
        .await
        .map_err(database_error)?
        .map(|row| {
            Ok(ArtifactPermissionIdentity {
                id: uuid_from_row(backend, &row, "id")?,
                scope_key: row.try_get("", "scope_key").map_err(database_error)?,
                installation_id: uuid_from_row(backend, &row, "installation_id")?,
                permission_key: row.try_get("", "permission_key").map_err(database_error)?,
            })
        })
        .transpose()?
        .ok_or(ArtifactPermissionAssignmentError::PermissionNotRegistered)
}

async fn grant_permission(
    transaction: &DatabaseTransaction,
    artifact_permission: &ArtifactPermissionIdentity,
    command: &ArtifactRolePermissionAssignmentCommand,
) -> Result<bool, ArtifactPermissionAssignmentError> {
    let backend = transaction.get_database_backend();
    let sql = placeholders(
        backend,
        "INSERT INTO rbac_artifact_role_permissions (id, tenant_id, role_id, artifact_permission_id, permission_scope_key, granted_by_actor_id) VALUES ({id}, {tenant_id}, {role_id}, {artifact_permission_id}, {permission_scope_key}, {actor_id}) ON CONFLICT (tenant_id, role_id, artifact_permission_id) DO NOTHING",
    );
    let result = transaction
        .execute_raw(Statement::from_sql_and_values(
            backend,
            sql,
            vec![
                uuid_value(backend, rustok_core::generate_id()),
                uuid_value(backend, command.tenant_id),
                uuid_value(backend, command.role_id),
                uuid_value(backend, artifact_permission.id),
                artifact_permission.scope_key.clone().into(),
                uuid_value(backend, command.actor_id),
            ],
        ))
        .await
        .map_err(database_error)?;
    Ok(result.rows_affected() == 1)
}

async fn revoke_permission(
    transaction: &DatabaseTransaction,
    artifact_permission: &ArtifactPermissionIdentity,
    command: &ArtifactRolePermissionAssignmentCommand,
) -> Result<bool, ArtifactPermissionAssignmentError> {
    let backend = transaction.get_database_backend();
    let sql = placeholders(
        backend,
        "DELETE FROM rbac_artifact_role_permissions WHERE tenant_id = {tenant_id} AND role_id = {role_id} AND artifact_permission_id = {artifact_permission_id} AND permission_scope_key = {permission_scope_key}",
    );
    let result = transaction
        .execute_raw(Statement::from_sql_and_values(
            backend,
            sql,
            vec![
                uuid_value(backend, command.tenant_id),
                uuid_value(backend, command.role_id),
                uuid_value(backend, artifact_permission.id),
                artifact_permission.scope_key.clone().into(),
            ],
        ))
        .await
        .map_err(database_error)?;
    Ok(result.rows_affected() == 1)
}

fn assignment_event(
    operation_id: Uuid,
    artifact_permission: &ArtifactPermissionIdentity,
    command: &ArtifactRolePermissionAssignmentCommand,
) -> RbacArtifactPermissionEvent {
    RbacArtifactPermissionEvent::AssignmentChanged {
        operation_id,
        artifact_permission_id: artifact_permission.id,
        role_id: command.role_id,
        installation_id: artifact_permission.installation_id,
        permission_key: artifact_permission.permission_key.clone(),
        granted: command.granted,
    }
}

fn placeholders(backend: DbBackend, template: &str) -> String {
    let mut sql = template.to_string();
    let mut names = Vec::new();
    while let Some(start) = sql.find('{') {
        let end = sql[start..]
            .find('}')
            .map(|offset| start + offset)
            .expect("placeholder must have a closing brace");
        let name = sql[start + 1..end].to_string();
        let index = match names.iter().position(|known| known == &name) {
            Some(index) => index,
            None => {
                names.push(name);
                names.len() - 1
            }
        };
        let placeholder = match backend {
            DbBackend::Sqlite => format!("?{}", index + 1),
            DbBackend::Postgres => format!("${}", index + 1),
            _ => unreachable!("unsupported database backend was validated"),
        };
        sql.replace_range(start..=end, &placeholder);
    }
    sql
}

fn database_error(error: impl std::fmt::Display) -> ArtifactPermissionAssignmentError {
    ArtifactPermissionAssignmentError::Database(error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn command(
        scope: ArtifactPermissionAssignmentScope,
    ) -> ArtifactRolePermissionAssignmentCommand {
        ArtifactRolePermissionAssignmentCommand {
            tenant_id: Uuid::new_v4(),
            role_id: Uuid::new_v4(),
            scope,
            installation_id: Uuid::new_v4(),
            permission_key: "sample.events.handle".to_string(),
            actor_id: Uuid::new_v4(),
            granted: true,
            idempotency_key: "grant-1".to_string(),
        }
    }

    #[test]
    fn assignment_validation_rejects_nil_installation_and_invalid_key() {
        let mut invalid_installation = command(ArtifactPermissionAssignmentScope::Tenant);
        invalid_installation.installation_id = Uuid::nil();
        assert!(matches!(
            validate_command(&invalid_installation),
            Err(ArtifactPermissionAssignmentError::InvalidCommand(
                "tenant, role, installation, and actor identities must be present"
            ))
        ));

        let mut invalid_key = command(ArtifactPermissionAssignmentScope::Tenant);
        invalid_key.permission_key = " sample.events.handle".to_string();
        assert!(matches!(
            validate_command(&invalid_key),
            Err(ArtifactPermissionAssignmentError::InvalidCommand(
                "permission key"
            ))
        ));
    }

    #[test]
    fn assignment_scope_uses_trusted_tenant_without_fallback() {
        let tenant_id = Uuid::new_v4();
        assert_eq!(
            scope_key(ArtifactPermissionAssignmentScope::Platform, tenant_id),
            "platform"
        );
        assert_eq!(
            scope_key(ArtifactPermissionAssignmentScope::Tenant, tenant_id),
            format!("tenant:{tenant_id}")
        );
    }

    #[test]
    fn exact_operation_retry_is_not_applied_twice() {
        let command = command(ArtifactPermissionAssignmentScope::Tenant);
        let artifact_permission = ArtifactPermissionIdentity {
            id: Uuid::new_v4(),
            scope_key: scope_key(command.scope, command.tenant_id),
            installation_id: command.installation_id,
            permission_key: command.permission_key.clone(),
        };
        let result = match_operation(
            StoredOperation {
                role_id: command.role_id,
                artifact_permission_id: artifact_permission.id,
                permission_scope_key: artifact_permission.scope_key.clone(),
                actor_id: command.actor_id,
                granted: command.granted,
            },
            &artifact_permission,
            &command,
        )
        .expect("exact retry");
        assert!(!result.applied);
    }

    #[test]
    fn assignment_event_retains_operation_and_exact_permission_identity() {
        let command = command(ArtifactPermissionAssignmentScope::Platform);
        let operation_id = Uuid::new_v4();
        let artifact_permission = ArtifactPermissionIdentity {
            id: Uuid::new_v4(),
            scope_key: "platform".to_string(),
            installation_id: command.installation_id,
            permission_key: command.permission_key.clone(),
        };
        assert_eq!(
            assignment_event(operation_id, &artifact_permission, &command),
            RbacArtifactPermissionEvent::AssignmentChanged {
                operation_id,
                artifact_permission_id: artifact_permission.id,
                role_id: command.role_id,
                installation_id: artifact_permission.installation_id,
                permission_key: artifact_permission.permission_key,
                granted: command.granted,
            }
        );
    }
}
