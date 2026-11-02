use sea_orm::{ConnectionTrait, Statement};
use sea_orm_migration::prelude::*;

use crate::backend::SupportedBackend;

/// Records only the canonical digest of the settings supplied to a committed
/// lifecycle hook. Older operations retain NULL and cannot be retried safely.
#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    fn use_transaction(&self) -> Option<bool> {
        Some(true)
    }

    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let backend = SupportedBackend::new(manager.get_database_backend())?;
        let constraint = match backend {
            SupportedBackend::Postgres => "hook_settings_digest ~ '^sha256:[0-9a-f]{64}$'",
            SupportedBackend::Sqlite => {
                "length(hook_settings_digest) = 71 AND substr(hook_settings_digest, 1, 7) = 'sha256:' AND substr(hook_settings_digest, 8) NOT GLOB '*[^0-9a-f]*'"
            }
        };
        manager.get_connection().execute_raw(Statement::from_string(
            backend.db_backend(),
            format!("ALTER TABLE module_operation_override_states ADD COLUMN hook_settings_digest TEXT CHECK (hook_settings_digest IS NULL OR ({constraint}))"),
        )).await?;
        manager.get_connection().execute_raw(Statement::from_string(
            backend.db_backend(),
            "ALTER TABLE module_operations ADD COLUMN recovery_source_id TEXT CHECK (recovery_source_id IS NULL OR length(recovery_source_id) = 36)".to_string(),
        )).await?;
        Ok(())
    }

    async fn down(&self, _: &SchemaManager) -> Result<(), DbErr> {
        Err(DbErr::Migration(
            "Lifecycle recovery evidence cannot be discarded by a downgrade".to_string(),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sea_orm::TransactionTrait;

    #[tokio::test]
    async fn sqlite_upgrade_preserves_legacy_rows_and_validates_evidence() {
        let db = sea_orm::Database::connect("sqlite::memory:").await.unwrap();
        db.execute_unprepared("CREATE TABLE module_operations (id TEXT PRIMARY KEY)")
            .await
            .unwrap();
        db.execute_unprepared(
            "CREATE TABLE module_operation_override_states (operation_id TEXT PRIMARY KEY)",
        )
        .await
        .unwrap();
        db.execute_unprepared("INSERT INTO module_operations VALUES ('old')")
            .await
            .unwrap();
        db.execute_unprepared("INSERT INTO module_operation_override_states VALUES ('old')")
            .await
            .unwrap();
        Migration.up(&SchemaManager::new(&db)).await.unwrap();
        let row = db.query_one_raw(Statement::from_string(sea_orm::DbBackend::Sqlite,
            "SELECT hook_settings_digest FROM module_operation_override_states WHERE operation_id = 'old'".to_string()))
            .await.unwrap().unwrap();
        assert_eq!(
            row.try_get::<Option<String>>("", "hook_settings_digest")
                .unwrap(),
            None
        );
        assert!(
            db.execute_unprepared(
                "UPDATE module_operation_override_states SET hook_settings_digest = 'invalid'"
            )
            .await
            .is_err()
        );
        let digest = format!("sha256:{}", "a".repeat(64));
        db.execute_raw(Statement::from_sql_and_values(
            sea_orm::DbBackend::Sqlite,
            "UPDATE module_operation_override_states SET hook_settings_digest = ?1",
            vec![digest.clone().into()],
        ))
        .await
        .unwrap();
        let row = db.query_one_raw(Statement::from_string(sea_orm::DbBackend::Sqlite,
            "SELECT hook_settings_digest FROM module_operation_override_states WHERE operation_id = 'old'".to_string())).await.unwrap().unwrap();
        assert_eq!(
            row.try_get::<String>("", "hook_settings_digest").unwrap(),
            digest
        );
        assert!(
            db.execute_unprepared("UPDATE module_operations SET recovery_source_id = 'invalid'")
                .await
                .is_err()
        );
        assert!(Migration.down(&SchemaManager::new(&db)).await.is_err());
    }

    #[tokio::test]
    async fn sqlite_upgrade_rolls_back_both_changes_on_failure() {
        let db = sea_orm::Database::connect("sqlite::memory:").await.unwrap();
        db.execute_unprepared(
            "CREATE TABLE module_operations (id TEXT PRIMARY KEY, recovery_source_id TEXT)",
        )
        .await
        .unwrap();
        db.execute_unprepared(
            "CREATE TABLE module_operation_override_states (operation_id TEXT PRIMARY KEY)",
        )
        .await
        .unwrap();
        assert_eq!(Migration.use_transaction(), Some(true));
        let transaction = db.begin().await.unwrap();
        assert!(
            Migration
                .up(&SchemaManager::new(&transaction))
                .await
                .is_err()
        );
        transaction.rollback().await.unwrap();
        let columns = db
            .query_all_raw(Statement::from_string(
                sea_orm::DbBackend::Sqlite,
                "PRAGMA table_info(module_operation_override_states)".to_string(),
            ))
            .await
            .unwrap();
        assert!(
            columns
                .iter()
                .all(|column| column.try_get::<String>("", "name").unwrap()
                    != "hook_settings_digest")
        );
    }
}
