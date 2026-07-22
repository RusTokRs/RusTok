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
                )));
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

#[cfg(test)]
mod tests {
    use super::Migration;
    use sea_orm::{ConnectionTrait, Database, DbBackend, Statement, TryGetable};
    use sea_orm_migration::prelude::{MigrationTrait, SchemaManager};

    #[tokio::test]
    async fn backfills_legacy_approvals_into_individual_durable_batches() {
        let database = Database::connect("sqlite::memory:").await.unwrap();
        database
            .execute_unprepared(
                "CREATE TABLE ai_approval_requests (id TEXT NOT NULL, run_id TEXT NOT NULL)",
            )
            .await
            .unwrap();
        database
            .execute_unprepared(
                "INSERT INTO ai_approval_requests (id, run_id) VALUES ('approval-1', 'run-1')",
            )
            .await
            .unwrap();

        Migration.up(&SchemaManager::new(&database)).await.unwrap();
        let row = database
            .query_one(Statement::from_string(
                DbBackend::Sqlite,
                "SELECT approval_batch_id FROM ai_approval_requests".to_string(),
            ))
            .await
            .unwrap()
            .unwrap();
        assert_eq!(
            String::try_get(&row, "", "approval_batch_id").unwrap(),
            "run-1:approval-1"
        );
        let column = database
            .query_one(Statement::from_string(
                DbBackend::Sqlite,
                "SELECT \"notnull\" FROM pragma_table_info('ai_approval_requests') WHERE name = 'approval_batch_id'"
                    .to_string(),
            ))
            .await
            .unwrap()
            .unwrap();
        assert_eq!(i64::try_get(&column, "", "notnull").unwrap(), 1);
    }
}
