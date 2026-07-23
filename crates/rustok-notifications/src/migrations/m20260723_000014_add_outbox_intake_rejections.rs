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
                "notification outbox intake rejection storage does not support database backend {backend:?}"
            ))),
        }
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .get_connection()
            .execute_unprepared("DROP TABLE IF EXISTS notification_outbox_intake_rejections;")
            .await
            .map(|_| ())
    }
}

const POSTGRES_UP: &str = r#"
CREATE TABLE IF NOT EXISTS notification_outbox_intake_rejections (
    outbox_event_id UUID PRIMARY KEY,
    event_type VARCHAR(128) NOT NULL,
    schema_version SMALLINT NOT NULL,
    error_code VARCHAR(100) NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    CONSTRAINT fk_notification_outbox_intake_rejection_event FOREIGN KEY (outbox_event_id)
        REFERENCES sys_events(id) ON DELETE CASCADE,
    CONSTRAINT ck_notification_outbox_intake_rejection_error CHECK (
        btrim(error_code) <> ''
    )
);

CREATE INDEX IF NOT EXISTS idx_notification_outbox_intake_rejection_created
    ON notification_outbox_intake_rejections (created_at, outbox_event_id);
"#;

const SQLITE_UP: &str = r#"
CREATE TABLE IF NOT EXISTS notification_outbox_intake_rejections (
    outbox_event_id TEXT PRIMARY KEY NOT NULL,
    event_type TEXT NOT NULL,
    schema_version INTEGER NOT NULL,
    error_code TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (outbox_event_id) REFERENCES sys_events(id) ON DELETE CASCADE,
    CHECK (length(event_type) <= 128),
    CHECK (length(trim(error_code)) BETWEEN 1 AND 100)
);

CREATE INDEX IF NOT EXISTS idx_notification_outbox_intake_rejection_created
    ON notification_outbox_intake_rejections (created_at, outbox_event_id);
"#;
