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
        match manager.get_database_backend() {
            DatabaseBackend::Postgres => manager
                .get_connection()
                .execute_unprepared(POSTGRES_DOWN)
                .await
                .map(|_| ()),
            DatabaseBackend::Sqlite => manager
                .get_connection()
                .execute_unprepared(SQLITE_DOWN)
                .await
                .map(|_| ()),
            backend => Err(DbErr::Custom(format!(
                "notification outbox intake rejection storage does not support database backend {backend:?}"
            ))),
        }
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

CREATE OR REPLACE FUNCTION notification_outbox_intake_receipt_terminal_guard()
RETURNS TRIGGER AS $$
BEGIN
    PERFORM pg_advisory_xact_lock(hashtextextended(NEW.outbox_event_id::text, 0));
    IF EXISTS (
        SELECT 1 FROM notification_outbox_intake_rejections
        WHERE outbox_event_id = NEW.outbox_event_id
    ) THEN
        RAISE EXCEPTION 'notification outbox event already has a rejection outcome';
    END IF;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE OR REPLACE FUNCTION notification_outbox_intake_rejection_terminal_guard()
RETURNS TRIGGER AS $$
BEGIN
    PERFORM pg_advisory_xact_lock(hashtextextended(NEW.outbox_event_id::text, 0));
    IF EXISTS (
        SELECT 1 FROM notification_outbox_intake_receipts
        WHERE outbox_event_id = NEW.outbox_event_id
    ) THEN
        RAISE EXCEPTION 'notification outbox event already has an accepted outcome';
    END IF;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS notification_outbox_intake_receipt_terminal_guard_insert
    ON notification_outbox_intake_receipts;
CREATE TRIGGER notification_outbox_intake_receipt_terminal_guard_insert
BEFORE INSERT ON notification_outbox_intake_receipts
FOR EACH ROW EXECUTE FUNCTION notification_outbox_intake_receipt_terminal_guard();

DROP TRIGGER IF EXISTS notification_outbox_intake_rejection_terminal_guard_insert
    ON notification_outbox_intake_rejections;
CREATE TRIGGER notification_outbox_intake_rejection_terminal_guard_insert
BEFORE INSERT ON notification_outbox_intake_rejections
FOR EACH ROW EXECUTE FUNCTION notification_outbox_intake_rejection_terminal_guard();
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

DROP TRIGGER IF EXISTS notification_outbox_intake_receipt_terminal_guard_insert;
CREATE TRIGGER notification_outbox_intake_receipt_terminal_guard_insert
BEFORE INSERT ON notification_outbox_intake_receipts
FOR EACH ROW
WHEN EXISTS (
    SELECT 1 FROM notification_outbox_intake_rejections
    WHERE outbox_event_id = NEW.outbox_event_id
)
BEGIN
    SELECT RAISE(ABORT, 'notification outbox event already has a rejection outcome');
END;

DROP TRIGGER IF EXISTS notification_outbox_intake_rejection_terminal_guard_insert;
CREATE TRIGGER notification_outbox_intake_rejection_terminal_guard_insert
BEFORE INSERT ON notification_outbox_intake_rejections
FOR EACH ROW
WHEN EXISTS (
    SELECT 1 FROM notification_outbox_intake_receipts
    WHERE outbox_event_id = NEW.outbox_event_id
)
BEGIN
    SELECT RAISE(ABORT, 'notification outbox event already has an accepted outcome');
END;
"#;

const POSTGRES_DOWN: &str = r#"
DROP TRIGGER IF EXISTS notification_outbox_intake_receipt_terminal_guard_insert
    ON notification_outbox_intake_receipts;
DROP TRIGGER IF EXISTS notification_outbox_intake_rejection_terminal_guard_insert
    ON notification_outbox_intake_rejections;
DROP FUNCTION IF EXISTS notification_outbox_intake_receipt_terminal_guard();
DROP FUNCTION IF EXISTS notification_outbox_intake_rejection_terminal_guard();
DROP TABLE IF EXISTS notification_outbox_intake_rejections;
"#;

const SQLITE_DOWN: &str = r#"
DROP TRIGGER IF EXISTS notification_outbox_intake_receipt_terminal_guard_insert;
DROP TRIGGER IF EXISTS notification_outbox_intake_rejection_terminal_guard_insert;
DROP TABLE IF EXISTS notification_outbox_intake_rejections;
"#;
