use sea_orm::{ConnectionTrait, DbBackend, Statement};
use sea_orm_migration::prelude::*;

/// Durable RBAC-owned metadata for permissions declared by admitted artifacts.
///
/// Language-neutral permission identity is stored separately from localized
/// labels and descriptions so authorization state can reference one immutable
/// parent row without selecting an arbitrary locale.
#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let statements: &[&str] = match manager.get_database_backend() {
            DbBackend::Postgres => &[
                "CREATE TABLE rbac_artifact_permission_definitions (id UUID PRIMARY KEY, scope_key TEXT NOT NULL, installation_id UUID NOT NULL, module_slug TEXT NOT NULL, release_digest TEXT NOT NULL, permission_key TEXT NOT NULL, registered_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP, UNIQUE (id, scope_key), UNIQUE (scope_key, installation_id, permission_key))",
                "CREATE INDEX rbac_artifact_permission_definitions_lookup_idx ON rbac_artifact_permission_definitions (scope_key, module_slug, permission_key)",
                "CREATE OR REPLACE FUNCTION rustok_reject_artifact_permission_definition_update() RETURNS TRIGGER LANGUAGE plpgsql AS $$ BEGIN RAISE EXCEPTION 'artifact permission definitions are immutable'; END; $$",
                "CREATE TRIGGER rbac_artifact_permission_definitions_immutable BEFORE UPDATE ON rbac_artifact_permission_definitions FOR EACH ROW EXECUTE FUNCTION rustok_reject_artifact_permission_definition_update()",
                "CREATE TABLE rbac_artifact_permission_translations (id UUID PRIMARY KEY, artifact_permission_id UUID NOT NULL REFERENCES rbac_artifact_permission_definitions (id) ON UPDATE RESTRICT ON DELETE CASCADE, locale VARCHAR(32) NOT NULL, label TEXT NOT NULL, description TEXT NOT NULL, registered_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP, UNIQUE (artifact_permission_id, locale))",
            ],
            DbBackend::Sqlite => &[
                "CREATE TABLE rbac_artifact_permission_definitions (id TEXT PRIMARY KEY, scope_key TEXT NOT NULL, installation_id TEXT NOT NULL, module_slug TEXT NOT NULL, release_digest TEXT NOT NULL, permission_key TEXT NOT NULL, registered_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP, UNIQUE (id, scope_key), UNIQUE (scope_key, installation_id, permission_key))",
                "CREATE INDEX rbac_artifact_permission_definitions_lookup_idx ON rbac_artifact_permission_definitions (scope_key, module_slug, permission_key)",
                "CREATE TRIGGER rbac_artifact_permission_definitions_immutable BEFORE UPDATE ON rbac_artifact_permission_definitions BEGIN SELECT RAISE(ABORT, 'artifact permission definitions are immutable'); END",
                "CREATE TABLE rbac_artifact_permission_translations (id TEXT PRIMARY KEY, artifact_permission_id TEXT NOT NULL REFERENCES rbac_artifact_permission_definitions (id) ON UPDATE RESTRICT ON DELETE CASCADE, locale VARCHAR(32) NOT NULL, label TEXT NOT NULL, description TEXT NOT NULL, registered_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP, UNIQUE (artifact_permission_id, locale))",
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
        let connection = manager.get_connection();
        connection
            .execute_unprepared("DROP TABLE rbac_artifact_permission_translations")
            .await?;
        connection
            .execute_unprepared("DROP TABLE rbac_artifact_permission_definitions")
            .await?;
        if manager.get_database_backend() == DbBackend::Postgres {
            connection
                .execute_unprepared(
                    "DROP FUNCTION IF EXISTS rustok_reject_artifact_permission_definition_update()",
                )
                .await?;
        }
        Ok(())
    }
}
