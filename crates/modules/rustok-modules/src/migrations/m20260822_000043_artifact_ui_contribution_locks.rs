use sea_orm::{ConnectionTrait, DbBackend, Statement};
use sea_orm_migration::prelude::*;

/// Serializes activation of declarative UI resources. Rows are durable global
/// lock identities, not UI state; the activation transaction retains each row
/// lock while it verifies that no active artifact owns the same route or
/// storefront slot in the affected tenant surface.
#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let statements: &[&str] = match manager.get_database_backend() {
            DbBackend::Postgres => &["CREATE TABLE module_artifact_ui_contribution_locks (\
                    resource_kind TEXT NOT NULL CHECK (resource_kind IN ('navigation_route', 'storefront_slot')),\
                    resource_key TEXT NOT NULL CHECK (length(trim(resource_key)) BETWEEN 1 AND 128),\
                    PRIMARY KEY (resource_kind, resource_key)\
                )"],
            DbBackend::Sqlite => &["CREATE TABLE module_artifact_ui_contribution_locks (\
                resource_kind TEXT NOT NULL CHECK (resource_kind IN ('navigation_route', 'storefront_slot')),\
                resource_key TEXT NOT NULL CHECK (length(trim(resource_key)) BETWEEN 1 AND 128),\
                PRIMARY KEY (resource_kind, resource_key)\
            )"],
            backend => {
                return Err(DbErr::Migration(format!(
                    "artifact UI contribution lock migration does not support database backend {backend:?}"
                )));
            }
        };

        for statement in statements {
            manager
                .get_connection()
                .execute_raw(Statement::from_string(
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
            .execute_unprepared("DROP TABLE module_artifact_ui_contribution_locks")
            .await
            .map(|_| ())
    }
}

#[cfg(test)]
mod tests {
    use sea_orm::{ConnectionTrait, Database, DbBackend, Statement};
    use sea_orm_migration::prelude::{MigrationTrait, SchemaManager};

    use super::Migration;

    #[tokio::test]
    async fn sqlite_schema_keeps_one_global_lock_identity_per_ui_resource() {
        let database = Database::connect("sqlite::memory:")
            .await
            .expect("database");
        Migration
            .up(&SchemaManager::new(&database))
            .await
            .expect("migration");

        database
            .execute_raw(Statement::from_string(
                DbBackend::Sqlite,
                "INSERT INTO module_artifact_ui_contribution_locks \
                 (resource_kind, resource_key) \
                 VALUES ('navigation_route', 'settings')"
                    .to_string(),
            ))
            .await
            .expect("initial lock");
        assert!(
            database
                .execute_raw(Statement::from_string(
                    DbBackend::Sqlite,
                    "INSERT INTO module_artifact_ui_contribution_locks \
                 (resource_kind, resource_key) \
                 VALUES ('navigation_route', 'settings')"
                        .to_string(),
                ))
                .await
                .is_err()
        );
    }
}
