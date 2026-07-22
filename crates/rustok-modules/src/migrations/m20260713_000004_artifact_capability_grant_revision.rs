use sea_orm::{ConnectionTrait, DbBackend, Statement};
use sea_orm_migration::prelude::*;

/// Persists the owner-selected capability-grant revision independently of an
/// artifact's declared capabilities and the policy revision used to evaluate it.
#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let statement = match manager.get_database_backend() {
            DbBackend::Postgres => {
                "ALTER TABLE module_artifact_installations \
                ADD COLUMN capability_grant_revision BIGINT NOT NULL DEFAULT 1 \
                CHECK (capability_grant_revision > 0)"
            }
            DbBackend::Sqlite => {
                "ALTER TABLE module_artifact_installations \
                ADD COLUMN capability_grant_revision INTEGER NOT NULL DEFAULT 1 \
                CHECK (capability_grant_revision > 0)"
            }
            backend => {
                return Err(DbErr::Migration(format!(
                    "module artifact capability-grant migration does not support database backend {backend:?}"
                )));
            }
        };
        manager
            .get_connection()
            .execute(Statement::from_string(
                manager.get_database_backend(),
                statement,
            ))
            .await?;
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let statement = "ALTER TABLE module_artifact_installations \
            DROP COLUMN capability_grant_revision";
        manager
            .get_connection()
            .execute(Statement::from_string(
                manager.get_database_backend(),
                statement,
            ))
            .await?;
        Ok(())
    }
}
