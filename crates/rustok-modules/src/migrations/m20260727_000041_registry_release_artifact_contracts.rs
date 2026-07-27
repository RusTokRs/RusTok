use sea_orm::{ConnectionTrait, DbBackend, Statement};
use sea_orm_migration::prelude::*;

/// Adds the immutable source reference required for platform-built lineage and
/// stores the exact installable artifact contract beside each published
/// registry release.
#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let statements: &[&str] = match manager.get_database_backend() {
            DbBackend::Postgres => &[
                "ALTER TABLE registry_publish_build_staging \
                 ADD COLUMN source_reference TEXT NULL \
                 CHECK (source_reference IS NULL OR length(trim(source_reference)) BETWEEN 1 AND 512)",
                "CREATE TABLE registry_module_release_artifacts (\
                    release_id TEXT PRIMARY KEY REFERENCES registry_module_releases(id) ON DELETE RESTRICT,\
                    request_id TEXT NOT NULL UNIQUE REFERENCES registry_publish_requests(id) ON DELETE RESTRICT,\
                    artifact JSONB NOT NULL,\
                    descriptor JSONB NOT NULL,\
                    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP\
                )",
            ],
            DbBackend::Sqlite => &[
                "ALTER TABLE registry_publish_build_staging \
                 ADD COLUMN source_reference TEXT NULL \
                 CHECK (source_reference IS NULL OR length(trim(source_reference)) BETWEEN 1 AND 512)",
                "CREATE TABLE registry_module_release_artifacts (\
                    release_id TEXT PRIMARY KEY NOT NULL REFERENCES registry_module_releases(id) ON DELETE RESTRICT,\
                    request_id TEXT NOT NULL UNIQUE REFERENCES registry_publish_requests(id) ON DELETE RESTRICT,\
                    artifact JSON NOT NULL,\
                    descriptor JSON NOT NULL,\
                    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP\
                )",
            ],
            backend => {
                return Err(DbErr::Migration(format!(
                    "registry release artifact contracts do not support database backend {backend:?}"
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
            .execute_unprepared("DROP TABLE registry_module_release_artifacts")
            .await?;
        manager
            .get_connection()
            .execute_unprepared(
                "ALTER TABLE registry_publish_build_staging DROP COLUMN source_reference",
            )
            .await
            .map(|_| ())
    }
}
