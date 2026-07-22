use sea_orm::{ConnectionTrait, DbBackend, Statement};
use sea_orm_migration::prelude::*;

/// Durable request identity, replay output, and recovery lease for artifact binding operations.
#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let statements: &[&str] = match manager.get_database_backend() {
            DbBackend::Postgres => &[
                "CREATE TABLE module_artifact_binding_operations (operation_id UUID PRIMARY KEY, tenant_id UUID NOT NULL, actor_id UUID NOT NULL, installation_id UUID NOT NULL, binding_id TEXT NOT NULL, idempotency_key TEXT NOT NULL, request_digest TEXT NOT NULL, status TEXT NOT NULL, response JSONB NULL, lease_expires_at TIMESTAMPTZ NOT NULL, created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP, completed_at TIMESTAMPTZ NULL, UNIQUE (tenant_id, actor_id, installation_id, binding_id, idempotency_key))",
                "CREATE INDEX module_artifact_binding_operations_recovery_idx ON module_artifact_binding_operations (status, lease_expires_at)",
            ],
            DbBackend::Sqlite => &[
                "CREATE TABLE module_artifact_binding_operations (operation_id TEXT PRIMARY KEY, tenant_id TEXT NOT NULL, actor_id TEXT NOT NULL, installation_id TEXT NOT NULL, binding_id TEXT NOT NULL, idempotency_key TEXT NOT NULL, request_digest TEXT NOT NULL, status TEXT NOT NULL, response TEXT NULL, lease_expires_at TEXT NOT NULL, created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP, completed_at TEXT NULL, UNIQUE (tenant_id, actor_id, installation_id, binding_id, idempotency_key))",
                "CREATE INDEX module_artifact_binding_operations_recovery_idx ON module_artifact_binding_operations (status, lease_expires_at)",
            ],
            backend => {
                return Err(DbErr::Migration(format!(
                    "artifact binding operation migration does not support {backend:?}"
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
        manager
            .get_connection()
            .execute_unprepared("DROP TABLE module_artifact_binding_operations")
            .await
            .map(|_| ())
    }
}
