use sea_orm::{ConnectionTrait, DbBackend, Statement};
use sea_orm_migration::prelude::*;

/// Durable RBAC-owned metadata for permissions declared by admitted artifacts.
#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let statements: &[&str] = match manager.get_database_backend() {
            DbBackend::Postgres => &[
                "CREATE TABLE rbac_artifact_permission_catalog (id UUID PRIMARY KEY, scope_key TEXT NOT NULL, installation_id UUID NOT NULL, module_slug TEXT NOT NULL, release_digest TEXT NOT NULL, permission_key TEXT NOT NULL, locale TEXT NOT NULL, label TEXT NOT NULL, description TEXT NOT NULL, registered_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP, UNIQUE (scope_key, installation_id, permission_key, locale))",
                "CREATE INDEX rbac_artifact_permission_catalog_lookup_idx ON rbac_artifact_permission_catalog (scope_key, module_slug, permission_key)",
            ],
            DbBackend::Sqlite => &[
                "CREATE TABLE rbac_artifact_permission_catalog (id TEXT PRIMARY KEY, scope_key TEXT NOT NULL, installation_id TEXT NOT NULL, module_slug TEXT NOT NULL, release_digest TEXT NOT NULL, permission_key TEXT NOT NULL, locale TEXT NOT NULL, label TEXT NOT NULL, description TEXT NOT NULL, registered_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP, UNIQUE (scope_key, installation_id, permission_key, locale))",
                "CREATE INDEX rbac_artifact_permission_catalog_lookup_idx ON rbac_artifact_permission_catalog (scope_key, module_slug, permission_key)",
            ],
            backend => {
                return Err(DbErr::Migration(format!(
                    "artifact permission catalog migration does not support {backend:?}"
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
            .execute_unprepared("DROP TABLE rbac_artifact_permission_catalog")
            .await
            .map(|_| ())
    }
}
