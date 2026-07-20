//! RBAC-owned grants and checks for immutable artifact permission vocabulary.

use sea_orm::{
    ConnectionTrait, DatabaseConnection, DatabaseTransaction, DbBackend, Statement,
    TransactionTrait,
};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid::Uuid;

const MAX_PERMISSION_KEY_LENGTH: usize = 256;
const MAX_IDEMPOTENCY_KEY_LENGTH: usize = 128;

/// An explicit role grant or revocation for one admitted artifact permission.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArtifactRolePermissionAssignmentCommand {
    pub tenant_id: Uuid,
    pub role_id: Uuid,
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
    #[error(
        "artifact permission is not registered for the requested installation and tenant scope"
    )]
    PermissionNotRegistered,
    #[error("artifact permission assignment storage failed: {0}")]
    Database(String),
}

/// Durable RBAC owner service for explicit dynamic artifact permission grants.
///
/// This service never writes the static `role_permissions` relation. Dynamic
/// permissions remain bound to the admitted installation that declared them.
#[derive(Clone)]
pub struct RbacArtifactPermissionAssignmentService {
    db: DatabaseConnection,
}

impl RbacArtifactPermissionAssignmentService {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }

    pub async fn assign(
        &self,
        command: ArtifactRolePermissionAssignmentCommand,
    ) -> Result<ArtifactRolePermissionAssignmentResult, ArtifactPermissionAssignmentError> {
        validate_command(&command)?;
        ensure_supported_backend(self.db.get_database_backend())?;

        let transaction = self.db.begin().await.map_err(database_error)?;
        if let Some(existing) = find_operation(&transaction, &command).await? {
            return match_operation(existing, &command);
        }

        let inserted_operation = insert_operation(&transaction, &command).await?;
        if !inserted_operation {
            let existing = find_operation(&transaction, &command)
                .await?
                .ok_or_else(|| {
                    ArtifactPermissionAssignmentError::Database(
                        "artifact permission operation disappeared after an idempotency conflict"
                            .to_string(),
                    )
                })?;
            return match_operation(existing, &command);
        }

        if !role_exists(&transaction, &command).await? {
            return Err(ArtifactPermissionAssignmentError::RoleNotFound);
        }
        if !permission_is_registered(&transaction, &command).await? {
            return Err(ArtifactPermissionAssignmentError::PermissionNotRegistered);
        }

        if command.granted {
            grant_permission(&transaction, &command).await?;
        } else {
            revoke_permission(&transaction, &command).await?;
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
        let sql = placeholders(
            backend,
            "SELECT 1 FROM users u INNER JOIN user_roles ur ON ur.user_id = u.id INNER JOIN roles r ON r.id = ur.role_id INNER JOIN rbac_artifact_role_permissions arp ON arp.role_id = r.id WHERE u.id = {user_id} AND u.tenant_id = {tenant_id} AND r.tenant_id = {tenant_id} AND arp.tenant_id = {tenant_id} AND arp.installation_id = {installation_id} AND arp.permission_key = {permission_key} LIMIT 1",
        );
        Ok(self
            .db
            .query_one(Statement::from_sql_and_values(
                backend,
                sql,
                vec![
                    user_id.into(),
                    tenant_id.into(),
                    installation_id.into(),
                    permission_key.into(),
                ],
            ))
            .await
            .map_err(database_error)?
            .is_some())
    }
}

#[derive(Debug)]
struct StoredOperation {
    role_id: Uuid,
    installation_id: Uuid,
    permission_key: String,
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
        "SELECT role_id, installation_id, permission_key, actor_id, granted FROM rbac_artifact_role_permission_operations WHERE tenant_id = {tenant_id} AND idempotency_key = {idempotency_key} LIMIT 1",
    );
    transaction
        .query_one(Statement::from_sql_and_values(
            backend,
            sql,
            vec![
                command.tenant_id.into(),
                command.idempotency_key.clone().into(),
            ],
        ))
        .await
        .map_err(database_error)?
        .map(|row| {
            Ok(StoredOperation {
                role_id: row.try_get("", "role_id").map_err(database_error)?,
                installation_id: row.try_get("", "installation_id").map_err(database_error)?,
                permission_key: row.try_get("", "permission_key").map_err(database_error)?,
                actor_id: row.try_get("", "actor_id").map_err(database_error)?,
                granted: row.try_get("", "granted").map_err(database_error)?,
            })
        })
        .transpose()
}

fn match_operation(
    existing: StoredOperation,
    command: &ArtifactRolePermissionAssignmentCommand,
) -> Result<ArtifactRolePermissionAssignmentResult, ArtifactPermissionAssignmentError> {
    if existing.role_id != command.role_id
        || existing.installation_id != command.installation_id
        || existing.permission_key != command.permission_key
        || existing.actor_id != command.actor_id
        || existing.granted != command.granted
    {
        return Err(ArtifactPermissionAssignmentError::IdempotencyConflict);
    }
    Ok(ArtifactRolePermissionAssignmentResult { applied: false })
}

async fn insert_operation(
    transaction: &DatabaseTransaction,
    command: &ArtifactRolePermissionAssignmentCommand,
) -> Result<bool, ArtifactPermissionAssignmentError> {
    let backend = transaction.get_database_backend();
    let sql = placeholders(
        backend,
        "INSERT INTO rbac_artifact_role_permission_operations (id, tenant_id, idempotency_key, role_id, installation_id, permission_key, actor_id, granted) VALUES ({id}, {tenant_id}, {idempotency_key}, {role_id}, {installation_id}, {permission_key}, {actor_id}, {granted}) ON CONFLICT (tenant_id, idempotency_key) DO NOTHING",
    );
    let result = transaction
        .execute(Statement::from_sql_and_values(
            backend,
            sql,
            vec![
                rustok_core::generate_id().into(),
                command.tenant_id.into(),
                command.idempotency_key.clone().into(),
                command.role_id.into(),
                command.installation_id.into(),
                command.permission_key.clone().into(),
                command.actor_id.into(),
                command.granted.into(),
            ],
        ))
        .await
        .map_err(database_error)?;
    Ok(result.rows_affected() == 1)
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
        .query_one(Statement::from_sql_and_values(
            backend,
            sql,
            vec![command.role_id.into(), command.tenant_id.into()],
        ))
        .await
        .map_err(database_error)?
        .is_some())
}

async fn permission_is_registered(
    transaction: &DatabaseTransaction,
    command: &ArtifactRolePermissionAssignmentCommand,
) -> Result<bool, ArtifactPermissionAssignmentError> {
    let backend = transaction.get_database_backend();
    let scope_key = format!("tenant:{}", command.tenant_id);
    let sql = placeholders(
        backend,
        "SELECT 1 FROM rbac_artifact_permission_catalog WHERE installation_id = {installation_id} AND permission_key = {permission_key} AND (scope_key = 'platform' OR scope_key = {scope_key}) LIMIT 1",
    );
    Ok(transaction
        .query_one(Statement::from_sql_and_values(
            backend,
            sql,
            vec![
                command.installation_id.into(),
                command.permission_key.clone().into(),
                scope_key.into(),
            ],
        ))
        .await
        .map_err(database_error)?
        .is_some())
}

async fn grant_permission(
    transaction: &DatabaseTransaction,
    command: &ArtifactRolePermissionAssignmentCommand,
) -> Result<(), ArtifactPermissionAssignmentError> {
    let backend = transaction.get_database_backend();
    let sql = placeholders(
        backend,
        "INSERT INTO rbac_artifact_role_permissions (id, tenant_id, role_id, installation_id, permission_key, granted_by_actor_id) VALUES ({id}, {tenant_id}, {role_id}, {installation_id}, {permission_key}, {actor_id}) ON CONFLICT (tenant_id, role_id, installation_id, permission_key) DO NOTHING",
    );
    transaction
        .execute(Statement::from_sql_and_values(
            backend,
            sql,
            vec![
                rustok_core::generate_id().into(),
                command.tenant_id.into(),
                command.role_id.into(),
                command.installation_id.into(),
                command.permission_key.clone().into(),
                command.actor_id.into(),
            ],
        ))
        .await
        .map_err(database_error)?;
    Ok(())
}

async fn revoke_permission(
    transaction: &DatabaseTransaction,
    command: &ArtifactRolePermissionAssignmentCommand,
) -> Result<(), ArtifactPermissionAssignmentError> {
    let backend = transaction.get_database_backend();
    let sql = placeholders(
        backend,
        "DELETE FROM rbac_artifact_role_permissions WHERE tenant_id = {tenant_id} AND role_id = {role_id} AND installation_id = {installation_id} AND permission_key = {permission_key}",
    );
    transaction
        .execute(Statement::from_sql_and_values(
            backend,
            sql,
            vec![
                command.tenant_id.into(),
                command.role_id.into(),
                command.installation_id.into(),
                command.permission_key.clone().into(),
            ],
        ))
        .await
        .map_err(database_error)?;
    Ok(())
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

    #[test]
    fn assignment_validation_rejects_empty_or_control_text_tokens() {
        let mut command = ArtifactRolePermissionAssignmentCommand {
            tenant_id: Uuid::new_v4(),
            role_id: Uuid::new_v4(),
            installation_id: Uuid::new_v4(),
            permission_key: "sample.events.handle".to_string(),
            actor_id: Uuid::new_v4(),
            granted: true,
            idempotency_key: "grant-1".to_string(),
        };
        command.permission_key = " sample.events.handle".to_string();
        assert!(matches!(
            validate_command(&command),
            Err(ArtifactPermissionAssignmentError::InvalidCommand(
                "permission key"
            ))
        ));
        command.permission_key = "sample.events\n.handle".to_string();
        assert!(matches!(
            validate_command(&command),
            Err(ArtifactPermissionAssignmentError::InvalidCommand(
                "permission key"
            ))
        ));
    }

    #[test]
    fn exact_operation_retry_is_not_applied_twice() {
        let command = ArtifactRolePermissionAssignmentCommand {
            tenant_id: Uuid::new_v4(),
            role_id: Uuid::new_v4(),
            installation_id: Uuid::new_v4(),
            permission_key: "sample.events.handle".to_string(),
            actor_id: Uuid::new_v4(),
            granted: true,
            idempotency_key: "grant-1".to_string(),
        };
        let result = match_operation(
            StoredOperation {
                role_id: command.role_id,
                installation_id: command.installation_id,
                permission_key: command.permission_key.clone(),
                actor_id: command.actor_id,
                granted: command.granted,
            },
            &command,
        )
        .expect("exact retry");
        assert!(!result.applied);
    }
}
