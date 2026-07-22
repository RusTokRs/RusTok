use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::DatabaseBackend;

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
            DatabaseBackend::Sqlite => manager
                .get_connection()
                .execute_unprepared(SQLITE_UP)
                .await
                .map(|_| ()),
            backend => Err(DbErr::Custom(format!(
                "notification source inbox does not support database backend {backend:?}"
            ))),
        }
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .get_connection()
            .execute_unprepared(
                r#"
                DROP TABLE IF EXISTS notification_source_inbox;
                "#,
            )
            .await
            .map(|_| ())
    }
}

const POSTGRES_UP: &str = r#"
CREATE TABLE IF NOT EXISTS notification_source_inbox (
    id UUID PRIMARY KEY,
    tenant_id UUID NOT NULL,
    source_slug VARCHAR(64) NOT NULL,
    source_event_id UUID NOT NULL,
    source_revision BIGINT NOT NULL,
    event_type VARCHAR(128) NOT NULL,
    status VARCHAR(24) NOT NULL DEFAULT 'pending',
    attempt_count INTEGER NOT NULL DEFAULT 0,
    next_attempt_at TIMESTAMPTZ NULL,
    lease_owner VARCHAR(191) NULL,
    lease_expires_at TIMESTAMPTZ NULL,
    fanout_job_id UUID NULL,
    last_error_code VARCHAR(100) NULL,
    last_error_message VARCHAR(2000) NULL,
    completed_at TIMESTAMPTZ NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    CONSTRAINT fk_notification_source_inbox_tenant FOREIGN KEY (tenant_id)
        REFERENCES tenants(id) ON DELETE CASCADE,
    CONSTRAINT fk_notification_source_inbox_job FOREIGN KEY (tenant_id, fanout_job_id)
        REFERENCES notification_fanout_jobs(tenant_id, id) ON DELETE RESTRICT,
    CONSTRAINT ck_notification_source_inbox_identity CHECK (
        source_revision > 0
        AND btrim(source_slug) <> ''
        AND btrim(event_type) <> ''
    ),
    CONSTRAINT ck_notification_source_inbox_status CHECK (
        status IN ('pending', 'processing', 'completed', 'suppressed', 'retryable_error', 'rejected')
    ),
    CONSTRAINT ck_notification_source_inbox_attempt CHECK (attempt_count >= 0),
    CONSTRAINT ck_notification_source_inbox_lease CHECK (
        (status = 'processing' AND lease_owner IS NOT NULL AND btrim(lease_owner) <> '' AND lease_expires_at IS NOT NULL)
        OR (status <> 'processing' AND lease_owner IS NULL AND lease_expires_at IS NULL)
    ),
    CONSTRAINT ck_notification_source_inbox_completion CHECK (
        (status IN ('completed', 'suppressed', 'rejected') AND completed_at IS NOT NULL)
        OR (status NOT IN ('completed', 'suppressed', 'rejected') AND completed_at IS NULL)
    )
);

CREATE UNIQUE INDEX IF NOT EXISTS ux_notification_source_inbox_event
    ON notification_source_inbox (tenant_id, source_slug, source_event_id, event_type);
CREATE INDEX IF NOT EXISTS idx_notification_source_inbox_recovery
    ON notification_source_inbox (status, next_attempt_at, lease_expires_at, updated_at);
CREATE INDEX IF NOT EXISTS idx_notification_source_inbox_job
    ON notification_source_inbox (tenant_id, fanout_job_id);
"#;

const SQLITE_UP: &str = r#"
CREATE TABLE IF NOT EXISTS notification_source_inbox (
    id TEXT PRIMARY KEY NOT NULL,
    tenant_id TEXT NOT NULL,
    source_slug TEXT NOT NULL,
    source_event_id TEXT NOT NULL,
    source_revision INTEGER NOT NULL CHECK (source_revision > 0),
    event_type TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'pending'
        CHECK (status IN ('pending', 'processing', 'completed', 'suppressed', 'retryable_error', 'rejected')),
    attempt_count INTEGER NOT NULL DEFAULT 0 CHECK (attempt_count >= 0),
    next_attempt_at TEXT NULL,
    lease_owner TEXT NULL,
    lease_expires_at TEXT NULL,
    fanout_job_id TEXT NULL,
    last_error_code TEXT NULL,
    last_error_message TEXT NULL,
    completed_at TEXT NULL,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (tenant_id) REFERENCES tenants(id) ON DELETE CASCADE,
    FOREIGN KEY (tenant_id, fanout_job_id)
        REFERENCES notification_fanout_jobs(tenant_id, id) ON DELETE RESTRICT,
    CHECK (length(trim(source_slug)) BETWEEN 1 AND 64),
    CHECK (length(trim(event_type)) BETWEEN 1 AND 128),
    CHECK (last_error_code IS NULL OR length(last_error_code) <= 100),
    CHECK (last_error_message IS NULL OR length(last_error_message) <= 2000),
    CHECK (
        (status = 'processing' AND lease_owner IS NOT NULL AND length(trim(lease_owner)) BETWEEN 1 AND 191 AND lease_expires_at IS NOT NULL)
        OR (status <> 'processing' AND lease_owner IS NULL AND lease_expires_at IS NULL)
    ),
    CHECK (
        (status IN ('completed', 'suppressed', 'rejected') AND completed_at IS NOT NULL)
        OR (status NOT IN ('completed', 'suppressed', 'rejected') AND completed_at IS NULL)
    )
);

CREATE UNIQUE INDEX IF NOT EXISTS ux_notification_source_inbox_event
    ON notification_source_inbox (tenant_id, source_slug, source_event_id, event_type);
CREATE INDEX IF NOT EXISTS idx_notification_source_inbox_recovery
    ON notification_source_inbox (status, next_attempt_at, lease_expires_at, updated_at);
CREATE INDEX IF NOT EXISTS idx_notification_source_inbox_job
    ON notification_source_inbox (tenant_id, fanout_job_id);
"#;
