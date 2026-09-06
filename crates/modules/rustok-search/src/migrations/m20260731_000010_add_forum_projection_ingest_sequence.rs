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
                "Forum projection ingest sequence does not support database backend {backend:?}"
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
                "Forum projection ingest sequence does not support database backend {backend:?}"
            ))),
        }
    }
}

const POSTGRES_UP: &str = r#"
CREATE SEQUENCE IF NOT EXISTS search_projection_inbox_ingest_sequence_seq AS BIGINT;

ALTER TABLE search_projection_inbox
    ADD COLUMN IF NOT EXISTS ingest_sequence BIGINT NULL;

WITH base AS (
    SELECT COALESCE(MAX(ingest_sequence), 0) AS current_max
    FROM search_projection_inbox
), ordered AS (
    SELECT
        event_id,
        ROW_NUMBER() OVER (
            ORDER BY created_at ASC, revision_at ASC, event_id ASC
        ) AS row_number
    FROM search_projection_inbox
    WHERE ingest_sequence IS NULL
)
UPDATE search_projection_inbox inbox
SET ingest_sequence = base.current_max + ordered.row_number
FROM base, ordered
WHERE inbox.event_id = ordered.event_id;

SELECT setval(
    'search_projection_inbox_ingest_sequence_seq',
    GREATEST(
        COALESCE((SELECT MAX(ingest_sequence) FROM search_projection_inbox), 0) + 1,
        1
    ),
    false
);

ALTER TABLE search_projection_inbox
    ALTER COLUMN ingest_sequence
        SET DEFAULT nextval('search_projection_inbox_ingest_sequence_seq'::regclass),
    ALTER COLUMN ingest_sequence SET NOT NULL;

ALTER SEQUENCE search_projection_inbox_ingest_sequence_seq
    OWNED BY search_projection_inbox.ingest_sequence;

DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1
        FROM pg_constraint
        WHERE conname = 'ck_search_projection_inbox_ingest_sequence'
    ) THEN
        ALTER TABLE search_projection_inbox
            ADD CONSTRAINT ck_search_projection_inbox_ingest_sequence
            CHECK (ingest_sequence > 0);
    END IF;
END
$$;

CREATE UNIQUE INDEX IF NOT EXISTS ux_search_projection_inbox_ingest_sequence
    ON search_projection_inbox (ingest_sequence);

ALTER TABLE search_projection_watermarks
    ADD COLUMN IF NOT EXISTS ingest_sequence BIGINT NOT NULL DEFAULT 0;

DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1
        FROM pg_constraint
        WHERE conname = 'ck_search_projection_watermark_ingest_sequence'
    ) THEN
        ALTER TABLE search_projection_watermarks
            ADD CONSTRAINT ck_search_projection_watermark_ingest_sequence
            CHECK (ingest_sequence >= 0);
    END IF;
END
$$;
"#;

const POSTGRES_DOWN: &str = r#"
ALTER TABLE search_projection_watermarks
    DROP CONSTRAINT IF EXISTS ck_search_projection_watermark_ingest_sequence,
    DROP COLUMN IF EXISTS ingest_sequence;

DROP INDEX IF EXISTS ux_search_projection_inbox_ingest_sequence;

ALTER TABLE search_projection_inbox
    DROP CONSTRAINT IF EXISTS ck_search_projection_inbox_ingest_sequence,
    DROP COLUMN IF EXISTS ingest_sequence;

DROP SEQUENCE IF EXISTS search_projection_inbox_ingest_sequence_seq;
"#;
