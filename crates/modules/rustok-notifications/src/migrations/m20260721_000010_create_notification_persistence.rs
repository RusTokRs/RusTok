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
                "notification persistence does not support database backend {backend:?}"
            ))),
        }
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .get_connection()
            .execute_unprepared(
                r#"
                DROP TABLE IF EXISTS notification_push_subscriptions;
                DROP TABLE IF EXISTS notification_digest_items;
                DROP TABLE IF EXISTS notification_digest_jobs;
                DROP TABLE IF EXISTS notification_preferences;
                DROP TABLE IF EXISTS notification_fanout_items;
                DROP TABLE IF EXISTS notification_fanout_jobs;
                DROP TABLE IF EXISTS notification_delivery_attempts;
                DROP TABLE IF EXISTS notifications;
                DROP INDEX IF EXISTS ux_users_tenant_identity;
                "#,
            )
            .await
            .map(|_| ())
    }
}

const POSTGRES_UP: &str = r#"
CREATE UNIQUE INDEX IF NOT EXISTS ux_users_tenant_identity
    ON users (tenant_id, id);

CREATE TABLE IF NOT EXISTS notifications (
    id UUID PRIMARY KEY,
    tenant_id UUID NOT NULL,
    recipient_id UUID NOT NULL,
    source_slug VARCHAR(64) NOT NULL,
    source_event_id UUID NOT NULL,
    source_revision BIGINT NOT NULL,
    notification_type VARCHAR(128) NOT NULL,
    template_key VARCHAR(128) NOT NULL,
    target_owner VARCHAR(64) NOT NULL,
    target_kind VARCHAR(128) NOT NULL,
    target_id UUID NOT NULL,
    actor_id UUID NULL,
    priority VARCHAR(16) NOT NULL DEFAULT 'normal',
    state VARCHAR(16) NOT NULL DEFAULT 'unread',
    template_data_json JSONB NOT NULL,
    group_key VARCHAR(191) NULL,
    idempotency_key VARCHAR(191) NOT NULL,
    seen_at TIMESTAMPTZ NULL,
    read_at TIMESTAMPTZ NULL,
    archived_at TIMESTAMPTZ NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    CONSTRAINT fk_notifications_tenant FOREIGN KEY (tenant_id)
        REFERENCES tenants(id) ON DELETE CASCADE,
    CONSTRAINT fk_notifications_recipient FOREIGN KEY (tenant_id, recipient_id)
        REFERENCES users(tenant_id, id) ON DELETE CASCADE,
    CONSTRAINT fk_notifications_actor FOREIGN KEY (actor_id)
        REFERENCES users(id) ON DELETE SET NULL,
    CONSTRAINT ck_notifications_source_revision CHECK (source_revision > 0),
    CONSTRAINT ck_notifications_priority CHECK (priority IN ('low', 'normal', 'high', 'urgent')),
    CONSTRAINT ck_notifications_state CHECK (state IN ('unread', 'seen', 'read', 'archived')),
    CONSTRAINT ck_notifications_payload CHECK (
        jsonb_typeof(template_data_json) = 'object'
        AND octet_length(template_data_json::text) <= 8192
    ),
    CONSTRAINT ck_notifications_strings CHECK (
        btrim(source_slug) <> ''
        AND btrim(notification_type) <> ''
        AND btrim(template_key) <> ''
        AND btrim(target_owner) <> ''
        AND btrim(target_kind) <> ''
        AND btrim(idempotency_key) <> ''
    ),
    CONSTRAINT ck_notifications_read_seen CHECK (read_at IS NULL OR seen_at IS NOT NULL),
    CONSTRAINT ck_notifications_state_timestamps CHECK (
        (state = 'unread' AND seen_at IS NULL AND read_at IS NULL AND archived_at IS NULL)
        OR (state = 'seen' AND seen_at IS NOT NULL AND read_at IS NULL AND archived_at IS NULL)
        OR (state = 'read' AND seen_at IS NOT NULL AND read_at IS NOT NULL AND archived_at IS NULL)
        OR (state = 'archived' AND archived_at IS NOT NULL)
    )
);

CREATE UNIQUE INDEX IF NOT EXISTS ux_notifications_tenant_identity
    ON notifications (tenant_id, id);
CREATE UNIQUE INDEX IF NOT EXISTS ux_notifications_source_recipient_dedupe
    ON notifications (tenant_id, recipient_id, source_slug, source_event_id, notification_type);
CREATE UNIQUE INDEX IF NOT EXISTS ux_notifications_idempotency
    ON notifications (tenant_id, idempotency_key);
CREATE INDEX IF NOT EXISTS idx_notifications_inbox
    ON notifications (tenant_id, recipient_id, state, created_at DESC, id DESC);
CREATE INDEX IF NOT EXISTS idx_notifications_group
    ON notifications (tenant_id, recipient_id, group_key, created_at DESC);

CREATE TABLE IF NOT EXISTS notification_delivery_attempts (
    id UUID PRIMARY KEY,
    tenant_id UUID NOT NULL,
    notification_id UUID NOT NULL,
    recipient_id UUID NOT NULL,
    channel VARCHAR(24) NOT NULL,
    status VARCHAR(24) NOT NULL DEFAULT 'pending',
    provider_key VARCHAR(64) NULL,
    idempotency_key VARCHAR(191) NOT NULL,
    attempt_count INTEGER NOT NULL DEFAULT 0,
    next_attempt_at TIMESTAMPTZ NULL,
    lease_owner VARCHAR(191) NULL,
    lease_expires_at TIMESTAMPTZ NULL,
    last_error_code VARCHAR(100) NULL,
    last_error_message VARCHAR(2000) NULL,
    provider_message_id VARCHAR(191) NULL,
    sent_at TIMESTAMPTZ NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    CONSTRAINT fk_notification_delivery_notification FOREIGN KEY (tenant_id, notification_id)
        REFERENCES notifications(tenant_id, id) ON DELETE CASCADE,
    CONSTRAINT fk_notification_delivery_recipient FOREIGN KEY (tenant_id, recipient_id)
        REFERENCES users(tenant_id, id) ON DELETE CASCADE,
    CONSTRAINT ck_notification_delivery_channel CHECK (channel IN ('in_app', 'email', 'web_push', 'mobile_push', 'sms')),
    CONSTRAINT ck_notification_delivery_status CHECK (status IN ('pending', 'leased', 'sent', 'retryable_error', 'permanent_error', 'cancelled')),
    CONSTRAINT ck_notification_delivery_attempt CHECK (attempt_count >= 0),
    CONSTRAINT ck_notification_delivery_lease CHECK (
        (status = 'leased' AND lease_owner IS NOT NULL AND btrim(lease_owner) <> '' AND lease_expires_at IS NOT NULL)
        OR (status <> 'leased' AND lease_owner IS NULL AND lease_expires_at IS NULL)
    ),
    CONSTRAINT ck_notification_delivery_sent CHECK (
        (status = 'sent' AND sent_at IS NOT NULL)
        OR (status <> 'sent' AND sent_at IS NULL)
    ),
    CONSTRAINT ck_notification_delivery_idempotency CHECK (btrim(idempotency_key) <> '')
);

CREATE UNIQUE INDEX IF NOT EXISTS ux_notification_delivery_idempotency
    ON notification_delivery_attempts (tenant_id, channel, idempotency_key);
CREATE INDEX IF NOT EXISTS idx_notification_delivery_recovery
    ON notification_delivery_attempts (status, next_attempt_at, lease_expires_at, updated_at);
CREATE INDEX IF NOT EXISTS idx_notification_delivery_notification
    ON notification_delivery_attempts (tenant_id, notification_id, created_at);

CREATE TABLE IF NOT EXISTS notification_fanout_jobs (
    id UUID PRIMARY KEY,
    tenant_id UUID NOT NULL,
    source_slug VARCHAR(64) NOT NULL,
    source_event_id UUID NOT NULL,
    source_revision BIGINT NOT NULL,
    notification_type VARCHAR(128) NOT NULL,
    descriptor_json JSONB NOT NULL,
    audience_cursor VARCHAR(512) NULL,
    status VARCHAR(24) NOT NULL DEFAULT 'pending',
    attempt_count INTEGER NOT NULL DEFAULT 0,
    next_attempt_at TIMESTAMPTZ NULL,
    lease_owner VARCHAR(191) NULL,
    lease_expires_at TIMESTAMPTZ NULL,
    last_error_code VARCHAR(100) NULL,
    last_error_message VARCHAR(2000) NULL,
    completed_at TIMESTAMPTZ NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    CONSTRAINT fk_notification_fanout_job_tenant FOREIGN KEY (tenant_id)
        REFERENCES tenants(id) ON DELETE CASCADE,
    CONSTRAINT ck_notification_fanout_job_identity CHECK (
        source_revision > 0
        AND btrim(source_slug) <> ''
        AND btrim(notification_type) <> ''
    ),
    CONSTRAINT ck_notification_fanout_job_payload CHECK (
        jsonb_typeof(descriptor_json) = 'object'
        AND octet_length(descriptor_json::text) <= 16384
    ),
    CONSTRAINT ck_notification_fanout_job_status CHECK (status IN ('pending', 'leased', 'completed', 'retryable_error', 'dead_letter')),
    CONSTRAINT ck_notification_fanout_job_attempt CHECK (attempt_count >= 0),
    CONSTRAINT ck_notification_fanout_job_lease CHECK (
        (status = 'leased' AND lease_owner IS NOT NULL AND btrim(lease_owner) <> '' AND lease_expires_at IS NOT NULL)
        OR (status <> 'leased' AND lease_owner IS NULL AND lease_expires_at IS NULL)
    ),
    CONSTRAINT ck_notification_fanout_job_completion CHECK (
        (status = 'completed' AND completed_at IS NOT NULL)
        OR (status <> 'completed' AND completed_at IS NULL)
    )
);

CREATE UNIQUE INDEX IF NOT EXISTS ux_notification_fanout_job_tenant_identity
    ON notification_fanout_jobs (tenant_id, id);
CREATE UNIQUE INDEX IF NOT EXISTS ux_notification_fanout_source
    ON notification_fanout_jobs (tenant_id, source_slug, source_event_id, notification_type);
CREATE INDEX IF NOT EXISTS idx_notification_fanout_recovery
    ON notification_fanout_jobs (status, next_attempt_at, lease_expires_at, updated_at);

CREATE TABLE IF NOT EXISTS notification_fanout_items (
    id UUID PRIMARY KEY,
    tenant_id UUID NOT NULL,
    fanout_job_id UUID NOT NULL,
    recipient_id UUID NOT NULL,
    status VARCHAR(16) NOT NULL DEFAULT 'pending',
    notification_id UUID NULL,
    idempotency_key VARCHAR(191) NOT NULL,
    last_error_code VARCHAR(100) NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    processed_at TIMESTAMPTZ NULL,
    CONSTRAINT fk_notification_fanout_item_job FOREIGN KEY (tenant_id, fanout_job_id)
        REFERENCES notification_fanout_jobs(tenant_id, id) ON DELETE CASCADE,
    CONSTRAINT fk_notification_fanout_item_recipient FOREIGN KEY (tenant_id, recipient_id)
        REFERENCES users(tenant_id, id) ON DELETE CASCADE,
    CONSTRAINT fk_notification_fanout_item_notification FOREIGN KEY (notification_id)
        REFERENCES notifications(id) ON DELETE SET NULL,
    CONSTRAINT ck_notification_fanout_item_status CHECK (status IN ('pending', 'processed', 'skipped', 'failed')),
    CONSTRAINT ck_notification_fanout_item_completion CHECK (
        (status = 'pending' AND processed_at IS NULL AND notification_id IS NULL)
        OR (status = 'processed' AND processed_at IS NOT NULL AND notification_id IS NOT NULL)
        OR (status IN ('skipped', 'failed') AND processed_at IS NOT NULL)
    ),
    CONSTRAINT ck_notification_fanout_item_idempotency CHECK (btrim(idempotency_key) <> '')
);

CREATE UNIQUE INDEX IF NOT EXISTS ux_notification_fanout_item_recipient
    ON notification_fanout_items (tenant_id, fanout_job_id, recipient_id);
CREATE UNIQUE INDEX IF NOT EXISTS ux_notification_fanout_item_idempotency
    ON notification_fanout_items (tenant_id, idempotency_key);
CREATE INDEX IF NOT EXISTS idx_notification_fanout_item_pending
    ON notification_fanout_items (tenant_id, fanout_job_id, status, created_at);

CREATE TABLE IF NOT EXISTS notification_preferences (
    id UUID PRIMARY KEY,
    tenant_id UUID NOT NULL,
    user_id UUID NOT NULL,
    source_scope VARCHAR(64) NOT NULL DEFAULT '*',
    type_scope VARCHAR(128) NOT NULL DEFAULT '*',
    delivery_mode VARCHAR(16) NOT NULL DEFAULT 'instant',
    in_app_enabled BOOLEAN NOT NULL DEFAULT TRUE,
    email_enabled BOOLEAN NOT NULL DEFAULT FALSE,
    push_enabled BOOLEAN NOT NULL DEFAULT FALSE,
    sms_enabled BOOLEAN NOT NULL DEFAULT FALSE,
    digest_mode VARCHAR(16) NOT NULL DEFAULT 'daily',
    timezone VARCHAR(64) NOT NULL DEFAULT 'UTC',
    quiet_start_minute SMALLINT NULL,
    quiet_end_minute SMALLINT NULL,
    revision BIGINT NOT NULL DEFAULT 1,
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    CONSTRAINT fk_notification_preference_user FOREIGN KEY (tenant_id, user_id)
        REFERENCES users(tenant_id, id) ON DELETE CASCADE,
    CONSTRAINT ck_notification_preference_mode CHECK (delivery_mode IN ('off', 'instant', 'digest')),
    CONSTRAINT ck_notification_preference_digest CHECK (digest_mode IN ('hourly', 'daily', 'weekly')),
    CONSTRAINT ck_notification_preference_scope CHECK (
        btrim(source_scope) <> '' AND btrim(type_scope) <> ''
        AND source_scope !~ '[[:space:]]'
        AND type_scope !~ '[[:space:]]'
    ),
    CONSTRAINT ck_notification_preference_timezone CHECK (btrim(timezone) <> ''),
    CONSTRAINT ck_notification_preference_quiet CHECK (
        (quiet_start_minute IS NULL AND quiet_end_minute IS NULL)
        OR (
            quiet_start_minute BETWEEN 0 AND 1439
            AND quiet_end_minute BETWEEN 0 AND 1439
            AND quiet_start_minute <> quiet_end_minute
        )
    ),
    CONSTRAINT ck_notification_preference_revision CHECK (revision > 0)
);

CREATE UNIQUE INDEX IF NOT EXISTS ux_notification_preference_scope
    ON notification_preferences (tenant_id, user_id, source_scope, type_scope);
CREATE INDEX IF NOT EXISTS idx_notification_preference_user
    ON notification_preferences (tenant_id, user_id, updated_at);

CREATE TABLE IF NOT EXISTS notification_digest_jobs (
    id UUID PRIMARY KEY,
    tenant_id UUID NOT NULL,
    recipient_id UUID NOT NULL,
    schedule_key VARCHAR(191) NOT NULL,
    digest_mode VARCHAR(16) NOT NULL,
    status VARCHAR(24) NOT NULL DEFAULT 'pending',
    window_start TIMESTAMPTZ NOT NULL,
    window_end TIMESTAMPTZ NOT NULL,
    attempt_count INTEGER NOT NULL DEFAULT 0,
    next_attempt_at TIMESTAMPTZ NULL,
    lease_owner VARCHAR(191) NULL,
    lease_expires_at TIMESTAMPTZ NULL,
    last_error_code VARCHAR(100) NULL,
    last_error_message VARCHAR(2000) NULL,
    sent_at TIMESTAMPTZ NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    CONSTRAINT fk_notification_digest_job_recipient FOREIGN KEY (tenant_id, recipient_id)
        REFERENCES users(tenant_id, id) ON DELETE CASCADE,
    CONSTRAINT ck_notification_digest_job_mode CHECK (digest_mode IN ('hourly', 'daily', 'weekly')),
    CONSTRAINT ck_notification_digest_job_status CHECK (status IN ('pending', 'leased', 'ready', 'sent', 'retryable_error', 'dead_letter')),
    CONSTRAINT ck_notification_digest_job_window CHECK (window_end > window_start),
    CONSTRAINT ck_notification_digest_job_attempt CHECK (attempt_count >= 0),
    CONSTRAINT ck_notification_digest_job_lease CHECK (
        (status = 'leased' AND lease_owner IS NOT NULL AND btrim(lease_owner) <> '' AND lease_expires_at IS NOT NULL)
        OR (status <> 'leased' AND lease_owner IS NULL AND lease_expires_at IS NULL)
    ),
    CONSTRAINT ck_notification_digest_job_sent CHECK (
        (status = 'sent' AND sent_at IS NOT NULL)
        OR (status <> 'sent' AND sent_at IS NULL)
    ),
    CONSTRAINT ck_notification_digest_job_schedule CHECK (btrim(schedule_key) <> '')
);

CREATE UNIQUE INDEX IF NOT EXISTS ux_notification_digest_job_tenant_identity
    ON notification_digest_jobs (tenant_id, id);
CREATE UNIQUE INDEX IF NOT EXISTS ux_notification_digest_window
    ON notification_digest_jobs (tenant_id, recipient_id, schedule_key, window_start);
CREATE INDEX IF NOT EXISTS idx_notification_digest_recovery
    ON notification_digest_jobs (status, next_attempt_at, lease_expires_at, updated_at);

CREATE TABLE IF NOT EXISTS notification_digest_items (
    id UUID PRIMARY KEY,
    tenant_id UUID NOT NULL,
    digest_job_id UUID NOT NULL,
    notification_id UUID NOT NULL,
    idempotency_key VARCHAR(191) NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    CONSTRAINT fk_notification_digest_item_job FOREIGN KEY (tenant_id, digest_job_id)
        REFERENCES notification_digest_jobs(tenant_id, id) ON DELETE CASCADE,
    CONSTRAINT fk_notification_digest_item_notification FOREIGN KEY (tenant_id, notification_id)
        REFERENCES notifications(tenant_id, id) ON DELETE CASCADE,
    CONSTRAINT ck_notification_digest_item_idempotency CHECK (btrim(idempotency_key) <> '')
);

CREATE UNIQUE INDEX IF NOT EXISTS ux_notification_digest_item_notification
    ON notification_digest_items (tenant_id, digest_job_id, notification_id);
CREATE UNIQUE INDEX IF NOT EXISTS ux_notification_digest_item_idempotency
    ON notification_digest_items (tenant_id, idempotency_key);

CREATE TABLE IF NOT EXISTS notification_push_subscriptions (
    id UUID PRIMARY KEY,
    tenant_id UUID NOT NULL,
    user_id UUID NOT NULL,
    platform VARCHAR(16) NOT NULL,
    endpoint_hash VARCHAR(64) NOT NULL,
    encrypted_endpoint VARCHAR(4096) NOT NULL,
    encrypted_p256dh VARCHAR(2048) NULL,
    encrypted_auth VARCHAR(2048) NULL,
    key_version VARCHAR(64) NOT NULL,
    status VARCHAR(16) NOT NULL DEFAULT 'active',
    failure_count INTEGER NOT NULL DEFAULT 0,
    last_success_at TIMESTAMPTZ NULL,
    revoked_at TIMESTAMPTZ NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    CONSTRAINT fk_notification_push_user FOREIGN KEY (tenant_id, user_id)
        REFERENCES users(tenant_id, id) ON DELETE CASCADE,
    CONSTRAINT ck_notification_push_platform CHECK (platform IN ('web', 'ios', 'android')),
    CONSTRAINT ck_notification_push_status CHECK (status IN ('active', 'revoked')),
    CONSTRAINT ck_notification_push_hash CHECK (endpoint_hash ~ '^[0-9a-f]{64}$'),
    CONSTRAINT ck_notification_push_secret CHECK (btrim(encrypted_endpoint) <> '' AND btrim(key_version) <> ''),
    CONSTRAINT ck_notification_push_failure CHECK (failure_count >= 0),
    CONSTRAINT ck_notification_push_revocation CHECK (
        (status = 'active' AND revoked_at IS NULL)
        OR (status = 'revoked' AND revoked_at IS NOT NULL)
    )
);

CREATE UNIQUE INDEX IF NOT EXISTS ux_notification_push_endpoint
    ON notification_push_subscriptions (tenant_id, user_id, endpoint_hash);
CREATE INDEX IF NOT EXISTS idx_notification_push_active
    ON notification_push_subscriptions (tenant_id, user_id, status, updated_at);

CREATE OR REPLACE FUNCTION enforce_notification_tenant_integrity()
RETURNS trigger AS $$
BEGIN
    IF TG_TABLE_NAME = 'notifications' AND NEW.actor_id IS NOT NULL
       AND NOT EXISTS (
           SELECT 1 FROM users
           WHERE users.id = NEW.actor_id AND users.tenant_id = NEW.tenant_id
       ) THEN
        RAISE EXCEPTION 'notification actor tenant mismatch' USING ERRCODE = '23514';
    END IF;

    IF TG_TABLE_NAME = 'notification_fanout_items' AND NEW.notification_id IS NOT NULL
       AND NOT EXISTS (
           SELECT 1 FROM notifications
           WHERE notifications.id = NEW.notification_id
             AND notifications.tenant_id = NEW.tenant_id
       ) THEN
        RAISE EXCEPTION 'notification fanout item tenant mismatch' USING ERRCODE = '23514';
    END IF;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS notifications_tenant_integrity_guard ON notifications;
CREATE TRIGGER notifications_tenant_integrity_guard
BEFORE INSERT OR UPDATE ON notifications
FOR EACH ROW EXECUTE FUNCTION enforce_notification_tenant_integrity();

DROP TRIGGER IF EXISTS notification_fanout_item_tenant_integrity_guard ON notification_fanout_items;
CREATE TRIGGER notification_fanout_item_tenant_integrity_guard
BEFORE INSERT OR UPDATE ON notification_fanout_items
FOR EACH ROW EXECUTE FUNCTION enforce_notification_tenant_integrity();
"#;

const SQLITE_UP: &str = r#"
CREATE UNIQUE INDEX IF NOT EXISTS ux_users_tenant_identity
    ON users (tenant_id, id);

CREATE TABLE IF NOT EXISTS notifications (
    id TEXT PRIMARY KEY NOT NULL,
    tenant_id TEXT NOT NULL,
    recipient_id TEXT NOT NULL,
    source_slug TEXT NOT NULL,
    source_event_id TEXT NOT NULL,
    source_revision INTEGER NOT NULL,
    notification_type TEXT NOT NULL,
    template_key TEXT NOT NULL,
    target_owner TEXT NOT NULL,
    target_kind TEXT NOT NULL,
    target_id TEXT NOT NULL,
    actor_id TEXT NULL,
    priority TEXT NOT NULL DEFAULT 'normal',
    state TEXT NOT NULL DEFAULT 'unread',
    template_data_json TEXT NOT NULL,
    group_key TEXT NULL,
    idempotency_key TEXT NOT NULL,
    seen_at TEXT NULL,
    read_at TEXT NULL,
    archived_at TEXT NULL,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (tenant_id) REFERENCES tenants(id) ON DELETE CASCADE,
    FOREIGN KEY (tenant_id, recipient_id) REFERENCES users(tenant_id, id) ON DELETE CASCADE,
    FOREIGN KEY (actor_id) REFERENCES users(id) ON DELETE SET NULL,
    CHECK (source_revision > 0),
    CHECK (priority IN ('low', 'normal', 'high', 'urgent')),
    CHECK (state IN ('unread', 'seen', 'read', 'archived')),
    CHECK (json_valid(template_data_json) AND json_type(template_data_json) = 'object' AND length(template_data_json) <= 8192),
    CHECK (trim(source_slug) <> '' AND length(source_slug) <= 64),
    CHECK (trim(notification_type) <> '' AND length(notification_type) <= 128),
    CHECK (trim(template_key) <> '' AND length(template_key) <= 128),
    CHECK (trim(target_owner) <> '' AND length(target_owner) <= 64),
    CHECK (trim(target_kind) <> '' AND length(target_kind) <= 128),
    CHECK (trim(idempotency_key) <> '' AND length(idempotency_key) <= 191),
    CHECK (read_at IS NULL OR seen_at IS NOT NULL),
    CHECK (
        (state = 'unread' AND seen_at IS NULL AND read_at IS NULL AND archived_at IS NULL)
        OR (state = 'seen' AND seen_at IS NOT NULL AND read_at IS NULL AND archived_at IS NULL)
        OR (state = 'read' AND seen_at IS NOT NULL AND read_at IS NOT NULL AND archived_at IS NULL)
        OR (state = 'archived' AND archived_at IS NOT NULL)
    )
);

CREATE UNIQUE INDEX IF NOT EXISTS ux_notifications_tenant_identity
    ON notifications (tenant_id, id);
CREATE UNIQUE INDEX IF NOT EXISTS ux_notifications_source_recipient_dedupe
    ON notifications (tenant_id, recipient_id, source_slug, source_event_id, notification_type);
CREATE UNIQUE INDEX IF NOT EXISTS ux_notifications_idempotency
    ON notifications (tenant_id, idempotency_key);
CREATE INDEX IF NOT EXISTS idx_notifications_inbox
    ON notifications (tenant_id, recipient_id, state, created_at DESC, id DESC);
CREATE INDEX IF NOT EXISTS idx_notifications_group
    ON notifications (tenant_id, recipient_id, group_key, created_at DESC);

CREATE TABLE IF NOT EXISTS notification_delivery_attempts (
    id TEXT PRIMARY KEY NOT NULL,
    tenant_id TEXT NOT NULL,
    notification_id TEXT NOT NULL,
    recipient_id TEXT NOT NULL,
    channel TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'pending',
    provider_key TEXT NULL,
    idempotency_key TEXT NOT NULL,
    attempt_count INTEGER NOT NULL DEFAULT 0,
    next_attempt_at TEXT NULL,
    lease_owner TEXT NULL,
    lease_expires_at TEXT NULL,
    last_error_code TEXT NULL,
    last_error_message TEXT NULL,
    provider_message_id TEXT NULL,
    sent_at TEXT NULL,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (tenant_id, notification_id) REFERENCES notifications(tenant_id, id) ON DELETE CASCADE,
    FOREIGN KEY (tenant_id, recipient_id) REFERENCES users(tenant_id, id) ON DELETE CASCADE,
    CHECK (channel IN ('in_app', 'email', 'web_push', 'mobile_push', 'sms')),
    CHECK (status IN ('pending', 'leased', 'sent', 'retryable_error', 'permanent_error', 'cancelled')),
    CHECK (attempt_count >= 0),
    CHECK (length(idempotency_key) BETWEEN 1 AND 191),
    CHECK (last_error_code IS NULL OR length(last_error_code) <= 100),
    CHECK (last_error_message IS NULL OR length(last_error_message) <= 2000),
    CHECK (
        (status = 'leased' AND lease_owner IS NOT NULL AND trim(lease_owner) <> '' AND lease_expires_at IS NOT NULL)
        OR (status <> 'leased' AND lease_owner IS NULL AND lease_expires_at IS NULL)
    ),
    CHECK ((status = 'sent' AND sent_at IS NOT NULL) OR (status <> 'sent' AND sent_at IS NULL))
);

CREATE UNIQUE INDEX IF NOT EXISTS ux_notification_delivery_idempotency
    ON notification_delivery_attempts (tenant_id, channel, idempotency_key);
CREATE INDEX IF NOT EXISTS idx_notification_delivery_recovery
    ON notification_delivery_attempts (status, next_attempt_at, lease_expires_at, updated_at);
CREATE INDEX IF NOT EXISTS idx_notification_delivery_notification
    ON notification_delivery_attempts (tenant_id, notification_id, created_at);

CREATE TABLE IF NOT EXISTS notification_fanout_jobs (
    id TEXT PRIMARY KEY NOT NULL,
    tenant_id TEXT NOT NULL,
    source_slug TEXT NOT NULL,
    source_event_id TEXT NOT NULL,
    source_revision INTEGER NOT NULL,
    notification_type TEXT NOT NULL,
    descriptor_json TEXT NOT NULL,
    audience_cursor TEXT NULL,
    status TEXT NOT NULL DEFAULT 'pending',
    attempt_count INTEGER NOT NULL DEFAULT 0,
    next_attempt_at TEXT NULL,
    lease_owner TEXT NULL,
    lease_expires_at TEXT NULL,
    last_error_code TEXT NULL,
    last_error_message TEXT NULL,
    completed_at TEXT NULL,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (tenant_id) REFERENCES tenants(id) ON DELETE CASCADE,
    CHECK (source_revision > 0),
    CHECK (length(source_slug) BETWEEN 1 AND 64),
    CHECK (length(notification_type) BETWEEN 1 AND 128),
    CHECK (json_valid(descriptor_json) AND json_type(descriptor_json) = 'object' AND length(descriptor_json) <= 16384),
    CHECK (audience_cursor IS NULL OR length(audience_cursor) <= 512),
    CHECK (status IN ('pending', 'leased', 'completed', 'retryable_error', 'dead_letter')),
    CHECK (attempt_count >= 0),
    CHECK (last_error_code IS NULL OR length(last_error_code) <= 100),
    CHECK (last_error_message IS NULL OR length(last_error_message) <= 2000),
    CHECK (
        (status = 'leased' AND lease_owner IS NOT NULL AND trim(lease_owner) <> '' AND lease_expires_at IS NOT NULL)
        OR (status <> 'leased' AND lease_owner IS NULL AND lease_expires_at IS NULL)
    ),
    CHECK ((status = 'completed' AND completed_at IS NOT NULL) OR (status <> 'completed' AND completed_at IS NULL))
);

CREATE UNIQUE INDEX IF NOT EXISTS ux_notification_fanout_job_tenant_identity
    ON notification_fanout_jobs (tenant_id, id);
CREATE UNIQUE INDEX IF NOT EXISTS ux_notification_fanout_source
    ON notification_fanout_jobs (tenant_id, source_slug, source_event_id, notification_type);
CREATE INDEX IF NOT EXISTS idx_notification_fanout_recovery
    ON notification_fanout_jobs (status, next_attempt_at, lease_expires_at, updated_at);

CREATE TABLE IF NOT EXISTS notification_fanout_items (
    id TEXT PRIMARY KEY NOT NULL,
    tenant_id TEXT NOT NULL,
    fanout_job_id TEXT NOT NULL,
    recipient_id TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'pending',
    notification_id TEXT NULL,
    idempotency_key TEXT NOT NULL,
    last_error_code TEXT NULL,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    processed_at TEXT NULL,
    FOREIGN KEY (tenant_id, fanout_job_id) REFERENCES notification_fanout_jobs(tenant_id, id) ON DELETE CASCADE,
    FOREIGN KEY (tenant_id, recipient_id) REFERENCES users(tenant_id, id) ON DELETE CASCADE,
    FOREIGN KEY (notification_id) REFERENCES notifications(id) ON DELETE SET NULL,
    CHECK (status IN ('pending', 'processed', 'skipped', 'failed')),
    CHECK (length(idempotency_key) BETWEEN 1 AND 191),
    CHECK (last_error_code IS NULL OR length(last_error_code) <= 100),
    CHECK (
        (status = 'pending' AND processed_at IS NULL AND notification_id IS NULL)
        OR (status = 'processed' AND processed_at IS NOT NULL AND notification_id IS NOT NULL)
        OR (status IN ('skipped', 'failed') AND processed_at IS NOT NULL)
    )
);

CREATE UNIQUE INDEX IF NOT EXISTS ux_notification_fanout_item_recipient
    ON notification_fanout_items (tenant_id, fanout_job_id, recipient_id);
CREATE UNIQUE INDEX IF NOT EXISTS ux_notification_fanout_item_idempotency
    ON notification_fanout_items (tenant_id, idempotency_key);
CREATE INDEX IF NOT EXISTS idx_notification_fanout_item_pending
    ON notification_fanout_items (tenant_id, fanout_job_id, status, created_at);

CREATE TABLE IF NOT EXISTS notification_preferences (
    id TEXT PRIMARY KEY NOT NULL,
    tenant_id TEXT NOT NULL,
    user_id TEXT NOT NULL,
    source_scope TEXT NOT NULL DEFAULT '*',
    type_scope TEXT NOT NULL DEFAULT '*',
    delivery_mode TEXT NOT NULL DEFAULT 'instant',
    in_app_enabled INTEGER NOT NULL DEFAULT 1,
    email_enabled INTEGER NOT NULL DEFAULT 0,
    push_enabled INTEGER NOT NULL DEFAULT 0,
    sms_enabled INTEGER NOT NULL DEFAULT 0,
    digest_mode TEXT NOT NULL DEFAULT 'daily',
    timezone TEXT NOT NULL DEFAULT 'UTC',
    quiet_start_minute INTEGER NULL,
    quiet_end_minute INTEGER NULL,
    revision INTEGER NOT NULL DEFAULT 1,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (tenant_id, user_id) REFERENCES users(tenant_id, id) ON DELETE CASCADE,
    CHECK (length(source_scope) BETWEEN 1 AND 64 AND source_scope NOT GLOB '*[[:space:]]*'),
    CHECK (length(type_scope) BETWEEN 1 AND 128 AND type_scope NOT GLOB '*[[:space:]]*'),
    CHECK (delivery_mode IN ('off', 'instant', 'digest')),
    CHECK (in_app_enabled IN (0, 1) AND email_enabled IN (0, 1) AND push_enabled IN (0, 1) AND sms_enabled IN (0, 1)),
    CHECK (digest_mode IN ('hourly', 'daily', 'weekly')),
    CHECK (length(trim(timezone)) BETWEEN 1 AND 64),
    CHECK (
        (quiet_start_minute IS NULL AND quiet_end_minute IS NULL)
        OR (
            quiet_start_minute BETWEEN 0 AND 1439
            AND quiet_end_minute BETWEEN 0 AND 1439
            AND quiet_start_minute <> quiet_end_minute
        )
    ),
    CHECK (revision > 0)
);

CREATE UNIQUE INDEX IF NOT EXISTS ux_notification_preference_scope
    ON notification_preferences (tenant_id, user_id, source_scope, type_scope);
CREATE INDEX IF NOT EXISTS idx_notification_preference_user
    ON notification_preferences (tenant_id, user_id, updated_at);

CREATE TABLE IF NOT EXISTS notification_digest_jobs (
    id TEXT PRIMARY KEY NOT NULL,
    tenant_id TEXT NOT NULL,
    recipient_id TEXT NOT NULL,
    schedule_key TEXT NOT NULL,
    digest_mode TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'pending',
    window_start TEXT NOT NULL,
    window_end TEXT NOT NULL,
    attempt_count INTEGER NOT NULL DEFAULT 0,
    next_attempt_at TEXT NULL,
    lease_owner TEXT NULL,
    lease_expires_at TEXT NULL,
    last_error_code TEXT NULL,
    last_error_message TEXT NULL,
    sent_at TEXT NULL,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (tenant_id, recipient_id) REFERENCES users(tenant_id, id) ON DELETE CASCADE,
    CHECK (length(schedule_key) BETWEEN 1 AND 191),
    CHECK (digest_mode IN ('hourly', 'daily', 'weekly')),
    CHECK (status IN ('pending', 'leased', 'ready', 'sent', 'retryable_error', 'dead_letter')),
    CHECK (window_end > window_start),
    CHECK (attempt_count >= 0),
    CHECK (last_error_code IS NULL OR length(last_error_code) <= 100),
    CHECK (last_error_message IS NULL OR length(last_error_message) <= 2000),
    CHECK (
        (status = 'leased' AND lease_owner IS NOT NULL AND trim(lease_owner) <> '' AND lease_expires_at IS NOT NULL)
        OR (status <> 'leased' AND lease_owner IS NULL AND lease_expires_at IS NULL)
    ),
    CHECK ((status = 'sent' AND sent_at IS NOT NULL) OR (status <> 'sent' AND sent_at IS NULL))
);

CREATE UNIQUE INDEX IF NOT EXISTS ux_notification_digest_job_tenant_identity
    ON notification_digest_jobs (tenant_id, id);
CREATE UNIQUE INDEX IF NOT EXISTS ux_notification_digest_window
    ON notification_digest_jobs (tenant_id, recipient_id, schedule_key, window_start);
CREATE INDEX IF NOT EXISTS idx_notification_digest_recovery
    ON notification_digest_jobs (status, next_attempt_at, lease_expires_at, updated_at);

CREATE TABLE IF NOT EXISTS notification_digest_items (
    id TEXT PRIMARY KEY NOT NULL,
    tenant_id TEXT NOT NULL,
    digest_job_id TEXT NOT NULL,
    notification_id TEXT NOT NULL,
    idempotency_key TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (tenant_id, digest_job_id) REFERENCES notification_digest_jobs(tenant_id, id) ON DELETE CASCADE,
    FOREIGN KEY (tenant_id, notification_id) REFERENCES notifications(tenant_id, id) ON DELETE CASCADE,
    CHECK (length(idempotency_key) BETWEEN 1 AND 191)
);

CREATE UNIQUE INDEX IF NOT EXISTS ux_notification_digest_item_notification
    ON notification_digest_items (tenant_id, digest_job_id, notification_id);
CREATE UNIQUE INDEX IF NOT EXISTS ux_notification_digest_item_idempotency
    ON notification_digest_items (tenant_id, idempotency_key);

CREATE TABLE IF NOT EXISTS notification_push_subscriptions (
    id TEXT PRIMARY KEY NOT NULL,
    tenant_id TEXT NOT NULL,
    user_id TEXT NOT NULL,
    platform TEXT NOT NULL,
    endpoint_hash TEXT NOT NULL,
    encrypted_endpoint TEXT NOT NULL,
    encrypted_p256dh TEXT NULL,
    encrypted_auth TEXT NULL,
    key_version TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'active',
    failure_count INTEGER NOT NULL DEFAULT 0,
    last_success_at TEXT NULL,
    revoked_at TEXT NULL,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (tenant_id, user_id) REFERENCES users(tenant_id, id) ON DELETE CASCADE,
    CHECK (platform IN ('web', 'ios', 'android')),
    CHECK (status IN ('active', 'revoked')),
    CHECK (length(endpoint_hash) = 64 AND endpoint_hash NOT GLOB '*[^0-9a-f]*'),
    CHECK (length(trim(encrypted_endpoint)) BETWEEN 1 AND 4096),
    CHECK (encrypted_p256dh IS NULL OR length(encrypted_p256dh) <= 2048),
    CHECK (encrypted_auth IS NULL OR length(encrypted_auth) <= 2048),
    CHECK (length(trim(key_version)) BETWEEN 1 AND 64),
    CHECK (failure_count >= 0),
    CHECK ((status = 'active' AND revoked_at IS NULL) OR (status = 'revoked' AND revoked_at IS NOT NULL))
);

CREATE UNIQUE INDEX IF NOT EXISTS ux_notification_push_endpoint
    ON notification_push_subscriptions (tenant_id, user_id, endpoint_hash);
CREATE INDEX IF NOT EXISTS idx_notification_push_active
    ON notification_push_subscriptions (tenant_id, user_id, status, updated_at);

CREATE TRIGGER IF NOT EXISTS notifications_actor_tenant_guard_insert
BEFORE INSERT ON notifications
FOR EACH ROW WHEN NEW.actor_id IS NOT NULL
BEGIN
    SELECT CASE WHEN NOT EXISTS (
        SELECT 1 FROM users WHERE id = NEW.actor_id AND tenant_id = NEW.tenant_id
    ) THEN RAISE(ABORT, 'notification actor tenant mismatch') END;
END;

CREATE TRIGGER IF NOT EXISTS notifications_actor_tenant_guard_update
BEFORE UPDATE OF tenant_id, actor_id ON notifications
FOR EACH ROW WHEN NEW.actor_id IS NOT NULL
BEGIN
    SELECT CASE WHEN NOT EXISTS (
        SELECT 1 FROM users WHERE id = NEW.actor_id AND tenant_id = NEW.tenant_id
    ) THEN RAISE(ABORT, 'notification actor tenant mismatch') END;
END;

CREATE TRIGGER IF NOT EXISTS notification_fanout_item_tenant_guard_insert
BEFORE INSERT ON notification_fanout_items
FOR EACH ROW WHEN NEW.notification_id IS NOT NULL
BEGIN
    SELECT CASE WHEN NOT EXISTS (
        SELECT 1 FROM notifications WHERE id = NEW.notification_id AND tenant_id = NEW.tenant_id
    ) THEN RAISE(ABORT, 'notification fanout item tenant mismatch') END;
END;

CREATE TRIGGER IF NOT EXISTS notification_fanout_item_tenant_guard_update
BEFORE UPDATE OF tenant_id, notification_id ON notification_fanout_items
FOR EACH ROW WHEN NEW.notification_id IS NOT NULL
BEGIN
    SELECT CASE WHEN NOT EXISTS (
        SELECT 1 FROM notifications WHERE id = NEW.notification_id AND tenant_id = NEW.tenant_id
    ) THEN RAISE(ABORT, 'notification fanout item tenant mismatch') END;
END;
"#;
