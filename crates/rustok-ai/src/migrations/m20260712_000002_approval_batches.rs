use sea_orm::{ConnectionTrait, DbBackend};
use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let connection = manager.get_connection();
        let statements: &[&str] = match manager.get_database_backend() {
            DbBackend::Postgres => &[
                "ALTER TABLE ai_approval_requests ADD COLUMN approval_batch_id TEXT",
                "UPDATE ai_approval_requests SET approval_batch_id = run_id::text || ':' || id::text",
                "ALTER TABLE ai_approval_requests ALTER COLUMN approval_batch_id SET NOT NULL",
            ],
            DbBackend::Sqlite => &[
                "ALTER TABLE ai_approval_requests ADD COLUMN approval_batch_id TEXT NOT NULL DEFAULT ''",
                "UPDATE ai_approval_requests SET approval_batch_id = run_id || ':' || id",
            ],
            backend => {
                return Err(DbErr::Migration(format!(
                    "AI approval batch migration does not support database backend {backend:?}"
                )))
            }
        };
        for statement in statements {
            connection.execute_unprepared(statement).await?;
        }
        Ok(())
    }

    async fn down(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
        Err(DbErr::Migration(
            "approval batches are required for deterministic agent resumption".to_string(),
        ))
    }
}
