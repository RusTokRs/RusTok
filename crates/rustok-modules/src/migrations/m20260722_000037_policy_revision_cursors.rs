use sea_orm::{ConnectionTrait, DbBackend, Statement};
use sea_orm_migration::prelude::*;

/// Persists one predecessor-bound policy cursor per tenant and outbox
/// consumer. This is consumer state, not a second event journal.
#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let statements: &[&str] = match manager.get_database_backend() {
            DbBackend::Postgres => &[
                "CREATE TABLE module_policy_revision_cursors (\
                    tenant_id UUID NOT NULL,\
                    consumer_key TEXT NOT NULL CHECK (length(trim(consumer_key)) BETWEEN 1 AND 128),\
                    current_revision TEXT NULL CHECK (current_revision IS NULL OR length(current_revision) = 71),\
                    updated_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,\
                    PRIMARY KEY (tenant_id, consumer_key)\
                )",
                "ALTER TABLE module_policy_revision_cursors ENABLE ROW LEVEL SECURITY",
                "CREATE POLICY module_policy_revision_cursors_scope \
                 ON module_policy_revision_cursors USING (tenant_id::text = current_setting('rustok.tenant_id', true)) \
                 WITH CHECK (tenant_id::text = current_setting('rustok.tenant_id', true))",
            ],
            DbBackend::Sqlite => &["CREATE TABLE module_policy_revision_cursors (\
                    tenant_id TEXT NOT NULL,\
                    consumer_key TEXT NOT NULL CHECK (length(trim(consumer_key)) BETWEEN 1 AND 128),\
                    current_revision TEXT NULL CHECK (current_revision IS NULL OR length(current_revision) = 71),\
                    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,\
                    PRIMARY KEY (tenant_id, consumer_key)\
                )"],
            backend => {
                return Err(DbErr::Migration(format!(
                    "policy revision cursor migration does not support database backend {backend:?}"
                )));
            }
        };
        for statement in statements {
            manager
                .get_connection()
                .execute(Statement::from_string(
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
            .execute_unprepared("DROP TABLE module_policy_revision_cursors")
            .await
            .map(|_| ())
    }
}
