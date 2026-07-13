use sea_orm::{ConnectionTrait, DbBackend, Statement};
use sea_orm_migration::prelude::*;

/// Adds the durable predecessor link required for a later atomic rollback
/// operation. The link is deliberately nullable: a module's first admitted
/// installation has no predecessor.
#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let statement = match manager.get_database_backend() {
            DbBackend::Postgres => "ALTER TABLE module_artifact_installations \
                ADD COLUMN previous_installation_id UUID NULL \
                REFERENCES module_artifact_installations(installation_id)",
            DbBackend::Sqlite => "ALTER TABLE module_artifact_installations \
                ADD COLUMN previous_installation_id TEXT NULL \
                REFERENCES module_artifact_installations(installation_id)",
            backend => {
                return Err(DbErr::Migration(format!(
                    "module artifact rollback-pointer migration does not support database backend {backend:?}"
                )));
            }
        };
        manager
            .get_connection()
            .execute(Statement::from_string(
                manager.get_database_backend(),
                statement.to_string(),
            ))
            .await?;
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let statement = match manager.get_database_backend() {
            DbBackend::Postgres => "ALTER TABLE module_artifact_installations \
                DROP COLUMN previous_installation_id",
            // SQLite cannot drop a referenced column without rebuilding the
            // table. The local test backend treats migration rollback as an
            // unsupported operation rather than silently losing data.
            DbBackend::Sqlite => {
                return Err(DbErr::Migration(
                    "SQLite rollback for module artifact rollback-pointer migration is unsupported"
                        .to_string(),
                ));
            }
            backend => {
                return Err(DbErr::Migration(format!(
                    "module artifact rollback-pointer migration does not support database backend {backend:?}"
                )));
            }
        };
        manager
            .get_connection()
            .execute(Statement::from_string(
                manager.get_database_backend(),
                statement.to_string(),
            ))
            .await?;
        Ok(())
    }
}
