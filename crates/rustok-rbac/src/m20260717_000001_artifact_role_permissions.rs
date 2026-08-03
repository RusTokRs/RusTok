use sea_orm::{ConnectionTrait, DbBackend, Statement};
use sea_orm_migration::prelude::*;

/// Durable role grants and idempotent operator operations for admitted artifact permissions.
///
/// Every authorization row references the exact immutable permission definition and
/// its admitted scope plus tenant-composite role/user parents. Database foreign keys
/// provide concurrency-safe parent protection; scope checks reject cross-tenant
/// permission identity even for direct database writes.
#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let statements: &[&str] = match manager.get_database_backend() {
            DbBackend::Postgres => &[
                "CREATE UNIQUE INDEX IF NOT EXISTS uq_rbac_roles_tenant_id_id ON roles (tenant_id, id)",
                "CREATE UNIQUE INDEX IF NOT EXISTS uq_rbac_users_tenant_id_id ON users (tenant_id, id)",
                "CREATE TABLE rbac_artifact_role_permissions (id UUID PRIMARY KEY, tenant_id UUID NOT NULL, role_id UUID NOT NULL, artifact_permission_id UUID NOT NULL, permission_scope_key TEXT NOT NULL, granted_by_actor_id UUID NOT NULL, granted_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP, CONSTRAINT ck_rbac_artifact_grant_permission_scope CHECK (permission_scope_key = 'platform' OR permission_scope_key = 'tenant:' || tenant_id::text), CONSTRAINT fk_rbac_artifact_grant_role FOREIGN KEY (tenant_id, role_id) REFERENCES roles (tenant_id, id) ON UPDATE RESTRICT ON DELETE RESTRICT, CONSTRAINT fk_rbac_artifact_grant_actor FOREIGN KEY (tenant_id, granted_by_actor_id) REFERENCES users (tenant_id, id) ON UPDATE RESTRICT ON DELETE RESTRICT, CONSTRAINT fk_rbac_artifact_grant_permission FOREIGN KEY (artifact_permission_id, permission_scope_key) REFERENCES rbac_artifact_permission_definitions (id, scope_key) ON UPDATE RESTRICT ON DELETE RESTRICT, UNIQUE (tenant_id, role_id, artifact_permission_id))",
                "CREATE INDEX rbac_artifact_role_permissions_authorize_idx ON rbac_artifact_role_permissions (tenant_id, role_id, artifact_permission_id)",
                "CREATE TABLE rbac_artifact_role_permission_operations (id UUID PRIMARY KEY, tenant_id UUID NOT NULL, idempotency_key TEXT NOT NULL, role_id UUID NOT NULL, artifact_permission_id UUID NOT NULL, permission_scope_key TEXT NOT NULL, actor_id UUID NOT NULL, granted BOOLEAN NOT NULL, applied_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP, CONSTRAINT ck_rbac_artifact_operation_permission_scope CHECK (permission_scope_key = 'platform' OR permission_scope_key = 'tenant:' || tenant_id::text), CONSTRAINT fk_rbac_artifact_operation_role FOREIGN KEY (tenant_id, role_id) REFERENCES roles (tenant_id, id) ON UPDATE RESTRICT ON DELETE RESTRICT, CONSTRAINT fk_rbac_artifact_operation_actor FOREIGN KEY (tenant_id, actor_id) REFERENCES users (tenant_id, id) ON UPDATE RESTRICT ON DELETE RESTRICT, CONSTRAINT fk_rbac_artifact_operation_permission FOREIGN KEY (artifact_permission_id, permission_scope_key) REFERENCES rbac_artifact_permission_definitions (id, scope_key) ON UPDATE RESTRICT ON DELETE RESTRICT, UNIQUE (tenant_id, idempotency_key))",
            ],
            DbBackend::Sqlite => &[
                "CREATE UNIQUE INDEX IF NOT EXISTS uq_rbac_roles_tenant_id_id ON roles (tenant_id, id)",
                "CREATE UNIQUE INDEX IF NOT EXISTS uq_rbac_users_tenant_id_id ON users (tenant_id, id)",
                "CREATE TABLE rbac_artifact_role_permissions (id TEXT PRIMARY KEY, tenant_id TEXT NOT NULL, role_id TEXT NOT NULL, artifact_permission_id TEXT NOT NULL, permission_scope_key TEXT NOT NULL, granted_by_actor_id TEXT NOT NULL, granted_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP, CHECK (permission_scope_key = 'platform' OR permission_scope_key = 'tenant:' || tenant_id), FOREIGN KEY (tenant_id, role_id) REFERENCES roles (tenant_id, id) ON UPDATE RESTRICT ON DELETE RESTRICT, FOREIGN KEY (tenant_id, granted_by_actor_id) REFERENCES users (tenant_id, id) ON UPDATE RESTRICT ON DELETE RESTRICT, FOREIGN KEY (artifact_permission_id, permission_scope_key) REFERENCES rbac_artifact_permission_definitions (id, scope_key) ON UPDATE RESTRICT ON DELETE RESTRICT, UNIQUE (tenant_id, role_id, artifact_permission_id))",
                "CREATE INDEX rbac_artifact_role_permissions_authorize_idx ON rbac_artifact_role_permissions (tenant_id, role_id, artifact_permission_id)",
                "CREATE TABLE rbac_artifact_role_permission_operations (id TEXT PRIMARY KEY, tenant_id TEXT NOT NULL, idempotency_key TEXT NOT NULL, role_id TEXT NOT NULL, artifact_permission_id TEXT NOT NULL, permission_scope_key TEXT NOT NULL, actor_id TEXT NOT NULL, granted BOOLEAN NOT NULL, applied_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP, CHECK (permission_scope_key = 'platform' OR permission_scope_key = 'tenant:' || tenant_id), FOREIGN KEY (tenant_id, role_id) REFERENCES roles (tenant_id, id) ON UPDATE RESTRICT ON DELETE RESTRICT, FOREIGN KEY (tenant_id, actor_id) REFERENCES users (tenant_id, id) ON UPDATE RESTRICT ON DELETE RESTRICT, FOREIGN KEY (artifact_permission_id, permission_scope_key) REFERENCES rbac_artifact_permission_definitions (id, scope_key) ON UPDATE RESTRICT ON DELETE RESTRICT, UNIQUE (tenant_id, idempotency_key))",
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
            .await?;
        connection
            .execute_unprepared("DROP INDEX IF EXISTS uq_rbac_roles_tenant_id_id")
            .await?;
        connection
            .execute_unprepared("DROP INDEX IF EXISTS uq_rbac_users_tenant_id_id")
            .await
            .map(|_| ())
    }
}
