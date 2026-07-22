use sea_orm::{ConnectionTrait, DbBackend, Statement};
use sea_orm_migration::prelude::*;

/// Durable role grants and idempotent operator operations for admitted artifact permissions.
#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let statements: &[&str] = match manager.get_database_backend() {
            DbBackend::Postgres => &[
                "CREATE TABLE rbac_artifact_role_permissions (id UUID PRIMARY KEY, tenant_id UUID NOT NULL, role_id UUID NOT NULL, installation_id UUID NOT NULL, permission_key TEXT NOT NULL, granted_by_actor_id UUID NOT NULL, granted_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP, UNIQUE (tenant_id, role_id, installation_id, permission_key))",
                "CREATE INDEX rbac_artifact_role_permissions_authorize_idx ON rbac_artifact_role_permissions (tenant_id, role_id, installation_id, permission_key)",
                "CREATE TABLE rbac_artifact_role_permission_operations (id UUID PRIMARY KEY, tenant_id UUID NOT NULL, idempotency_key TEXT NOT NULL, role_id UUID NOT NULL, installation_id UUID NOT NULL, permission_key TEXT NOT NULL, actor_id UUID NOT NULL, granted BOOLEAN NOT NULL, applied_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP, UNIQUE (tenant_id, idempotency_key))",
            ],
            DbBackend::Sqlite => &[
                "CREATE TABLE rbac_artifact_role_permissions (id TEXT PRIMARY KEY, tenant_id TEXT NOT NULL, role_id TEXT NOT NULL, installation_id TEXT NOT NULL, permission_key TEXT NOT NULL, granted_by_actor_id TEXT NOT NULL, granted_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP, UNIQUE (tenant_id, role_id, installation_id, permission_key))",
                "CREATE INDEX rbac_artifact_role_permissions_authorize_idx ON rbac_artifact_role_permissions (tenant_id, role_id, installation_id, permission_key)",
                "CREATE TABLE rbac_artifact_role_permission_operations (id TEXT PRIMARY KEY, tenant_id TEXT NOT NULL, idempotency_key TEXT NOT NULL, role_id TEXT NOT NULL, installation_id TEXT NOT NULL, permission_key TEXT NOT NULL, actor_id TEXT NOT NULL, granted BOOLEAN NOT NULL, applied_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP, UNIQUE (tenant_id, idempotency_key))",
            ],
            backend => {
                return Err(DbErr::Migration(format!(
                    "artifact role permission migration does not support {backend:?}"
                )));
            }
        };

        for statement in statements {
            manager
                .get_connection()
                .execute(Statement::from_string(
                    manager.get_database_backend(),
                    (*statement).to_string(),
                ))
                .await?;
        }
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let connection = manager.get_connection();
        connection
            .execute_unprepared("DROP TABLE rbac_artifact_role_permission_operations")
            .await?;
        connection
            .execute_unprepared("DROP TABLE rbac_artifact_role_permissions")
            .await
            .map(|_| ())
    }
}
