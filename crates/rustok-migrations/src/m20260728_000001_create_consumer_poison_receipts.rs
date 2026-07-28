use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::DatabaseBackend;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let sql = match manager.get_database_backend() {
            DatabaseBackend::Postgres => POSTGRES_UP,
            DatabaseBackend::Sqlite => SQLITE_UP,
            backend => {
                return Err(DbErr::Custom(format!(
                    "Consumer poison receipts do not support database backend {backend:?}"
                )));
            }
        };
        manager
            .get_connection()
            .execute_unprepared(sql)
            .await
            .map(|_| ())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .get_connection()
            .execute_unprepared("DROP TABLE IF EXISTS sys_consumer_poison_receipts;")
            .await
            .map(|_| ())
    }
}

const POSTGRES_UP: &str = r#"
CREATE TABLE IF NOT EXISTS sys_consumer_poison_receipts (
    delivery_id UUID PRIMARY KEY,
    consumer_group VARCHAR(191) NOT NULL,
    source_stream VARCHAR(191) NOT NULL,
    source_topic VARCHAR(191) NOT NULL,
    source_partition INTEGER NOT NULL,
    source_offset BIGINT NOT NULL,
    payload BYTEA NOT NULL,
    stable_error_code VARCHAR(191) NOT NULL,
    delivery_attempt_count INTEGER NOT NULL,
    state VARCHAR(16) NOT NULL DEFAULT 'reserved',
    publisher_id UUID,
    lease_expires_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    published_at TIMESTAMPTZ,
    acknowledged_at TIMESTAMPTZ,
    CONSTRAINT uq_sys_consumer_poison_source UNIQUE (
        consumer_group,
        source_stream,
        source_topic,
        source_partition,
        source_offset
    ),
    CONSTRAINT ck_sys_consumer_poison_group CHECK (
        length(consumer_group) BETWEEN 1 AND 191
    ),
    CONSTRAINT ck_sys_consumer_poison_stream CHECK (
        length(source_stream) BETWEEN 1 AND 191
    ),
    CONSTRAINT ck_sys_consumer_poison_topic CHECK (
        length(source_topic) BETWEEN 1 AND 191
    ),
    CONSTRAINT ck_sys_consumer_poison_partition CHECK (source_partition > 0),
    CONSTRAINT ck_sys_consumer_poison_offset CHECK (source_offset >= 0),
    CONSTRAINT ck_sys_consumer_poison_payload CHECK (octet_length(payload) > 0),
    CONSTRAINT ck_sys_consumer_poison_error CHECK (
        length(stable_error_code) BETWEEN 1 AND 191
    ),
    CONSTRAINT ck_sys_consumer_poison_attempts CHECK (delivery_attempt_count > 0),
    CONSTRAINT ck_sys_consumer_poison_state CHECK (
        state IN ('reserved', 'publishing', 'published', 'acknowledged')
    ),
    CONSTRAINT ck_sys_consumer_poison_lease CHECK (
        (state = 'publishing' AND publisher_id IS NOT NULL AND lease_expires_at IS NOT NULL)
        OR
        (state <> 'publishing' AND publisher_id IS NULL AND lease_expires_at IS NULL)
    ),
    CONSTRAINT ck_sys_consumer_poison_publish_time CHECK (
        (state IN ('reserved', 'publishing') AND published_at IS NULL AND acknowledged_at IS NULL)
        OR
        (state = 'published' AND published_at IS NOT NULL AND acknowledged_at IS NULL)
        OR
        (state = 'acknowledged' AND published_at IS NOT NULL AND acknowledged_at IS NOT NULL)
    )
);

CREATE INDEX IF NOT EXISTS idx_sys_consumer_poison_state
    ON sys_consumer_poison_receipts (state, lease_expires_at, updated_at);
"#;

const SQLITE_UP: &str = r#"
CREATE TABLE IF NOT EXISTS sys_consumer_poison_receipts (
    delivery_id TEXT PRIMARY KEY,
    consumer_group TEXT NOT NULL,
    source_stream TEXT NOT NULL,
    source_topic TEXT NOT NULL,
    source_partition INTEGER NOT NULL,
    source_offset INTEGER NOT NULL,
    payload BLOB NOT NULL,
    stable_error_code TEXT NOT NULL,
    delivery_attempt_count INTEGER NOT NULL,
    state TEXT NOT NULL DEFAULT 'reserved',
    publisher_id TEXT,
    lease_expires_at TEXT,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    published_at TEXT,
    acknowledged_at TEXT,
    UNIQUE (
        consumer_group,
        source_stream,
        source_topic,
        source_partition,
        source_offset
    ),
    CHECK (length(consumer_group) BETWEEN 1 AND 191),
    CHECK (length(source_stream) BETWEEN 1 AND 191),
    CHECK (length(source_topic) BETWEEN 1 AND 191),
    CHECK (source_partition > 0),
    CHECK (source_offset >= 0),
    CHECK (length(payload) > 0),
    CHECK (length(stable_error_code) BETWEEN 1 AND 191),
    CHECK (delivery_attempt_count > 0),
    CHECK (state IN ('reserved', 'publishing', 'published', 'acknowledged')),
    CHECK (
        (state = 'publishing' AND publisher_id IS NOT NULL AND lease_expires_at IS NOT NULL)
        OR
        (state <> 'publishing' AND publisher_id IS NULL AND lease_expires_at IS NULL)
    ),
    CHECK (
        (state IN ('reserved', 'publishing') AND published_at IS NULL AND acknowledged_at IS NULL)
        OR
        (state = 'published' AND published_at IS NOT NULL AND acknowledged_at IS NULL)
        OR
        (state = 'acknowledged' AND published_at IS NOT NULL AND acknowledged_at IS NOT NULL)
    )
);

CREATE INDEX IF NOT EXISTS idx_sys_consumer_poison_state
    ON sys_consumer_poison_receipts (state, lease_expires_at, updated_at);
"#;
