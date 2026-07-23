use sea_orm::{ConnectionTrait, DbBackend, Statement};
use sea_orm_migration::prelude::*;

/// Persists the immutable approval command required to replay final registry
/// publication after the publish request transitions to `published`.
#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let statement = match manager.get_database_backend() {
            DbBackend::Postgres => {
                "CREATE TABLE registry_publication_operations (\
                operation_id UUID PRIMARY KEY,\
                request_id TEXT NOT NULL REFERENCES registry_publish_requests(id),\
                idempotency_key UUID NOT NULL,\
                actor_principal JSONB NOT NULL,\
                publisher_principal JSONB NOT NULL,\
                allow_owner_rebind BOOLEAN NOT NULL,\
                approval_override JSONB NULL,\
                release_id TEXT NOT NULL REFERENCES registry_module_releases(id),\
                committed_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,\
                UNIQUE (request_id, idempotency_key)\
            )"
            }
            DbBackend::Sqlite => {
                "CREATE TABLE registry_publication_operations (\
                operation_id TEXT PRIMARY KEY NOT NULL,\
                request_id TEXT NOT NULL REFERENCES registry_publish_requests(id),\
                idempotency_key TEXT NOT NULL,\
                actor_principal JSON NOT NULL,\
                publisher_principal JSON NOT NULL,\
                allow_owner_rebind INTEGER NOT NULL CHECK (allow_owner_rebind IN (0, 1)),\
                approval_override JSON NULL,\
                release_id TEXT NOT NULL REFERENCES registry_module_releases(id),\
                committed_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,\
                UNIQUE (request_id, idempotency_key)\
            )"
            }
            backend => {
                return Err(DbErr::Migration(format!(
                    "registry publication idempotency migration does not support database backend {backend:?}"
                )));
            }
        };
        manager
            .get_connection()
            .execute(Statement::from_string(
                manager.get_database_backend(),
                statement.to_string(),
            ))
            .await
            .map(|_| ())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .get_connection()
            .execute_unprepared("DROP TABLE registry_publication_operations")
            .await
            .map(|_| ())
    }
}
