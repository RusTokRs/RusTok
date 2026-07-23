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
                "notification outbox intake does not support database backend {backend:?}"
            ))),
        }
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .get_connection()
            .execute_unprepared(
                r#"
                DROP TABLE IF EXISTS notification_outbox_intake_receipts;
                DROP INDEX IF EXISTS ux_notification_source_inbox_tenant_id;
                "#,
            )
            .await
            .map(|_| ())
    }
}

const POSTGRES_UP: &str = r#"
CREATE UNIQUE INDEX IF NOT EXISTS ux_notification_source_inbox_tenant_id
    ON notification_source_inbox (tenant_id, id);

CREATE TABLE IF NOT EXISTS notification_outbox_intake_receipts (
    outbox_event_id UUID PRIMARY KEY,
    tenant_id UUID NOT NULL,
    event_type VARCHAR(128) NOT NULL,
    source_slug VARCHAR(64) NOT NULL,
    source_event_id UUID NOT NULL,
    source_revision BIGINT NOT NULL,
    source_inbox_id UUID NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    CONSTRAINT fk_notification_outbox_intake_tenant FOREIGN KEY (tenant_id)
        REFERENCES tenants(id) ON DELETE CASCADE,
    CONSTRAINT fk_notification_outbox_intake_source FOREIGN KEY (tenant_id, source_inbox_id)
        REFERENCES notification_source_inbox(tenant_id, id) ON DELETE CASCADE,
    CONSTRAINT ck_notification_outbox_intake_identity CHECK (
        source_revision > 0
        AND btrim(event_type) <> ''
        AND btrim(source_slug) <> ''
        AND source_event_id = outbox_event_id
    )
);

CREATE INDEX IF NOT EXISTS idx_notification_outbox_intake_source
    ON notification_outbox_intake_receipts (tenant_id, source_slug, source_event_id);
"#;

const SQLITE_UP: &str = r#"
CREATE UNIQUE INDEX IF NOT EXISTS ux_notification_source_inbox_tenant_id
    ON notification_source_inbox (tenant_id, id);

CREATE TABLE IF NOT EXISTS notification_outbox_intake_receipts (
    outbox_event_id TEXT PRIMARY KEY NOT NULL,
    tenant_id TEXT NOT NULL,
    event_type TEXT NOT NULL,
    source_slug TEXT NOT NULL,
    source_event_id TEXT NOT NULL,
    source_revision INTEGER NOT NULL CHECK (source_revision > 0),
    source_inbox_id TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (tenant_id) REFERENCES tenants(id) ON DELETE CASCADE,
    FOREIGN KEY (tenant_id, source_inbox_id)
        REFERENCES notification_source_inbox(tenant_id, id) ON DELETE CASCADE,
    CHECK (length(trim(event_type)) BETWEEN 1 AND 128),
    CHECK (length(trim(source_slug)) BETWEEN 1 AND 64),
    CHECK (source_event_id = outbox_event_id)
);

CREATE INDEX IF NOT EXISTS idx_notification_outbox_intake_source
    ON notification_outbox_intake_receipts (tenant_id, source_slug, source_event_id);
"#;
