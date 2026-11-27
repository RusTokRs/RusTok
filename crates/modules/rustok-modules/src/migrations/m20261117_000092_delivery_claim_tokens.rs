use sea_orm::ConnectionTrait;
use sea_orm_migration::prelude::*;

use crate::backend::SupportedBackend;

/// Existing running rows retain a NULL token and cannot be completed by new
/// workers. Their leases expire and are reclaimed normally. Deploy only after
/// stopping old workers, which do not enforce this fence.
#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    fn use_transaction(&self) -> Option<bool> {
        Some(true)
    }

    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let sql_type = match SupportedBackend::new(manager.get_database_backend())? {
            SupportedBackend::Postgres => "UUID",
            SupportedBackend::Sqlite => "TEXT",
        };
        for table in [
            "module_artifact_event_deliveries",
            "module_artifact_schedule_deliveries",
        ] {
            manager
                .get_connection()
                .execute_unprepared(&format!(
                    "ALTER TABLE {table} ADD COLUMN claim_token {sql_type} NULL"
                ))
                .await?;
        }
        Ok(())
    }

    async fn down(&self, _: &SchemaManager) -> Result<(), DbErr> {
        Err(DbErr::Migration(
            "delivery claim fencing cannot be safely downgraded while workers or claims may exist"
                .into(),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sea_orm::{Database, DbBackend, Statement};

    #[tokio::test]
    async fn sqlite_legacy_running_rows_have_no_authorized_claim_token() {
        let db = Database::connect("sqlite::memory:").await.unwrap();
        for table in [
            "module_artifact_event_deliveries",
            "module_artifact_schedule_deliveries",
        ] {
            db.execute_unprepared(&format!("CREATE TABLE {table} (status TEXT NOT NULL)"))
                .await
                .unwrap();
            db.execute_unprepared(&format!("INSERT INTO {table} VALUES ('running')"))
                .await
                .unwrap();
        }
        Migration.up(&SchemaManager::new(&db)).await.unwrap();
        for table in [
            "module_artifact_event_deliveries",
            "module_artifact_schedule_deliveries",
        ] {
            let row = db
                .query_one_raw(Statement::from_string(
                    DbBackend::Sqlite,
                    format!("SELECT claim_token FROM {table}"),
                ))
                .await
                .unwrap()
                .unwrap();
            assert_eq!(
                row.try_get::<Option<String>>("", "claim_token").unwrap(),
                None
            );
            let result = db.execute_unprepared(&format!("UPDATE {table} SET status = 'succeeded' WHERE status = 'running' AND claim_token = 'new-worker-token'")).await.unwrap();
            assert_eq!(result.rows_affected(), 0);
        }
    }
}
