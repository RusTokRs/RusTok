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
                "rustok-forum topic route identity migration does not support {backend:?}"
            ))),
        }
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        match manager.get_database_backend() {
            DatabaseBackend::Postgres => execute(manager, POSTGRES_DOWN).await,
            DatabaseBackend::Sqlite => execute(manager, SQLITE_DOWN).await,
            backend => Err(DbErr::Custom(format!(
                "rustok-forum topic route identity rollback does not support {backend:?}"
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
CREATE TABLE IF NOT EXISTS forum_topic_route_aliases (
    tenant_id UUID NOT NULL,
    alias_id UUID NOT NULL,
    topic_id UUID NOT NULL,
    locale VARCHAR(64) NOT NULL,
    short_id VARCHAR(12) NOT NULL,
    slug VARCHAR(255) NOT NULL,
    disposition VARCHAR(16) NOT NULL,
    target_topic_id UUID NULL,
    target_locale VARCHAR(64) NULL,
    reason VARCHAR(500) NOT NULL,
    created_at TIMESTAMPTZ NOT NULL,
    CONSTRAINT pk_forum_topic_route_aliases
        PRIMARY KEY (tenant_id, alias_id),
    CONSTRAINT uq_forum_topic_route_alias
        UNIQUE (tenant_id, locale, short_id, slug),
    CONSTRAINT fk_forum_topic_route_alias_source
        FOREIGN KEY (tenant_id, topic_id)
        REFERENCES forum_topics (tenant_id, id)
        ON UPDATE RESTRICT ON DELETE RESTRICT,
    CONSTRAINT fk_forum_topic_route_alias_target
        FOREIGN KEY (tenant_id, target_topic_id)
        REFERENCES forum_topics (tenant_id, id)
        ON UPDATE RESTRICT ON DELETE RESTRICT,
    CONSTRAINT ck_forum_topic_route_alias_ids CHECK (
        alias_id <> '00000000-0000-0000-0000-000000000000'::uuid
        AND topic_id <> '00000000-0000-0000-0000-000000000000'::uuid
        AND (
            target_topic_id IS NULL
            OR target_topic_id <> '00000000-0000-0000-0000-000000000000'::uuid
        )
    ),
    CONSTRAINT ck_forum_topic_route_alias_locale CHECK (
        length(locale) BETWEEN 1 AND 64
        AND locale = btrim(locale)
        AND position(E'\n' in locale) = 0
        AND position(E'\r' in locale) = 0
    ),
    CONSTRAINT ck_forum_topic_route_alias_short_id CHECK (
        short_id ~ '^[0-9a-f]{12}$'
    ),
    CONSTRAINT ck_forum_topic_route_alias_slug CHECK (
        length(slug) BETWEEN 1 AND 255
        AND slug = lower(slug)
        AND slug = btrim(slug)
        AND slug ~ '^[a-z0-9]+(?:-[a-z0-9]+)*$'
    ),
    CONSTRAINT ck_forum_topic_route_alias_disposition CHECK (
        (
            disposition = 'redirect'
            AND target_topic_id IS NOT NULL
            AND target_locale IS NOT NULL
            AND length(target_locale) BETWEEN 1 AND 64
            AND target_locale = btrim(target_locale)
        )
        OR (
            disposition = 'gone'
            AND target_topic_id IS NULL
            AND target_locale IS NULL
        )
    ),
    CONSTRAINT ck_forum_topic_route_alias_reason CHECK (
        length(reason) BETWEEN 1 AND 500
        AND reason = btrim(reason)
        AND position(E'\n' in reason) = 0
        AND position(E'\r' in reason) = 0
    )
);

CREATE INDEX IF NOT EXISTS idx_forum_topic_route_alias_target
    ON forum_topic_route_aliases (
        tenant_id, target_topic_id, target_locale, created_at, alias_id
    );
CREATE INDEX IF NOT EXISTS idx_forum_topic_merge_operations_route_backfill
    ON forum_topic_merge_operations (tenant_id, merged_at, operation_id);

CREATE OR REPLACE FUNCTION forum_reject_topic_route_alias_mutation()
RETURNS trigger AS $$
BEGIN
    RAISE EXCEPTION 'forum topic route aliases are append-only';
    RETURN NULL;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS forum_topic_route_alias_update
    ON forum_topic_route_aliases;
CREATE TRIGGER forum_topic_route_alias_update
BEFORE UPDATE ON forum_topic_route_aliases
FOR EACH ROW EXECUTE FUNCTION forum_reject_topic_route_alias_mutation();

DROP TRIGGER IF EXISTS forum_topic_route_alias_delete
    ON forum_topic_route_aliases;
CREATE TRIGGER forum_topic_route_alias_delete
BEFORE DELETE ON forum_topic_route_aliases
FOR EACH ROW EXECUTE FUNCTION forum_reject_topic_route_alias_mutation();
"#;

const POSTGRES_DOWN: &str = r#"
DROP TRIGGER IF EXISTS forum_topic_route_alias_delete
    ON forum_topic_route_aliases;
DROP TRIGGER IF EXISTS forum_topic_route_alias_update
    ON forum_topic_route_aliases;
DROP INDEX IF EXISTS idx_forum_topic_merge_operations_route_backfill;
DROP TABLE IF EXISTS forum_topic_route_aliases;
DROP FUNCTION IF EXISTS forum_reject_topic_route_alias_mutation();
"#;

const SQLITE_UP: &str = r#"
CREATE TABLE IF NOT EXISTS forum_topic_route_aliases (
    tenant_id TEXT NOT NULL,
    alias_id TEXT NOT NULL,
    topic_id TEXT NOT NULL,
    locale TEXT NOT NULL,
    short_id TEXT NOT NULL,
    slug TEXT NOT NULL,
    disposition TEXT NOT NULL,
    target_topic_id TEXT NULL,
    target_locale TEXT NULL,
    reason TEXT NOT NULL,
    created_at TEXT NOT NULL,
    PRIMARY KEY (tenant_id, alias_id),
    UNIQUE (tenant_id, locale, short_id, slug),
    FOREIGN KEY (tenant_id, topic_id)
        REFERENCES forum_topics (tenant_id, id)
        ON UPDATE RESTRICT ON DELETE RESTRICT,
    FOREIGN KEY (tenant_id, target_topic_id)
        REFERENCES forum_topics (tenant_id, id)
        ON UPDATE RESTRICT ON DELETE RESTRICT,
    CHECK (
        alias_id <> '00000000-0000-0000-0000-000000000000'
        AND topic_id <> '00000000-0000-0000-0000-000000000000'
        AND (
            target_topic_id IS NULL
            OR target_topic_id <> '00000000-0000-0000-0000-000000000000'
        )
    ),
    CHECK (
        length(locale) BETWEEN 1 AND 64
        AND locale = trim(locale)
        AND instr(locale, char(10)) = 0
        AND instr(locale, char(13)) = 0
    ),
    CHECK (
        length(short_id) = 12
        AND short_id = lower(short_id)
        AND short_id NOT GLOB '*[^0-9a-f]*'
    ),
    CHECK (
        length(slug) BETWEEN 1 AND 255
        AND slug = lower(slug)
        AND slug = trim(slug)
        AND slug NOT GLOB '*[^a-z0-9-]*'
        AND slug NOT LIKE '-%'
        AND slug NOT LIKE '%-'
        AND slug NOT LIKE '%--%'
    ),
    CHECK (
        (
            disposition = 'redirect'
            AND target_topic_id IS NOT NULL
            AND target_locale IS NOT NULL
            AND length(target_locale) BETWEEN 1 AND 64
            AND target_locale = trim(target_locale)
        )
        OR (
            disposition = 'gone'
            AND target_topic_id IS NULL
            AND target_locale IS NULL
        )
    ),
    CHECK (
        length(reason) BETWEEN 1 AND 500
        AND reason = trim(reason)
        AND instr(reason, char(10)) = 0
        AND instr(reason, char(13)) = 0
    )
);

CREATE INDEX IF NOT EXISTS idx_forum_topic_route_alias_target
    ON forum_topic_route_aliases (
        tenant_id, target_topic_id, target_locale, created_at, alias_id
    );
CREATE INDEX IF NOT EXISTS idx_forum_topic_merge_operations_route_backfill
    ON forum_topic_merge_operations (tenant_id, merged_at, operation_id);

CREATE TRIGGER IF NOT EXISTS forum_topic_route_alias_update
BEFORE UPDATE ON forum_topic_route_aliases
BEGIN
    SELECT RAISE(ABORT, 'forum topic route aliases are append-only');
END;

CREATE TRIGGER IF NOT EXISTS forum_topic_route_alias_delete
BEFORE DELETE ON forum_topic_route_aliases
BEGIN
    SELECT RAISE(ABORT, 'forum topic route aliases are append-only');
END;
"#;

const SQLITE_DOWN: &str = r#"
DROP TRIGGER IF EXISTS forum_topic_route_alias_delete;
DROP TRIGGER IF EXISTS forum_topic_route_alias_update;
DROP INDEX IF EXISTS idx_forum_topic_merge_operations_route_backfill;
DROP TABLE IF EXISTS forum_topic_route_aliases;
"#;
