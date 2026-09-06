use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::{ConnectionTrait, DatabaseBackend};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        match manager.get_database_backend() {
            DatabaseBackend::Postgres => manager
                .get_connection()
                .execute_unprepared(POSTGRES_UP)
                .await
                .map(|_| ()),
            DatabaseBackend::Sqlite => Ok(()),
            backend => Err(DbErr::Custom(format!(
                "Forum projection ingest sequence lookup does not support database backend {backend:?}"
            ))),
        }
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        match manager.get_database_backend() {
            DatabaseBackend::Postgres => manager
                .get_connection()
                .execute_unprepared(POSTGRES_DOWN)
                .await
                .map(|_| ()),
            DatabaseBackend::Sqlite => Ok(()),
            backend => Err(DbErr::Custom(format!(
                "Forum projection ingest sequence lookup does not support database backend {backend:?}"
            ))),
        }
    }
}

const POSTGRES_UP: &str = r#"
DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1
        FROM pg_constraint
        WHERE conname = 'ck_search_projection_inbox_ingest_sequence'
          AND conrelid = 'search_projection_inbox'::regclass
    ) THEN
        ALTER TABLE search_projection_inbox
            ADD CONSTRAINT ck_search_projection_inbox_ingest_sequence
            CHECK (ingest_sequence > 0);
    END IF;
END
$$;

DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1
        FROM pg_constraint
        WHERE conname = 'ck_search_projection_watermark_ingest_sequence'
          AND conrelid = 'search_projection_watermarks'::regclass
    ) THEN
        ALTER TABLE search_projection_watermarks
            ADD CONSTRAINT ck_search_projection_watermark_ingest_sequence
            CHECK (ingest_sequence >= 0);
    END IF;
END
$$;

CREATE INDEX IF NOT EXISTS idx_search_projection_inbox_due_ingest_sequence
    ON search_projection_inbox (source_module, tenant_id, ingest_sequence)
    WHERE status IN ('pending', 'retryable_error');
"#;

const POSTGRES_DOWN: &str = r#"
DROP INDEX IF EXISTS idx_search_projection_inbox_due_ingest_sequence;
"#;
