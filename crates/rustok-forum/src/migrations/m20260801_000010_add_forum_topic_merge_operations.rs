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
                "rustok-forum topic merge operations migration does not support {backend:?}"
            ))),
        }
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        match manager.get_database_backend() {
            DatabaseBackend::Postgres => execute(manager, POSTGRES_DOWN).await,
            DatabaseBackend::Sqlite => execute(manager, SQLITE_DOWN).await,
            backend => Err(DbErr::Custom(format!(
                "rustok-forum topic merge operations migration does not support {backend:?}"
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
CREATE TABLE IF NOT EXISTS forum_topic_merge_locks (
    tenant_id UUID NOT NULL PRIMARY KEY,
    touched_at TIMESTAMPTZ NOT NULL
);

CREATE TABLE IF NOT EXISTS forum_topic_merge_operations (
    tenant_id UUID NOT NULL,
    operation_id UUID NOT NULL,
    source_topic_id UUID NOT NULL,
    target_topic_id UUID NOT NULL,
    category_id UUID NOT NULL,
    actor_id UUID NOT NULL,
    reason VARCHAR(500) NOT NULL,
    moved_reply_count INTEGER NOT NULL,
    moved_published_reply_count INTEGER NOT NULL,
    resulting_published_reply_count INTEGER NOT NULL,
    position_offset BIGINT NOT NULL,
    event_id UUID NOT NULL,
    merged_at TIMESTAMPTZ NOT NULL,
    CONSTRAINT pk_forum_topic_merge_operations
        PRIMARY KEY (tenant_id, operation_id),
    CONSTRAINT uq_forum_topic_merge_operation_event UNIQUE (event_id),
    CONSTRAINT fk_forum_topic_merge_operation_source_topic
        FOREIGN KEY (tenant_id, source_topic_id)
        REFERENCES forum_topics (tenant_id, id)
        ON UPDATE CASCADE ON DELETE RESTRICT,
    CONSTRAINT fk_forum_topic_merge_operation_target_topic
        FOREIGN KEY (tenant_id, target_topic_id)
        REFERENCES forum_topics (tenant_id, id)
        ON UPDATE CASCADE ON DELETE RESTRICT,
    CONSTRAINT fk_forum_topic_merge_operation_category
        FOREIGN KEY (tenant_id, category_id)
        REFERENCES forum_categories (tenant_id, id)
        ON UPDATE CASCADE ON DELETE RESTRICT,
    CONSTRAINT fk_forum_topic_merge_operation_actor
        FOREIGN KEY (tenant_id, actor_id)
        REFERENCES users (tenant_id, id)
        ON UPDATE CASCADE ON DELETE RESTRICT,
    CONSTRAINT ck_forum_topic_merge_operation_ids CHECK (
        operation_id <> '00000000-0000-0000-0000-000000000000'::uuid
        AND source_topic_id <> '00000000-0000-0000-0000-000000000000'::uuid
        AND target_topic_id <> '00000000-0000-0000-0000-000000000000'::uuid
        AND category_id <> '00000000-0000-0000-0000-000000000000'::uuid
        AND actor_id <> '00000000-0000-0000-0000-000000000000'::uuid
        AND event_id = operation_id
    ),
    CONSTRAINT ck_forum_topic_merge_operation_topics
        CHECK (source_topic_id <> target_topic_id),
    CONSTRAINT ck_forum_topic_merge_operation_reason CHECK (
        length(reason) BETWEEN 1 AND 500
        AND reason = btrim(reason)
        AND position(E'\n' in reason) = 0
        AND position(E'\r' in reason) = 0
    ),
    CONSTRAINT ck_forum_topic_merge_operation_counts CHECK (
        moved_reply_count BETWEEN 0 AND 500
        AND moved_published_reply_count BETWEEN 0 AND moved_reply_count
        AND resulting_published_reply_count >= moved_published_reply_count
        AND position_offset >= 0
    )
);

CREATE INDEX IF NOT EXISTS idx_forum_topic_merge_operations_source_history
    ON forum_topic_merge_operations (
        tenant_id, source_topic_id, merged_at DESC, operation_id
    );
CREATE INDEX IF NOT EXISTS idx_forum_topic_merge_operations_target_history
    ON forum_topic_merge_operations (
        tenant_id, target_topic_id, merged_at DESC, operation_id
    );

CREATE OR REPLACE FUNCTION forum_reject_topic_merge_operation_mutation()
RETURNS trigger AS $$
BEGIN
    RAISE EXCEPTION 'forum topic merge operations are append-only';
    RETURN NULL;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS forum_topic_merge_operation_update
    ON forum_topic_merge_operations;
CREATE TRIGGER forum_topic_merge_operation_update
BEFORE UPDATE ON forum_topic_merge_operations
FOR EACH ROW EXECUTE FUNCTION forum_reject_topic_merge_operation_mutation();

DROP TRIGGER IF EXISTS forum_topic_merge_operation_delete
    ON forum_topic_merge_operations;
CREATE TRIGGER forum_topic_merge_operation_delete
BEFORE DELETE ON forum_topic_merge_operations
FOR EACH ROW EXECUTE FUNCTION forum_reject_topic_merge_operation_mutation();
"#;

const POSTGRES_DOWN: &str = r#"
DROP TRIGGER IF EXISTS forum_topic_merge_operation_delete
    ON forum_topic_merge_operations;
DROP TRIGGER IF EXISTS forum_topic_merge_operation_update
    ON forum_topic_merge_operations;
DROP TABLE IF EXISTS forum_topic_merge_operations;
DROP TABLE IF EXISTS forum_topic_merge_locks;
DROP FUNCTION IF EXISTS forum_reject_topic_merge_operation_mutation();
"#;

const SQLITE_UP: &str = r#"
CREATE TABLE IF NOT EXISTS forum_topic_merge_locks (
    tenant_id TEXT NOT NULL PRIMARY KEY,
    touched_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS forum_topic_merge_operations (
    tenant_id TEXT NOT NULL,
    operation_id TEXT NOT NULL,
    source_topic_id TEXT NOT NULL,
    target_topic_id TEXT NOT NULL,
    category_id TEXT NOT NULL,
    actor_id TEXT NOT NULL,
    reason TEXT NOT NULL,
    moved_reply_count INTEGER NOT NULL,
    moved_published_reply_count INTEGER NOT NULL,
    resulting_published_reply_count INTEGER NOT NULL,
    position_offset INTEGER NOT NULL,
    event_id TEXT NOT NULL,
    merged_at TEXT NOT NULL,
    PRIMARY KEY (tenant_id, operation_id),
    UNIQUE (event_id),
    FOREIGN KEY (tenant_id, source_topic_id)
        REFERENCES forum_topics (tenant_id, id)
        ON UPDATE CASCADE ON DELETE RESTRICT,
    FOREIGN KEY (tenant_id, target_topic_id)
        REFERENCES forum_topics (tenant_id, id)
        ON UPDATE CASCADE ON DELETE RESTRICT,
    FOREIGN KEY (tenant_id, category_id)
        REFERENCES forum_categories (tenant_id, id)
        ON UPDATE CASCADE ON DELETE RESTRICT,
    FOREIGN KEY (tenant_id, actor_id)
        REFERENCES users (tenant_id, id)
        ON UPDATE CASCADE ON DELETE RESTRICT,
    CHECK (
        operation_id <> '00000000-0000-0000-0000-000000000000'
        AND source_topic_id <> '00000000-0000-0000-0000-000000000000'
        AND target_topic_id <> '00000000-0000-0000-0000-000000000000'
        AND category_id <> '00000000-0000-0000-0000-000000000000'
        AND actor_id <> '00000000-0000-0000-0000-000000000000'
        AND event_id = operation_id
    ),
    CHECK (source_topic_id <> target_topic_id),
    CHECK (
        length(reason) BETWEEN 1 AND 500
        AND reason = trim(reason)
        AND instr(reason, char(10)) = 0
        AND instr(reason, char(13)) = 0
    ),
    CHECK (
        moved_reply_count BETWEEN 0 AND 500
        AND moved_published_reply_count BETWEEN 0 AND moved_reply_count
        AND resulting_published_reply_count >= moved_published_reply_count
        AND position_offset >= 0
    )
);

CREATE INDEX IF NOT EXISTS idx_forum_topic_merge_operations_source_history
    ON forum_topic_merge_operations (
        tenant_id, source_topic_id, merged_at DESC, operation_id
    );
CREATE INDEX IF NOT EXISTS idx_forum_topic_merge_operations_target_history
    ON forum_topic_merge_operations (
        tenant_id, target_topic_id, merged_at DESC, operation_id
    );

CREATE TRIGGER IF NOT EXISTS forum_topic_merge_operation_update
BEFORE UPDATE ON forum_topic_merge_operations
BEGIN
    SELECT RAISE(ABORT, 'forum topic merge operations are append-only');
END;

CREATE TRIGGER IF NOT EXISTS forum_topic_merge_operation_delete
BEFORE DELETE ON forum_topic_merge_operations
BEGIN
    SELECT RAISE(ABORT, 'forum topic merge operations are append-only');
END;
"#;

const SQLITE_DOWN: &str = r#"
DROP TABLE IF EXISTS forum_topic_merge_operations;
DROP TABLE IF EXISTS forum_topic_merge_locks;
"#;
