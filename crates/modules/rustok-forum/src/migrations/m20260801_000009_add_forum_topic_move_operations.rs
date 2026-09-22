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
                "rustok-forum topic move operations migration does not support {backend:?}"
            ))),
        }
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        match manager.get_database_backend() {
            DatabaseBackend::Postgres => execute(manager, POSTGRES_DOWN).await,
            DatabaseBackend::Sqlite => execute(manager, SQLITE_DOWN).await,
            backend => Err(DbErr::Custom(format!(
                "rustok-forum topic move operations migration does not support {backend:?}"
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
CREATE TABLE IF NOT EXISTS forum_topic_move_locks (
    tenant_id UUID NOT NULL PRIMARY KEY,
    touched_at TIMESTAMPTZ NOT NULL
);

CREATE TABLE IF NOT EXISTS forum_topic_move_operations (
    tenant_id UUID NOT NULL,
    operation_id UUID NOT NULL,
    topic_id UUID NOT NULL,
    source_category_id UUID NOT NULL,
    target_category_id UUID NOT NULL,
    actor_id UUID NOT NULL,
    reason VARCHAR(500) NOT NULL,
    published_reply_count INTEGER NOT NULL,
    event_id UUID NOT NULL,
    moved_at TIMESTAMPTZ NOT NULL,
    CONSTRAINT pk_forum_topic_move_operations
        PRIMARY KEY (tenant_id, operation_id),
    CONSTRAINT uq_forum_topic_move_operation_event UNIQUE (event_id),
    CONSTRAINT fk_forum_topic_move_operation_topic
        FOREIGN KEY (tenant_id, topic_id)
        REFERENCES forum_topics (tenant_id, id)
        ON UPDATE CASCADE ON DELETE RESTRICT,
    CONSTRAINT fk_forum_topic_move_operation_source_category
        FOREIGN KEY (tenant_id, source_category_id)
        REFERENCES forum_categories (tenant_id, id)
        ON UPDATE CASCADE ON DELETE RESTRICT,
    CONSTRAINT fk_forum_topic_move_operation_target_category
        FOREIGN KEY (tenant_id, target_category_id)
        REFERENCES forum_categories (tenant_id, id)
        ON UPDATE CASCADE ON DELETE RESTRICT,
    CONSTRAINT fk_forum_topic_move_operation_actor
        FOREIGN KEY (tenant_id, actor_id)
        REFERENCES users (tenant_id, id)
        ON UPDATE CASCADE ON DELETE RESTRICT,
    CONSTRAINT ck_forum_topic_move_operation_ids CHECK (
        operation_id <> '00000000-0000-0000-0000-000000000000'::uuid
        AND topic_id <> '00000000-0000-0000-0000-000000000000'::uuid
        AND source_category_id <> '00000000-0000-0000-0000-000000000000'::uuid
        AND target_category_id <> '00000000-0000-0000-0000-000000000000'::uuid
        AND actor_id <> '00000000-0000-0000-0000-000000000000'::uuid
        AND event_id = operation_id
    ),
    CONSTRAINT ck_forum_topic_move_operation_categories
        CHECK (source_category_id <> target_category_id),
    CONSTRAINT ck_forum_topic_move_operation_reason CHECK (
        length(reason) BETWEEN 1 AND 500
        AND reason = btrim(reason)
        AND position(E'\n' in reason) = 0
        AND position(E'\r' in reason) = 0
    ),
    CONSTRAINT ck_forum_topic_move_operation_reply_count
        CHECK (published_reply_count >= 0)
);

CREATE INDEX IF NOT EXISTS idx_forum_topic_move_operations_history
    ON forum_topic_move_operations (tenant_id, topic_id, moved_at DESC, operation_id);

CREATE OR REPLACE FUNCTION forum_reject_topic_move_operation_mutation()
RETURNS trigger AS $$
BEGIN
    RAISE EXCEPTION 'forum topic move operations are append-only';
    RETURN NULL;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS forum_topic_move_operation_update
    ON forum_topic_move_operations;
CREATE TRIGGER forum_topic_move_operation_update
BEFORE UPDATE ON forum_topic_move_operations
FOR EACH ROW EXECUTE FUNCTION forum_reject_topic_move_operation_mutation();

DROP TRIGGER IF EXISTS forum_topic_move_operation_delete
    ON forum_topic_move_operations;
CREATE TRIGGER forum_topic_move_operation_delete
BEFORE DELETE ON forum_topic_move_operations
FOR EACH ROW EXECUTE FUNCTION forum_reject_topic_move_operation_mutation();
"#;

const POSTGRES_DOWN: &str = r#"
DROP TRIGGER IF EXISTS forum_topic_move_operation_delete
    ON forum_topic_move_operations;
DROP TRIGGER IF EXISTS forum_topic_move_operation_update
    ON forum_topic_move_operations;
DROP TABLE IF EXISTS forum_topic_move_operations;
DROP TABLE IF EXISTS forum_topic_move_locks;
DROP FUNCTION IF EXISTS forum_reject_topic_move_operation_mutation();
"#;

const SQLITE_UP: &str = r#"
CREATE TABLE IF NOT EXISTS forum_topic_move_locks (
    tenant_id TEXT NOT NULL PRIMARY KEY,
    touched_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS forum_topic_move_operations (
    tenant_id TEXT NOT NULL,
    operation_id TEXT NOT NULL,
    topic_id TEXT NOT NULL,
    source_category_id TEXT NOT NULL,
    target_category_id TEXT NOT NULL,
    actor_id TEXT NOT NULL,
    reason TEXT NOT NULL,
    published_reply_count INTEGER NOT NULL,
    event_id TEXT NOT NULL,
    moved_at TEXT NOT NULL,
    PRIMARY KEY (tenant_id, operation_id),
    UNIQUE (event_id),
    FOREIGN KEY (tenant_id, topic_id)
        REFERENCES forum_topics (tenant_id, id)
        ON UPDATE CASCADE ON DELETE RESTRICT,
    FOREIGN KEY (tenant_id, source_category_id)
        REFERENCES forum_categories (tenant_id, id)
        ON UPDATE CASCADE ON DELETE RESTRICT,
    FOREIGN KEY (tenant_id, target_category_id)
        REFERENCES forum_categories (tenant_id, id)
        ON UPDATE CASCADE ON DELETE RESTRICT,
    FOREIGN KEY (tenant_id, actor_id)
        REFERENCES users (tenant_id, id)
        ON UPDATE CASCADE ON DELETE RESTRICT,
    CHECK (
        operation_id <> '00000000-0000-0000-0000-000000000000'
        AND topic_id <> '00000000-0000-0000-0000-000000000000'
        AND source_category_id <> '00000000-0000-0000-0000-000000000000'
        AND target_category_id <> '00000000-0000-0000-0000-000000000000'
        AND actor_id <> '00000000-0000-0000-0000-000000000000'
        AND event_id = operation_id
    ),
    CHECK (source_category_id <> target_category_id),
    CHECK (
        length(reason) BETWEEN 1 AND 500
        AND reason = trim(reason)
        AND instr(reason, char(10)) = 0
        AND instr(reason, char(13)) = 0
    ),
    CHECK (published_reply_count >= 0)
);

CREATE INDEX IF NOT EXISTS idx_forum_topic_move_operations_history
    ON forum_topic_move_operations (tenant_id, topic_id, moved_at DESC, operation_id);

CREATE TRIGGER IF NOT EXISTS forum_topic_move_operation_update
BEFORE UPDATE ON forum_topic_move_operations
BEGIN
    SELECT RAISE(ABORT, 'forum topic move operations are append-only');
END;

CREATE TRIGGER IF NOT EXISTS forum_topic_move_operation_delete
BEFORE DELETE ON forum_topic_move_operations
BEGIN
    SELECT RAISE(ABORT, 'forum topic move operations are append-only');
END;
"#;

const SQLITE_DOWN: &str = r#"
DROP TABLE IF EXISTS forum_topic_move_operations;
DROP TABLE IF EXISTS forum_topic_move_locks;
"#;
