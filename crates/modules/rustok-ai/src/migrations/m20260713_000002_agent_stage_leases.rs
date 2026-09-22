use sea_orm::{ConnectionTrait, DbBackend};
use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let connection = manager.get_connection();
        let statements: &[&str] = match manager.get_database_backend() {
            DbBackend::Postgres | DbBackend::Sqlite => &[
                "ALTER TABLE ai_agent_workflow_stages ADD COLUMN lease_token UUID NULL",
                "ALTER TABLE ai_agent_workflow_stages ADD COLUMN lease_expires_at TIMESTAMPTZ NULL",
                "ALTER TABLE ai_agent_workflow_stages ADD COLUMN attempt_count INTEGER NOT NULL DEFAULT 0",
                "CREATE INDEX ai_agent_workflow_stages_lease_idx ON ai_agent_workflow_stages (tenant_id, status, lease_expires_at)",
            ],
            other => {
                return Err(DbErr::Migration(format!(
                    "AI agent stage lease migration does not support database backend {other:?}"
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
            "AI agent stage lease migration is intentionally irreversible".to_string(),
        ))
    }
}
