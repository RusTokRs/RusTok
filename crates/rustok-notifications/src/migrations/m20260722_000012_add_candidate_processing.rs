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
                "notification candidate processing does not support database backend {backend:?}"
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
                "notification candidate processing does not support database backend {backend:?}"
            ))),
        }
    }
}

const POSTGRES_UP: &str = r#"
ALTER TABLE notification_fanout_items
    ADD COLUMN IF NOT EXISTS attempt_count INTEGER NOT NULL DEFAULT 0,
    ADD COLUMN IF NOT EXISTS next_attempt_at TIMESTAMPTZ NULL,
    ADD COLUMN IF NOT EXISTS lease_owner VARCHAR(191) NULL,
    ADD COLUMN IF NOT EXISTS lease_expires_at TIMESTAMPTZ NULL;

ALTER TABLE notification_fanout_items
    DROP CONSTRAINT IF EXISTS ck_notification_fanout_item_status,
    DROP CONSTRAINT IF EXISTS ck_notification_fanout_item_completion,
    DROP CONSTRAINT IF EXISTS ck_notification_fanout_item_attempt,
    DROP CONSTRAINT IF EXISTS ck_notification_fanout_item_lease;

ALTER TABLE notification_fanout_items
    ADD CONSTRAINT ck_notification_fanout_item_status CHECK (
        status IN ('pending', 'processing', 'processed', 'skipped', 'retryable_error', 'failed')
    ),
    ADD CONSTRAINT ck_notification_fanout_item_attempt CHECK (attempt_count >= 0),
    ADD CONSTRAINT ck_notification_fanout_item_lease CHECK (
        (status = 'processing' AND lease_owner IS NOT NULL AND btrim(lease_owner) <> '' AND lease_expires_at IS NOT NULL)
        OR (status <> 'processing' AND lease_owner IS NULL AND lease_expires_at IS NULL)
    ),
    ADD CONSTRAINT ck_notification_fanout_item_completion CHECK (
        (status IN ('pending', 'retryable_error') AND processed_at IS NULL AND notification_id IS NULL)
        OR (status = 'processing' AND processed_at IS NULL AND notification_id IS NULL)
        OR (status = 'processed' AND processed_at IS NOT NULL AND notification_id IS NOT NULL)
        OR (status IN ('skipped', 'failed') AND processed_at IS NOT NULL AND notification_id IS NULL)
    );

CREATE INDEX IF NOT EXISTS idx_notification_fanout_item_recovery
    ON notification_fanout_items (status, next_attempt_at, lease_expires_at, updated_at);
"#;

const POSTGRES_DOWN: &str = r#"
UPDATE notification_fanout_items
SET status = 'failed',
    notification_id = NULL,
    processed_at = COALESCE(processed_at, CURRENT_TIMESTAMP),
    last_error_code = COALESCE(last_error_code, 'NOTIFICATION_CANDIDATE_DOWNGRADE'),
    lease_owner = NULL,
    lease_expires_at = NULL,
    next_attempt_at = NULL
WHERE status IN ('processing', 'retryable_error');

DROP INDEX IF EXISTS idx_notification_fanout_item_recovery;

ALTER TABLE notification_fanout_items
    DROP CONSTRAINT IF EXISTS ck_notification_fanout_item_status,
    DROP CONSTRAINT IF EXISTS ck_notification_fanout_item_completion,
    DROP CONSTRAINT IF EXISTS ck_notification_fanout_item_attempt,
    DROP CONSTRAINT IF EXISTS ck_notification_fanout_item_lease;

ALTER TABLE notification_fanout_items
    ADD CONSTRAINT ck_notification_fanout_item_status CHECK (
        status IN ('pending', 'processed', 'skipped', 'failed')
    ),
    ADD CONSTRAINT ck_notification_fanout_item_completion CHECK (
        (status = 'pending' AND processed_at IS NULL AND notification_id IS NULL)
        OR (status = 'processed' AND processed_at IS NOT NULL AND notification_id IS NOT NULL)
        OR (status IN ('skipped', 'failed') AND processed_at IS NOT NULL)
    );

ALTER TABLE notification_fanout_items
    DROP COLUMN IF EXISTS attempt_count,
    DROP COLUMN IF EXISTS next_attempt_at,
    DROP COLUMN IF EXISTS lease_owner,
    DROP COLUMN IF EXISTS lease_expires_at;
"#;

const SQLITE_UP: &str = r#"
DROP TRIGGER IF EXISTS notification_fanout_item_tenant_guard_insert;
DROP TRIGGER IF EXISTS notification_fanout_item_tenant_guard_update;

ALTER TABLE notification_fanout_items RENAME TO notification_fanout_items_before_candidate_processing;

CREATE TABLE notification_fanout_items (
    id TEXT PRIMARY KEY NOT NULL,
    tenant_id TEXT NOT NULL,
    fanout_job_id TEXT NOT NULL,
    recipient_id TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'pending'
        CHECK (status IN ('pending', 'processing', 'processed', 'skipped', 'retryable_error', 'failed')),
    notification_id TEXT NULL,
    idempotency_key TEXT NOT NULL,
    last_error_code TEXT NULL,
    attempt_count INTEGER NOT NULL DEFAULT 0 CHECK (attempt_count >= 0),
    next_attempt_at TEXT NULL,
    lease_owner TEXT NULL,
    lease_expires_at TEXT NULL,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    processed_at TEXT NULL,
    FOREIGN KEY (tenant_id, fanout_job_id) REFERENCES notification_fanout_jobs(tenant_id, id) ON DELETE CASCADE,
    FOREIGN KEY (tenant_id, recipient_id) REFERENCES users(tenant_id, id) ON DELETE CASCADE,
    FOREIGN KEY (notification_id) REFERENCES notifications(id) ON DELETE SET NULL,
    CHECK (length(idempotency_key) BETWEEN 1 AND 191),
    CHECK (last_error_code IS NULL OR length(last_error_code) <= 100),
    CHECK (
        (status = 'processing' AND lease_owner IS NOT NULL AND length(trim(lease_owner)) BETWEEN 1 AND 191 AND lease_expires_at IS NOT NULL)
        OR (status <> 'processing' AND lease_owner IS NULL AND lease_expires_at IS NULL)
    ),
    CHECK (
        (status IN ('pending', 'retryable_error') AND processed_at IS NULL AND notification_id IS NULL)
        OR (status = 'processing' AND processed_at IS NULL AND notification_id IS NULL)
        OR (status = 'processed' AND processed_at IS NOT NULL AND notification_id IS NOT NULL)
        OR (status IN ('skipped', 'failed') AND processed_at IS NOT NULL AND notification_id IS NULL)
    )
);

INSERT INTO notification_fanout_items (
    id, tenant_id, fanout_job_id, recipient_id, status, notification_id,
    idempotency_key, last_error_code, attempt_count, next_attempt_at,
    lease_owner, lease_expires_at, created_at, updated_at, processed_at
)
SELECT
    id, tenant_id, fanout_job_id, recipient_id, status, notification_id,
    idempotency_key, last_error_code, 0, NULL,
    NULL, NULL, created_at, updated_at, processed_at
FROM notification_fanout_items_before_candidate_processing;

DROP TABLE notification_fanout_items_before_candidate_processing;

CREATE UNIQUE INDEX ux_notification_fanout_item_recipient
    ON notification_fanout_items (tenant_id, fanout_job_id, recipient_id);
CREATE UNIQUE INDEX ux_notification_fanout_item_idempotency
    ON notification_fanout_items (tenant_id, idempotency_key);
CREATE INDEX idx_notification_fanout_item_pending
    ON notification_fanout_items (tenant_id, fanout_job_id, status, created_at);
CREATE INDEX idx_notification_fanout_item_recovery
    ON notification_fanout_items (status, next_attempt_at, lease_expires_at, updated_at);

CREATE TRIGGER notification_fanout_item_tenant_guard_insert
BEFORE INSERT ON notification_fanout_items
FOR EACH ROW WHEN NEW.notification_id IS NOT NULL
BEGIN
    SELECT CASE WHEN NOT EXISTS (
        SELECT 1 FROM notifications WHERE id = NEW.notification_id AND tenant_id = NEW.tenant_id
    ) THEN RAISE(ABORT, 'notification fanout item tenant mismatch') END;
END;

CREATE TRIGGER notification_fanout_item_tenant_guard_update
BEFORE UPDATE OF tenant_id, notification_id ON notification_fanout_items
FOR EACH ROW WHEN NEW.notification_id IS NOT NULL
BEGIN
    SELECT CASE WHEN NOT EXISTS (
        SELECT 1 FROM notifications WHERE id = NEW.notification_id AND tenant_id = NEW.tenant_id
    ) THEN RAISE(ABORT, 'notification fanout item tenant mismatch') END;
END;
"#;

const SQLITE_DOWN: &str = r#"
DROP TRIGGER IF EXISTS notification_fanout_item_tenant_guard_insert;
DROP TRIGGER IF EXISTS notification_fanout_item_tenant_guard_update;

ALTER TABLE notification_fanout_items RENAME TO notification_fanout_items_with_candidate_processing;

CREATE TABLE notification_fanout_items (
    id TEXT PRIMARY KEY NOT NULL,
    tenant_id TEXT NOT NULL,
    fanout_job_id TEXT NOT NULL,
    recipient_id TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'pending'
        CHECK (status IN ('pending', 'processed', 'skipped', 'failed')),
    notification_id TEXT NULL,
    idempotency_key TEXT NOT NULL,
    last_error_code TEXT NULL,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    processed_at TEXT NULL,
    FOREIGN KEY (tenant_id, fanout_job_id) REFERENCES notification_fanout_jobs(tenant_id, id) ON DELETE CASCADE,
    FOREIGN KEY (tenant_id, recipient_id) REFERENCES users(tenant_id, id) ON DELETE CASCADE,
    FOREIGN KEY (notification_id) REFERENCES notifications(id) ON DELETE SET NULL,
    CHECK (length(idempotency_key) BETWEEN 1 AND 191),
    CHECK (last_error_code IS NULL OR length(last_error_code) <= 100),
    CHECK (
        (status = 'pending' AND processed_at IS NULL AND notification_id IS NULL)
        OR (status = 'processed' AND processed_at IS NOT NULL AND notification_id IS NOT NULL)
        OR (status IN ('skipped', 'failed') AND processed_at IS NOT NULL)
    )
);

INSERT INTO notification_fanout_items (
    id, tenant_id, fanout_job_id, recipient_id, status, notification_id,
    idempotency_key, last_error_code, created_at, updated_at, processed_at
)
SELECT
    id, tenant_id, fanout_job_id, recipient_id,
    CASE WHEN status IN ('processing', 'retryable_error') THEN 'failed' ELSE status END,
    CASE WHEN status IN ('processing', 'retryable_error') THEN NULL ELSE notification_id END,
    idempotency_key,
    CASE
        WHEN status IN ('processing', 'retryable_error')
            THEN COALESCE(last_error_code, 'NOTIFICATION_CANDIDATE_DOWNGRADE')
        ELSE last_error_code
    END,
    created_at,
    updated_at,
    CASE
        WHEN status IN ('processing', 'retryable_error') THEN COALESCE(processed_at, CURRENT_TIMESTAMP)
        ELSE processed_at
    END
FROM notification_fanout_items_with_candidate_processing;

DROP TABLE notification_fanout_items_with_candidate_processing;

CREATE UNIQUE INDEX ux_notification_fanout_item_recipient
    ON notification_fanout_items (tenant_id, fanout_job_id, recipient_id);
CREATE UNIQUE INDEX ux_notification_fanout_item_idempotency
    ON notification_fanout_items (tenant_id, idempotency_key);
CREATE INDEX idx_notification_fanout_item_pending
    ON notification_fanout_items (tenant_id, fanout_job_id, status, created_at);

CREATE TRIGGER notification_fanout_item_tenant_guard_insert
BEFORE INSERT ON notification_fanout_items
FOR EACH ROW WHEN NEW.notification_id IS NOT NULL
BEGIN
    SELECT CASE WHEN NOT EXISTS (
        SELECT 1 FROM notifications WHERE id = NEW.notification_id AND tenant_id = NEW.tenant_id
    ) THEN RAISE(ABORT, 'notification fanout item tenant mismatch') END;
END;

CREATE TRIGGER notification_fanout_item_tenant_guard_update
BEFORE UPDATE OF tenant_id, notification_id ON notification_fanout_items
FOR EACH ROW WHEN NEW.notification_id IS NOT NULL
BEGIN
    SELECT CASE WHEN NOT EXISTS (
        SELECT 1 FROM notifications WHERE id = NEW.notification_id AND tenant_id = NEW.tenant_id
    ) THEN RAISE(ABORT, 'notification fanout item tenant mismatch') END;
END;
"#;
