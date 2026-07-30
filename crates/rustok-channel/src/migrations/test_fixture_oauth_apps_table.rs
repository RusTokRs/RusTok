use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::{ConnectionTrait, DatabaseBackend, Statement};

/// Test-only fixture normalization.
///
/// Older channel unit fixtures created `o_auth_apps`, while the production owner
/// table and migration dependency have always been `oauth_apps`. This migration is
/// compiled and inserted into the migration list only under `cfg(test)`; it never
/// exists in the release migration graph and does not create a production alias.
#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        if manager.get_database_backend() != DatabaseBackend::Sqlite {
            return Ok(());
        }

        let canonical_exists = table_exists(manager, "oauth_apps").await?;
        let legacy_exists = table_exists(manager, "o_auth_apps").await?;
        match (canonical_exists, legacy_exists) {
            (false, true) => {
                manager
                    .get_connection()
                    .execute_unprepared("ALTER TABLE o_auth_apps RENAME TO oauth_apps")
                    .await?;
            }
            (true, true) => {
                return Err(DbErr::Custom(
                    "channel test fixture defines both oauth_apps and legacy o_auth_apps"
                        .to_string(),
                ));
            }
            _ => {}
        }
        Ok(())
    }

    async fn down(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
        Ok(())
    }
}

async fn table_exists(manager: &SchemaManager<'_>, table: &str) -> Result<bool, DbErr> {
    Ok(manager
        .get_connection()
        .query_one(Statement::from_sql_and_values(
            DatabaseBackend::Sqlite,
            "SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = ?1 LIMIT 1",
            vec![table.into()],
        ))
        .await?
        .is_some())
}

#[cfg(test)]
mod tests {
    use super::*;
    use sea_orm::{ConnectionTrait, Database};

    #[tokio::test]
    async fn renames_only_the_legacy_test_fixture_table() {
        let db = Database::connect("sqlite::memory:").await.unwrap();
        db.execute_unprepared(
            "CREATE TABLE o_auth_apps (id TEXT PRIMARY KEY, tenant_id TEXT NOT NULL)",
        )
        .await
        .unwrap();

        Migration.up(&SchemaManager::new(&db)).await.unwrap();

        assert!(table_exists(&SchemaManager::new(&db), "oauth_apps").await.unwrap());
        assert!(!table_exists(&SchemaManager::new(&db), "o_auth_apps").await.unwrap());
    }
}
