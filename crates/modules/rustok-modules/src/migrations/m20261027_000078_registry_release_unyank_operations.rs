use sea_orm::{ConnectionTrait, Statement};
use sea_orm_migration::prelude::*;

use crate::backend::SupportedBackend;

/// Adds the durable idempotency ledger for the separately authorized
/// release un-yank transition. Publication remains insert-only and never
/// restores a yanked release as a side effect.
#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let backend = SupportedBackend::new(manager.get_database_backend())?;
        let statement = match backend {
            SupportedBackend::Postgres => {
                "CREATE TABLE registry_release_unyank_operations (\
                    operation_id UUID PRIMARY KEY,\
                    release_id TEXT NOT NULL REFERENCES registry_module_releases(id),\
                    idempotency_key UUID NOT NULL,\
                    actor_id UUID NOT NULL,\
                    trace_id TEXT NOT NULL,\
                    correlation_id UUID NOT NULL,\
                    actor_principal JSONB NOT NULL,\
                    actor_can_manage_modules BOOLEAN NOT NULL,\
                    reason TEXT NOT NULL,\
                    reason_code TEXT NOT NULL,\
                    resulting_status TEXT NOT NULL CHECK (resulting_status = 'active'),\
                    committed_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,\
                    UNIQUE (release_id, idempotency_key)\
                )"
            }
            SupportedBackend::Sqlite => {
                "CREATE TABLE registry_release_unyank_operations (\
                    operation_id TEXT PRIMARY KEY NOT NULL,\
                    release_id TEXT NOT NULL REFERENCES registry_module_releases(id),\
                    idempotency_key TEXT NOT NULL,\
                    actor_id TEXT NOT NULL,\
                    trace_id TEXT NOT NULL,\
                    correlation_id TEXT NOT NULL,\
                    actor_principal JSON NOT NULL,\
                    actor_can_manage_modules INTEGER NOT NULL CHECK (actor_can_manage_modules IN (0, 1)),\
                    reason TEXT NOT NULL,\
                    reason_code TEXT NOT NULL,\
                    resulting_status TEXT NOT NULL CHECK (resulting_status = 'active'),\
                    committed_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,\
                    UNIQUE (release_id, idempotency_key)\
                )"
            }
        };
        manager
            .get_connection()
            .execute_raw(Statement::from_string(
                backend.db_backend(),
                statement.to_string(),
            ))
            .await?;
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .get_connection()
            .execute_unprepared("DROP TABLE registry_release_unyank_operations")
            .await
            .map(|_| ())
    }
}
