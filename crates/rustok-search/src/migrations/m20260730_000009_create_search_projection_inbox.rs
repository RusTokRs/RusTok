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
                "search projection inbox does not support database backend {backend:?}"
            ))),
        }
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .get_connection()
            .execute_unprepared(
                r#"
                DROP TABLE IF EXISTS search_projection_watermarks;
                DROP TABLE IF EXISTS search_projection_inbox;
                "#,
            )
            .await
            .map(|_| ())
    }
}

const POSTGRES_UP: &str = r#"
CREATE TABLE IF NOT EXISTS search_projection_inbox (
    event_id UUID PRIMARY KEY,
    tenant_id UUID NOT NULL,
    source_module VARCHAR(64) NOT NULL,
    scope_key VARCHAR(191) NOT NULL,
    event_type VARCHAR(128) NOT NULL,
    revision_at TIMESTAMPTZ NOT NULL,
    envelope_json JSONB NOT NULL,
    status VARCHAR(24) NOT NULL DEFAULT 'pending',
    attempt_count INTEGER NOT NULL DEFAULT 0,
    next_attempt_at TIMESTAMPTZ NULL,
    last_error VARCHAR(2000) NULL,
    completed_at TIMESTAMPTZ NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    CONSTRAINT ck_search_projection_inbox_identity CHECK (
        btrim(source_module) <> '' AND btrim(scope_key) <> '' AND btrim(event_type) <> ''
    ),
    CONSTRAINT ck_search_projection_inbox_status CHECK (
        status IN ('pending', 'processing', 'completed', 'skipped', 'retryable_error', 'dead_letter')
    ),
    CONSTRAINT ck_search_projection_inbox_attempt CHECK (attempt_count >= 0),
    CONSTRAINT ck_search_projection_inbox_completion CHECK (
        (status IN ('completed', 'skipped', 'dead_letter') AND completed_at IS NOT NULL)
        OR (status NOT IN ('completed', 'skipped', 'dead_letter') AND completed_at IS NULL)
    ),
    CONSTRAINT ck_search_projection_inbox_payload CHECK (
        jsonb_typeof(envelope_json) = 'object'
        AND octet_length(envelope_json::text) <= 65536
    )
);

CREATE INDEX IF NOT EXISTS idx_search_projection_inbox_due
    ON search_projection_inbox (
        tenant_id, source_module, status, revision_at, event_id, next_attempt_at
    );
CREATE INDEX IF NOT EXISTS idx_search_projection_inbox_scope
    ON search_projection_inbox (
        tenant_id, source_module, scope_key, revision_at, event_id
    );

CREATE TABLE IF NOT EXISTS search_projection_watermarks (
    tenant_id UUID NOT NULL,
    source_module VARCHAR(64) NOT NULL,
    scope_key VARCHAR(191) NOT NULL,
    revision_at TIMESTAMPTZ NOT NULL,
    event_id UUID NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    PRIMARY KEY (tenant_id, source_module, scope_key),
    CONSTRAINT ck_search_projection_watermark_identity CHECK (
        btrim(source_module) <> '' AND btrim(scope_key) <> ''
    )
);
"#;

const SQLITE_UP: &str = r#"
CREATE TABLE IF NOT EXISTS search_projection_inbox (
    event_id TEXT PRIMARY KEY,
    tenant_id TEXT NOT NULL,
    source_module TEXT NOT NULL,
    scope_key TEXT NOT NULL,
    event_type TEXT NOT NULL,
    revision_at TEXT NOT NULL,
    envelope_json TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'pending',
    attempt_count INTEGER NOT NULL DEFAULT 0,
    next_attempt_at TEXT NULL,
    last_error TEXT NULL,
    completed_at TEXT NULL,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    CHECK (length(trim(source_module)) > 0),
    CHECK (length(trim(scope_key)) > 0),
    CHECK (length(trim(event_type)) > 0),
    CHECK (status IN ('pending', 'processing', 'completed', 'skipped', 'retryable_error', 'dead_letter')),
    CHECK (attempt_count >= 0),
    CHECK (
        (status IN ('completed', 'skipped', 'dead_letter') AND completed_at IS NOT NULL)
        OR (status NOT IN ('completed', 'skipped', 'dead_letter') AND completed_at IS NULL)
    ),
    CHECK (json_valid(envelope_json) AND length(envelope_json) <= 65536)
);

CREATE INDEX IF NOT EXISTS idx_search_projection_inbox_due
    ON search_projection_inbox (
        tenant_id, source_module, status, revision_at, event_id, next_attempt_at
    );
CREATE INDEX IF NOT EXISTS idx_search_projection_inbox_scope
    ON search_projection_inbox (
        tenant_id, source_module, scope_key, revision_at, event_id
    );

CREATE TABLE IF NOT EXISTS search_projection_watermarks (
    tenant_id TEXT NOT NULL,
    source_module TEXT NOT NULL,
    scope_key TEXT NOT NULL,
    revision_at TEXT NOT NULL,
    event_id TEXT NOT NULL,
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    PRIMARY KEY (tenant_id, source_module, scope_key),
    CHECK (length(trim(source_module)) > 0),
    CHECK (length(trim(scope_key)) > 0)
);
"#;
