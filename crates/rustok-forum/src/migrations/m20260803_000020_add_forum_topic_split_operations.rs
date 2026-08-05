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
                "rustok-forum topic split operations migration does not support {backend:?}"
            ))),
        }
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        match manager.get_database_backend() {
            DatabaseBackend::Postgres => execute(manager, POSTGRES_DOWN).await,
            DatabaseBackend::Sqlite => execute(manager, SQLITE_DOWN).await,
            backend => Err(DbErr::Custom(format!(
                "rustok-forum topic split operations rollback does not support {backend:?}"
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
CREATE TABLE IF NOT EXISTS forum_topic_split_locks (
    tenant_id UUID NOT NULL PRIMARY KEY,
    touched_at TIMESTAMPTZ NOT NULL
);

CREATE TABLE IF NOT EXISTS forum_topic_split_operations (
    tenant_id UUID NOT NULL,
    operation_id UUID NOT NULL,
    source_topic_id UUID NOT NULL,
    target_topic_id UUID NOT NULL,
    category_id UUID NOT NULL,
    actor_id UUID NOT NULL,
    reason VARCHAR(500) NOT NULL,
    command_fingerprint VARCHAR(64) NOT NULL,
    moved_reply_count INTEGER NOT NULL,
    moved_published_reply_count INTEGER NOT NULL,
    source_resulting_published_reply_count INTEGER NOT NULL,
    target_resulting_published_reply_count INTEGER NOT NULL,
    solution_reply_id UUID,
    event_id UUID NOT NULL,
    split_at TIMESTAMPTZ NOT NULL,
    CONSTRAINT pk_forum_topic_split_operations
        PRIMARY KEY (tenant_id, operation_id),
    CONSTRAINT uq_forum_topic_split_operation_event UNIQUE (event_id),
    CONSTRAINT uq_forum_topic_split_operation_target
        UNIQUE (tenant_id, target_topic_id),
    CONSTRAINT fk_forum_topic_split_operation_source_topic
        FOREIGN KEY (tenant_id, source_topic_id)
        REFERENCES forum_topics (tenant_id, id)
        ON UPDATE CASCADE ON DELETE RESTRICT,
    CONSTRAINT fk_forum_topic_split_operation_target_topic
        FOREIGN KEY (tenant_id, target_topic_id)
        REFERENCES forum_topics (tenant_id, id)
        ON UPDATE CASCADE ON DELETE RESTRICT,
    CONSTRAINT fk_forum_topic_split_operation_category
        FOREIGN KEY (tenant_id, category_id)
        REFERENCES forum_categories (tenant_id, id)
        ON UPDATE CASCADE ON DELETE RESTRICT,
    CONSTRAINT fk_forum_topic_split_operation_actor
        FOREIGN KEY (tenant_id, actor_id)
        REFERENCES users (tenant_id, id)
        ON UPDATE CASCADE ON DELETE RESTRICT,
    CONSTRAINT ck_forum_topic_split_operation_ids CHECK (
        operation_id <> '00000000-0000-0000-0000-000000000000'::uuid
        AND source_topic_id <> '00000000-0000-0000-0000-000000000000'::uuid
        AND target_topic_id <> '00000000-0000-0000-0000-000000000000'::uuid
        AND category_id <> '00000000-0000-0000-0000-000000000000'::uuid
        AND actor_id <> '00000000-0000-0000-0000-000000000000'::uuid
        AND source_topic_id <> target_topic_id
        AND event_id = operation_id
        AND (solution_reply_id IS NULL OR solution_reply_id <> '00000000-0000-0000-0000-000000000000'::uuid)
    ),
    CONSTRAINT ck_forum_topic_split_operation_reason CHECK (
        length(reason) BETWEEN 1 AND 500
        AND reason = btrim(reason)
        AND position(E'\n' in reason) = 0
        AND position(E'\r' in reason) = 0
    ),
    CONSTRAINT ck_forum_topic_split_operation_fingerprint CHECK (
        command_fingerprint ~ '^[0-9a-f]{64}$'
    ),
    CONSTRAINT ck_forum_topic_split_operation_counts CHECK (
        moved_reply_count BETWEEN 1 AND 500
        AND moved_published_reply_count BETWEEN 0 AND moved_reply_count
        AND source_resulting_published_reply_count >= 0
        AND target_resulting_published_reply_count = moved_published_reply_count
    )
);

CREATE TABLE IF NOT EXISTS forum_topic_split_reply_items (
    tenant_id UUID NOT NULL,
    operation_id UUID NOT NULL,
    reply_id UUID NOT NULL,
    source_position BIGINT NOT NULL,
    target_position BIGINT NOT NULL,
    was_published BOOLEAN NOT NULL,
    CONSTRAINT pk_forum_topic_split_reply_items
        PRIMARY KEY (tenant_id, operation_id, reply_id),
    CONSTRAINT uq_forum_topic_split_reply_target_position
        UNIQUE (tenant_id, operation_id, target_position),
    CONSTRAINT fk_forum_topic_split_reply_operation
        FOREIGN KEY (tenant_id, operation_id)
        REFERENCES forum_topic_split_operations (tenant_id, operation_id)
        ON UPDATE CASCADE ON DELETE RESTRICT,
    CONSTRAINT fk_forum_topic_split_reply_identity
        FOREIGN KEY (tenant_id, reply_id)
        REFERENCES forum_replies (tenant_id, id)
        ON UPDATE CASCADE ON DELETE RESTRICT,
    CONSTRAINT ck_forum_topic_split_reply_positions CHECK (
        source_position > 0
        AND target_position BETWEEN 1 AND 500
    )
);

CREATE INDEX IF NOT EXISTS idx_forum_topic_split_source_history
    ON forum_topic_split_operations (
        tenant_id, source_topic_id, split_at DESC, operation_id
    );
CREATE INDEX IF NOT EXISTS idx_forum_topic_split_reply_history
    ON forum_topic_split_reply_items (tenant_id, reply_id, operation_id);

CREATE OR REPLACE FUNCTION forum_reject_topic_split_audit_mutation()
RETURNS trigger AS $$
BEGIN
    RAISE EXCEPTION 'forum topic split audit is append-only';
    RETURN NULL;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS forum_topic_split_operation_update
    ON forum_topic_split_operations;
CREATE TRIGGER forum_topic_split_operation_update
BEFORE UPDATE ON forum_topic_split_operations
FOR EACH ROW EXECUTE FUNCTION forum_reject_topic_split_audit_mutation();

DROP TRIGGER IF EXISTS forum_topic_split_operation_delete
    ON forum_topic_split_operations;
CREATE TRIGGER forum_topic_split_operation_delete
BEFORE DELETE ON forum_topic_split_operations
FOR EACH ROW EXECUTE FUNCTION forum_reject_topic_split_audit_mutation();

DROP TRIGGER IF EXISTS forum_topic_split_reply_item_update
    ON forum_topic_split_reply_items;
CREATE TRIGGER forum_topic_split_reply_item_update
BEFORE UPDATE ON forum_topic_split_reply_items
FOR EACH ROW EXECUTE FUNCTION forum_reject_topic_split_audit_mutation();

DROP TRIGGER IF EXISTS forum_topic_split_reply_item_delete
    ON forum_topic_split_reply_items;
CREATE TRIGGER forum_topic_split_reply_item_delete
BEFORE DELETE ON forum_topic_split_reply_items
FOR EACH ROW EXECUTE FUNCTION forum_reject_topic_split_audit_mutation();
"#;

const POSTGRES_DOWN: &str = r#"
DROP TRIGGER IF EXISTS forum_topic_split_reply_item_delete
    ON forum_topic_split_reply_items;
DROP TRIGGER IF EXISTS forum_topic_split_reply_item_update
    ON forum_topic_split_reply_items;
DROP TRIGGER IF EXISTS forum_topic_split_operation_delete
    ON forum_topic_split_operations;
DROP TRIGGER IF EXISTS forum_topic_split_operation_update
    ON forum_topic_split_operations;
DROP TABLE IF EXISTS forum_topic_split_reply_items;
DROP TABLE IF EXISTS forum_topic_split_operations;
DROP TABLE IF EXISTS forum_topic_split_locks;
DROP FUNCTION IF EXISTS forum_reject_topic_split_audit_mutation();
"#;

const SQLITE_UP: &str = r#"
CREATE TABLE IF NOT EXISTS forum_topic_split_locks (
    tenant_id TEXT NOT NULL PRIMARY KEY,
    touched_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS forum_topic_split_operations (
    tenant_id TEXT NOT NULL,
    operation_id TEXT NOT NULL,
    source_topic_id TEXT NOT NULL,
    target_topic_id TEXT NOT NULL,
    category_id TEXT NOT NULL,
    actor_id TEXT NOT NULL,
    reason TEXT NOT NULL,
    command_fingerprint TEXT NOT NULL,
    moved_reply_count INTEGER NOT NULL,
    moved_published_reply_count INTEGER NOT NULL,
    source_resulting_published_reply_count INTEGER NOT NULL,
    target_resulting_published_reply_count INTEGER NOT NULL,
    solution_reply_id TEXT,
    event_id TEXT NOT NULL,
    split_at TEXT NOT NULL,
    PRIMARY KEY (tenant_id, operation_id),
    UNIQUE (event_id),
    UNIQUE (tenant_id, target_topic_id),
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
        AND source_topic_id <> target_topic_id
        AND event_id = operation_id
        AND (solution_reply_id IS NULL OR solution_reply_id <> '00000000-0000-0000-0000-000000000000')
    ),
    CHECK (
        length(reason) BETWEEN 1 AND 500
        AND reason = trim(reason)
        AND instr(reason, char(10)) = 0
        AND instr(reason, char(13)) = 0
    ),
    CHECK (
        length(command_fingerprint) = 64
        AND command_fingerprint NOT GLOB '*[^0-9a-f]*'
    ),
    CHECK (
        moved_reply_count BETWEEN 1 AND 500
        AND moved_published_reply_count BETWEEN 0 AND moved_reply_count
        AND source_resulting_published_reply_count >= 0
        AND target_resulting_published_reply_count = moved_published_reply_count
    )
);

CREATE TABLE IF NOT EXISTS forum_topic_split_reply_items (
    tenant_id TEXT NOT NULL,
    operation_id TEXT NOT NULL,
    reply_id TEXT NOT NULL,
    source_position INTEGER NOT NULL,
    target_position INTEGER NOT NULL,
    was_published INTEGER NOT NULL CHECK (was_published IN (0, 1)),
    PRIMARY KEY (tenant_id, operation_id, reply_id),
    UNIQUE (tenant_id, operation_id, target_position),
    FOREIGN KEY (tenant_id, operation_id)
        REFERENCES forum_topic_split_operations (tenant_id, operation_id)
        ON UPDATE CASCADE ON DELETE RESTRICT,
    FOREIGN KEY (tenant_id, reply_id)
        REFERENCES forum_replies (tenant_id, id)
        ON UPDATE CASCADE ON DELETE RESTRICT,
    CHECK (
        source_position > 0
        AND target_position BETWEEN 1 AND 500
    )
);

CREATE INDEX IF NOT EXISTS idx_forum_topic_split_source_history
    ON forum_topic_split_operations (
        tenant_id, source_topic_id, split_at DESC, operation_id
    );
CREATE INDEX IF NOT EXISTS idx_forum_topic_split_reply_history
    ON forum_topic_split_reply_items (tenant_id, reply_id, operation_id);

CREATE TRIGGER IF NOT EXISTS forum_topic_split_operation_update
BEFORE UPDATE ON forum_topic_split_operations
BEGIN
    SELECT RAISE(ABORT, 'forum topic split audit is append-only');
END;

CREATE TRIGGER IF NOT EXISTS forum_topic_split_operation_delete
BEFORE DELETE ON forum_topic_split_operations
BEGIN
    SELECT RAISE(ABORT, 'forum topic split audit is append-only');
END;

CREATE TRIGGER IF NOT EXISTS forum_topic_split_reply_item_update
BEFORE UPDATE ON forum_topic_split_reply_items
BEGIN
    SELECT RAISE(ABORT, 'forum topic split audit is append-only');
END;

CREATE TRIGGER IF NOT EXISTS forum_topic_split_reply_item_delete
BEFORE DELETE ON forum_topic_split_reply_items
BEGIN
    SELECT RAISE(ABORT, 'forum topic split audit is append-only');
END;
"#;

const SQLITE_DOWN: &str = r#"
DROP TABLE IF EXISTS forum_topic_split_reply_items;
DROP TABLE IF EXISTS forum_topic_split_operations;
DROP TABLE IF EXISTS forum_topic_split_locks;
"#;
