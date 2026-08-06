use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::DatabaseBackend;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        match manager.get_database_backend() {
            DatabaseBackend::Postgres => execute(manager, POSTGRES_UP).await,
            DatabaseBackend::Sqlite => execute(manager, SQLITE_UP).await,
            backend => Err(DbErr::Custom(format!(
                "rustok-forum topic route tombstone visibility migration does not support {backend:?}"
            ))),
        }
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        match manager.get_database_backend() {
            DatabaseBackend::Postgres => execute(manager, POSTGRES_DOWN).await,
            DatabaseBackend::Sqlite => execute(manager, SQLITE_DOWN).await,
            backend => Err(DbErr::Custom(format!(
                "rustok-forum topic route tombstone visibility rollback does not support {backend:?}"
            ))),
        }
    }
}

async fn execute(manager: &SchemaManager<'_>, sql: &str) -> Result<(), DbErr> {
    manager
        .get_connection()
        .execute_unprepared(sql)
        .await
        .map(|_| ())
}

const POSTGRES_UP: &str = r#"
CREATE TABLE IF NOT EXISTS forum_topic_route_tombstone_visibility (
    tenant_id UUID NOT NULL,
    topic_id UUID NOT NULL,
    publicly_disclosable BOOLEAN NOT NULL,
    route_channel_restricted BOOLEAN NOT NULL,
    created_at TIMESTAMPTZ NOT NULL,
    CONSTRAINT pk_forum_topic_route_tombstone_visibility
        PRIMARY KEY (tenant_id, topic_id),
    CONSTRAINT fk_forum_topic_route_tombstone_visibility_topic
        FOREIGN KEY (tenant_id, topic_id)
        REFERENCES forum_topics (tenant_id, id)
        ON UPDATE RESTRICT ON DELETE RESTRICT,
    CONSTRAINT ck_forum_topic_route_tombstone_visibility_topic
        CHECK (topic_id <> '00000000-0000-0000-0000-000000000000'::uuid)
);

CREATE TABLE IF NOT EXISTS forum_topic_route_tombstone_channels (
    tenant_id UUID NOT NULL,
    topic_id UUID NOT NULL,
    channel_slug VARCHAR(128) NOT NULL,
    CONSTRAINT pk_forum_topic_route_tombstone_channels
        PRIMARY KEY (tenant_id, topic_id, channel_slug),
    CONSTRAINT fk_forum_topic_route_tombstone_channels_snapshot
        FOREIGN KEY (tenant_id, topic_id)
        REFERENCES forum_topic_route_tombstone_visibility (tenant_id, topic_id)
        ON UPDATE RESTRICT ON DELETE RESTRICT,
    CONSTRAINT ck_forum_topic_route_tombstone_channel_slug CHECK (
        length(channel_slug) BETWEEN 1 AND 128
        AND channel_slug = lower(channel_slug)
        AND channel_slug = btrim(channel_slug)
        AND position(E'\n' in channel_slug) = 0
        AND position(E'\r' in channel_slug) = 0
    )
);

CREATE INDEX IF NOT EXISTS idx_forum_topic_route_tombstone_channel_lookup
    ON forum_topic_route_tombstone_channels (tenant_id, channel_slug, topic_id);

CREATE OR REPLACE FUNCTION forum_reject_topic_route_tombstone_visibility_mutation()
RETURNS trigger AS $$
BEGIN
    RAISE EXCEPTION 'forum topic route tombstone visibility is append-only';
    RETURN NULL;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS forum_topic_route_tombstone_visibility_update
    ON forum_topic_route_tombstone_visibility;
CREATE TRIGGER forum_topic_route_tombstone_visibility_update
BEFORE UPDATE ON forum_topic_route_tombstone_visibility
FOR EACH ROW EXECUTE FUNCTION forum_reject_topic_route_tombstone_visibility_mutation();

DROP TRIGGER IF EXISTS forum_topic_route_tombstone_visibility_delete
    ON forum_topic_route_tombstone_visibility;
CREATE TRIGGER forum_topic_route_tombstone_visibility_delete
BEFORE DELETE ON forum_topic_route_tombstone_visibility
FOR EACH ROW EXECUTE FUNCTION forum_reject_topic_route_tombstone_visibility_mutation();

DROP TRIGGER IF EXISTS forum_topic_route_tombstone_channel_update
    ON forum_topic_route_tombstone_channels;
CREATE TRIGGER forum_topic_route_tombstone_channel_update
BEFORE UPDATE ON forum_topic_route_tombstone_channels
FOR EACH ROW EXECUTE FUNCTION forum_reject_topic_route_tombstone_visibility_mutation();

DROP TRIGGER IF EXISTS forum_topic_route_tombstone_channel_delete
    ON forum_topic_route_tombstone_channels;
CREATE TRIGGER forum_topic_route_tombstone_channel_delete
BEFORE DELETE ON forum_topic_route_tombstone_channels
FOR EACH ROW EXECUTE FUNCTION forum_reject_topic_route_tombstone_visibility_mutation();
"#;

const POSTGRES_DOWN: &str = r#"
DROP TRIGGER IF EXISTS forum_topic_route_tombstone_channel_delete
    ON forum_topic_route_tombstone_channels;
DROP TRIGGER IF EXISTS forum_topic_route_tombstone_channel_update
    ON forum_topic_route_tombstone_channels;
DROP TRIGGER IF EXISTS forum_topic_route_tombstone_visibility_delete
    ON forum_topic_route_tombstone_visibility;
DROP TRIGGER IF EXISTS forum_topic_route_tombstone_visibility_update
    ON forum_topic_route_tombstone_visibility;
DROP INDEX IF EXISTS idx_forum_topic_route_tombstone_channel_lookup;
DROP TABLE IF EXISTS forum_topic_route_tombstone_channels;
DROP TABLE IF EXISTS forum_topic_route_tombstone_visibility;
DROP FUNCTION IF EXISTS forum_reject_topic_route_tombstone_visibility_mutation();
"#;

const SQLITE_UP: &str = r#"
CREATE TABLE IF NOT EXISTS forum_topic_route_tombstone_visibility (
    tenant_id TEXT NOT NULL,
    topic_id TEXT NOT NULL,
    publicly_disclosable INTEGER NOT NULL,
    route_channel_restricted INTEGER NOT NULL,
    created_at TEXT NOT NULL,
    PRIMARY KEY (tenant_id, topic_id),
    FOREIGN KEY (tenant_id, topic_id)
        REFERENCES forum_topics (tenant_id, id)
        ON UPDATE RESTRICT ON DELETE RESTRICT,
    CHECK (topic_id <> '00000000-0000-0000-0000-000000000000'),
    CHECK (publicly_disclosable IN (0, 1)),
    CHECK (route_channel_restricted IN (0, 1))
);

CREATE TABLE IF NOT EXISTS forum_topic_route_tombstone_channels (
    tenant_id TEXT NOT NULL,
    topic_id TEXT NOT NULL,
    channel_slug TEXT NOT NULL,
    PRIMARY KEY (tenant_id, topic_id, channel_slug),
    FOREIGN KEY (tenant_id, topic_id)
        REFERENCES forum_topic_route_tombstone_visibility (tenant_id, topic_id)
        ON UPDATE RESTRICT ON DELETE RESTRICT,
    CHECK (
        length(channel_slug) BETWEEN 1 AND 128
        AND channel_slug = lower(channel_slug)
        AND channel_slug = trim(channel_slug)
        AND instr(channel_slug, char(10)) = 0
        AND instr(channel_slug, char(13)) = 0
    )
);

CREATE INDEX IF NOT EXISTS idx_forum_topic_route_tombstone_channel_lookup
    ON forum_topic_route_tombstone_channels (tenant_id, channel_slug, topic_id);

CREATE TRIGGER IF NOT EXISTS forum_topic_route_tombstone_visibility_update
BEFORE UPDATE ON forum_topic_route_tombstone_visibility
BEGIN
    SELECT RAISE(ABORT, 'forum topic route tombstone visibility is append-only');
END;

CREATE TRIGGER IF NOT EXISTS forum_topic_route_tombstone_visibility_delete
BEFORE DELETE ON forum_topic_route_tombstone_visibility
BEGIN
    SELECT RAISE(ABORT, 'forum topic route tombstone visibility is append-only');
END;

CREATE TRIGGER IF NOT EXISTS forum_topic_route_tombstone_channel_update
BEFORE UPDATE ON forum_topic_route_tombstone_channels
BEGIN
    SELECT RAISE(ABORT, 'forum topic route tombstone visibility is append-only');
END;

CREATE TRIGGER IF NOT EXISTS forum_topic_route_tombstone_channel_delete
BEFORE DELETE ON forum_topic_route_tombstone_channels
BEGIN
    SELECT RAISE(ABORT, 'forum topic route tombstone visibility is append-only');
END;
"#;

const SQLITE_DOWN: &str = r#"
DROP TRIGGER IF EXISTS forum_topic_route_tombstone_channel_delete;
DROP TRIGGER IF EXISTS forum_topic_route_tombstone_channel_update;
DROP TRIGGER IF EXISTS forum_topic_route_tombstone_visibility_delete;
DROP TRIGGER IF EXISTS forum_topic_route_tombstone_visibility_update;
DROP INDEX IF EXISTS idx_forum_topic_route_tombstone_channel_lookup;
DROP TABLE IF EXISTS forum_topic_route_tombstone_channels;
DROP TABLE IF EXISTS forum_topic_route_tombstone_visibility;
"#;
