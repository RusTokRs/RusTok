use sea_orm::{DatabaseBackend, Statement};
use sea_orm_migration::prelude::*;

const SNAPSHOT_TABLES: [&str; 3] = [
    "forum_topic_delete_snapshots",
    "forum_topic_reply_delete_snapshots",
    "forum_reply_delete_snapshots",
];

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    fn use_transaction(&self) -> Option<bool> {
        Some(true)
    }

    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let backend = manager.get_database_backend();
        if !matches!(backend, DatabaseBackend::Postgres | DatabaseBackend::Sqlite) {
            return Err(DbErr::Custom(format!(
                "delete snapshot reasons do not support {backend:?}"
            )));
        }

        // Predecessor snapshots have no recorded provenance.
        for table in SNAPSHOT_TABLES {
            manager
                .get_connection()
                .execute_unprepared(&format!(
                    "ALTER TABLE {table} ADD COLUMN reason TEXT CHECK (reason IN ('user_request', 'moderation', 'system'))"
                ))
                .await?;
        }
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();
        let backend = manager.get_database_backend();
        match backend {
            DatabaseBackend::Postgres => {
                db.execute_unprepared(
                    "LOCK TABLE forum_topic_delete_snapshots, forum_topic_reply_delete_snapshots, forum_reply_delete_snapshots IN ACCESS EXCLUSIVE MODE",
                )
                .await?;
            }
            DatabaseBackend::Sqlite => {}
            backend => {
                return Err(DbErr::Custom(format!(
                    "delete snapshot reason rollback does not support {backend:?}"
                )));
            }
        }

        for table in SNAPSHOT_TABLES {
            let statement = Statement::from_string(
                backend,
                format!("SELECT 1 FROM {table} WHERE reason IS NOT NULL LIMIT 1"),
            );
            if db.query_one_raw(statement).await?.is_some() {
                return Err(DbErr::Custom(format!(
                    "cannot remove recorded deletion reasons from {table}"
                )));
            }
        }
        for table in SNAPSHOT_TABLES {
            db.execute_unprepared(&format!("ALTER TABLE {table} DROP COLUMN reason"))
                .await?;
        }
        Ok(())
    }
}
