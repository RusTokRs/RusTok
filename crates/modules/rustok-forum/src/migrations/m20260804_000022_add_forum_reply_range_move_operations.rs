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
                "rustok-forum reply range move migration does not support {backend:?}"
            ))),
        }
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        match manager.get_database_backend() {
            DatabaseBackend::Postgres => execute(manager, POSTGRES_DOWN).await,
            DatabaseBackend::Sqlite => execute(manager, SQLITE_DOWN).await,
            backend => Err(DbErr::Custom(format!(
                "rustok-forum reply range move rollback does not support {backend:?}"
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
CREATE TABLE IF NOT EXISTS forum_reply_range_move_locks (
    tenant_id UUID NOT NULL PRIMARY KEY,
    touched_at TIMESTAMPTZ NOT NULL
);

CREATE TABLE IF NOT EXISTS forum_reply_range_move_operations (
    tenant_id UUID NOT NULL,
    operation_id UUID NOT NULL,
    source_topic_id UUID NOT NULL,
    target_topic_id UUID NOT NULL,
    source_category_id UUID NOT NULL,
    target_category_id UUID NOT NULL,
    actor_id UUID NOT NULL,
    reason VARCHAR(500) NOT NULL,
    command_fingerprint VARCHAR(64) NOT NULL,
    source_start_position BIGINT NOT NULL,
    source_end_position BIGINT NOT NULL,
    target_start_position BIGINT NOT NULL,
    target_end_position BIGINT NOT NULL,
    moved_reply_count INTEGER NOT NULL,
    moved_published_reply_count INTEGER NOT NULL,
    source_resulting_published_reply_count INTEGER NOT NULL,
    target_resulting_published_reply_count INTEGER NOT NULL,
    moved_solution_reply_id UUID NULL,
    source_resulting_solution_reply_id UUID NULL,
    target_resulting_solution_reply_id UUID NULL,
    event_id UUID NOT NULL,
    moved_at TIMESTAMPTZ NOT NULL,
    CONSTRAINT pk_forum_reply_range_move_operations
        PRIMARY KEY (tenant_id, operation_id),
    CONSTRAINT uq_forum_reply_range_move_event UNIQUE (event_id),
    CONSTRAINT fk_forum_reply_range_move_source_topic
        FOREIGN KEY (tenant_id, source_topic_id)
        REFERENCES forum_topics (tenant_id, id)
        ON UPDATE RESTRICT ON DELETE RESTRICT,
    CONSTRAINT fk_forum_reply_range_move_target_topic
        FOREIGN KEY (tenant_id, target_topic_id)
        REFERENCES forum_topics (tenant_id, id)
        ON UPDATE RESTRICT ON DELETE RESTRICT,
    CONSTRAINT fk_forum_reply_range_move_source_category
        FOREIGN KEY (tenant_id, source_category_id)
        REFERENCES forum_categories (tenant_id, id)
        ON UPDATE RESTRICT ON DELETE RESTRICT,
    CONSTRAINT fk_forum_reply_range_move_target_category
        FOREIGN KEY (tenant_id, target_category_id)
        REFERENCES forum_categories (tenant_id, id)
        ON UPDATE RESTRICT ON DELETE RESTRICT,
    CONSTRAINT fk_forum_reply_range_move_actor
        FOREIGN KEY (tenant_id, actor_id)
        REFERENCES users (tenant_id, id)
        ON UPDATE RESTRICT ON DELETE RESTRICT,
    CONSTRAINT ck_forum_reply_range_move_ids CHECK (
        operation_id <> '00000000-0000-0000-0000-000000000000'::uuid
        AND source_topic_id <> '00000000-0000-0000-0000-000000000000'::uuid
        AND target_topic_id <> '00000000-0000-0000-0000-000000000000'::uuid
        AND source_category_id <> '00000000-0000-0000-0000-000000000000'::uuid
        AND target_category_id <> '00000000-0000-0000-0000-000000000000'::uuid
        AND actor_id <> '00000000-0000-0000-0000-000000000000'::uuid
        AND source_topic_id <> target_topic_id
        AND event_id = operation_id
    ),
    CONSTRAINT ck_forum_reply_range_move_reason CHECK (
        length(reason) BETWEEN 1 AND 500
        AND reason = btrim(reason)
        AND position(E'\n' in reason) = 0
        AND position(E'\r' in reason) = 0
    ),
    CONSTRAINT ck_forum_reply_range_move_fingerprint CHECK (
        length(command_fingerprint) = 64
        AND command_fingerprint = lower(command_fingerprint)
    ),
    CONSTRAINT ck_forum_reply_range_move_positions CHECK (
        source_start_position > 0
        AND source_end_position >= source_start_position
        AND target_start_position > 0
        AND target_end_position >= target_start_position
        AND target_end_position = target_start_position + moved_reply_count - 1
    ),
    CONSTRAINT ck_forum_reply_range_move_counts CHECK (
        moved_reply_count BETWEEN 1 AND 500
        AND moved_published_reply_count BETWEEN 0 AND moved_reply_count
        AND source_resulting_published_reply_count >= 0
        AND target_resulting_published_reply_count >= moved_published_reply_count
    )
);

CREATE INDEX IF NOT EXISTS idx_forum_reply_range_move_source_history
    ON forum_reply_range_move_operations (
        tenant_id, source_topic_id, moved_at DESC, operation_id
    );
CREATE INDEX IF NOT EXISTS idx_forum_reply_range_move_target_history
    ON forum_reply_range_move_operations (
        tenant_id, target_topic_id, moved_at DESC, operation_id
    );

CREATE TABLE IF NOT EXISTS forum_reply_range_move_items (
    tenant_id UUID NOT NULL,
    operation_id UUID NOT NULL,
    reply_id UUID NOT NULL,
    source_parent_reply_id UUID NULL,
    target_parent_reply_id UUID NULL,
    source_position BIGINT NOT NULL,
    target_position BIGINT NOT NULL,
    was_published BOOLEAN NOT NULL,
    moved_at TIMESTAMPTZ NOT NULL,
    CONSTRAINT pk_forum_reply_range_move_items
        PRIMARY KEY (tenant_id, operation_id, reply_id),
    CONSTRAINT uq_forum_reply_range_move_source_position
        UNIQUE (tenant_id, operation_id, source_position),
    CONSTRAINT uq_forum_reply_range_move_target_position
        UNIQUE (tenant_id, operation_id, target_position),
    CONSTRAINT fk_forum_reply_range_move_item_operation
        FOREIGN KEY (tenant_id, operation_id)
        REFERENCES forum_reply_range_move_operations (tenant_id, operation_id)
        ON UPDATE RESTRICT ON DELETE RESTRICT,
    CONSTRAINT fk_forum_reply_range_move_item_reply
        FOREIGN KEY (tenant_id, reply_id)
        REFERENCES forum_replies (tenant_id, id)
        ON UPDATE RESTRICT ON DELETE RESTRICT,
    CONSTRAINT ck_forum_reply_range_move_item_ids CHECK (
        reply_id <> '00000000-0000-0000-0000-000000000000'::uuid
    ),
    CONSTRAINT ck_forum_reply_range_move_item_positions CHECK (
        source_position > 0 AND target_position > 0
    )
);

CREATE OR REPLACE FUNCTION forum_reject_reply_range_move_audit_mutation()
RETURNS trigger AS $$
BEGIN
    RAISE EXCEPTION 'forum reply range move audit is append-only';
    RETURN NULL;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS forum_reply_range_move_operation_update
    ON forum_reply_range_move_operations;
CREATE TRIGGER forum_reply_range_move_operation_update
BEFORE UPDATE ON forum_reply_range_move_operations
FOR EACH ROW EXECUTE FUNCTION forum_reject_reply_range_move_audit_mutation();

DROP TRIGGER IF EXISTS forum_reply_range_move_operation_delete
    ON forum_reply_range_move_operations;
CREATE TRIGGER forum_reply_range_move_operation_delete
BEFORE DELETE ON forum_reply_range_move_operations
FOR EACH ROW EXECUTE FUNCTION forum_reject_reply_range_move_audit_mutation();

DROP TRIGGER IF EXISTS forum_reply_range_move_item_update
    ON forum_reply_range_move_items;
CREATE TRIGGER forum_reply_range_move_item_update
BEFORE UPDATE ON forum_reply_range_move_items
FOR EACH ROW EXECUTE FUNCTION forum_reject_reply_range_move_audit_mutation();

DROP TRIGGER IF EXISTS forum_reply_range_move_item_delete
    ON forum_reply_range_move_items;
CREATE TRIGGER forum_reply_range_move_item_delete
BEFORE DELETE ON forum_reply_range_move_items
FOR EACH ROW EXECUTE FUNCTION forum_reject_reply_range_move_audit_mutation();
"#;

const POSTGRES_DOWN: &str = r#"
DROP TRIGGER IF EXISTS forum_reply_range_move_item_delete
    ON forum_reply_range_move_items;
DROP TRIGGER IF EXISTS forum_reply_range_move_item_update
    ON forum_reply_range_move_items;
DROP TRIGGER IF EXISTS forum_reply_range_move_operation_delete
    ON forum_reply_range_move_operations;
DROP TRIGGER IF EXISTS forum_reply_range_move_operation_update
    ON forum_reply_range_move_operations;
DROP TABLE IF EXISTS forum_reply_range_move_items;
DROP TABLE IF EXISTS forum_reply_range_move_operations;
DROP TABLE IF EXISTS forum_reply_range_move_locks;
DROP FUNCTION IF EXISTS forum_reject_reply_range_move_audit_mutation();
"#;

const SQLITE_UP: &str = r#"
CREATE TABLE IF NOT EXISTS forum_reply_range_move_locks (
    tenant_id TEXT NOT NULL PRIMARY KEY,
    touched_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS forum_reply_range_move_operations (
    tenant_id TEXT NOT NULL,
    operation_id TEXT NOT NULL,
    source_topic_id TEXT NOT NULL,
    target_topic_id TEXT NOT NULL,
    source_category_id TEXT NOT NULL,
    target_category_id TEXT NOT NULL,
    actor_id TEXT NOT NULL,
    reason TEXT NOT NULL,
    command_fingerprint TEXT NOT NULL,
    source_start_position INTEGER NOT NULL,
    source_end_position INTEGER NOT NULL,
    target_start_position INTEGER NOT NULL,
    target_end_position INTEGER NOT NULL,
    moved_reply_count INTEGER NOT NULL,
    moved_published_reply_count INTEGER NOT NULL,
    source_resulting_published_reply_count INTEGER NOT NULL,
    target_resulting_published_reply_count INTEGER NOT NULL,
    moved_solution_reply_id TEXT NULL,
    source_resulting_solution_reply_id TEXT NULL,
    target_resulting_solution_reply_id TEXT NULL,
    event_id TEXT NOT NULL,
    moved_at TEXT NOT NULL,
    PRIMARY KEY (tenant_id, operation_id),
    UNIQUE (event_id),
    FOREIGN KEY (tenant_id, source_topic_id)
        REFERENCES forum_topics (tenant_id, id)
        ON UPDATE RESTRICT ON DELETE RESTRICT,
    FOREIGN KEY (tenant_id, target_topic_id)
        REFERENCES forum_topics (tenant_id, id)
        ON UPDATE RESTRICT ON DELETE RESTRICT,
    FOREIGN KEY (tenant_id, source_category_id)
        REFERENCES forum_categories (tenant_id, id)
        ON UPDATE RESTRICT ON DELETE RESTRICT,
    FOREIGN KEY (tenant_id, target_category_id)
        REFERENCES forum_categories (tenant_id, id)
        ON UPDATE RESTRICT ON DELETE RESTRICT,
    FOREIGN KEY (tenant_id, actor_id)
        REFERENCES users (tenant_id, id)
        ON UPDATE RESTRICT ON DELETE RESTRICT,
    CHECK (
        operation_id <> '00000000-0000-0000-0000-000000000000'
        AND source_topic_id <> '00000000-0000-0000-0000-000000000000'
        AND target_topic_id <> '00000000-0000-0000-0000-000000000000'
        AND source_category_id <> '00000000-0000-0000-0000-000000000000'
        AND target_category_id <> '00000000-0000-0000-0000-000000000000'
        AND actor_id <> '00000000-0000-0000-0000-000000000000'
        AND source_topic_id <> target_topic_id
        AND event_id = operation_id
    ),
    CHECK (
        length(reason) BETWEEN 1 AND 500
        AND reason = trim(reason)
        AND instr(reason, char(10)) = 0
        AND instr(reason, char(13)) = 0
    ),
    CHECK (
        length(command_fingerprint) = 64
        AND command_fingerprint = lower(command_fingerprint)
    ),
    CHECK (
        source_start_position > 0
        AND source_end_position >= source_start_position
        AND target_start_position > 0
        AND target_end_position >= target_start_position
        AND target_end_position = target_start_position + moved_reply_count - 1
    ),
    CHECK (
        moved_reply_count BETWEEN 1 AND 500
        AND moved_published_reply_count BETWEEN 0 AND moved_reply_count
        AND source_resulting_published_reply_count >= 0
        AND target_resulting_published_reply_count >= moved_published_reply_count
    )
);

CREATE INDEX IF NOT EXISTS idx_forum_reply_range_move_source_history
    ON forum_reply_range_move_operations (
        tenant_id, source_topic_id, moved_at DESC, operation_id
    );
CREATE INDEX IF NOT EXISTS idx_forum_reply_range_move_target_history
    ON forum_reply_range_move_operations (
        tenant_id, target_topic_id, moved_at DESC, operation_id
    );

CREATE TABLE IF NOT EXISTS forum_reply_range_move_items (
    tenant_id TEXT NOT NULL,
    operation_id TEXT NOT NULL,
    reply_id TEXT NOT NULL,
    source_parent_reply_id TEXT NULL,
    target_parent_reply_id TEXT NULL,
    source_position INTEGER NOT NULL,
    target_position INTEGER NOT NULL,
    was_published INTEGER NOT NULL,
    moved_at TEXT NOT NULL,
    PRIMARY KEY (tenant_id, operation_id, reply_id),
    UNIQUE (tenant_id, operation_id, source_position),
    UNIQUE (tenant_id, operation_id, target_position),
    FOREIGN KEY (tenant_id, operation_id)
        REFERENCES forum_reply_range_move_operations (tenant_id, operation_id)
        ON UPDATE RESTRICT ON DELETE RESTRICT,
    FOREIGN KEY (tenant_id, reply_id)
        REFERENCES forum_replies (tenant_id, id)
        ON UPDATE RESTRICT ON DELETE RESTRICT,
    CHECK (reply_id <> '00000000-0000-0000-0000-000000000000'),
    CHECK (source_position > 0 AND target_position > 0),
    CHECK (was_published IN (0, 1))
);

CREATE TRIGGER IF NOT EXISTS forum_reply_range_move_operation_update
BEFORE UPDATE ON forum_reply_range_move_operations
BEGIN
    SELECT RAISE(ABORT, 'forum reply range move audit is append-only');
END;
CREATE TRIGGER IF NOT EXISTS forum_reply_range_move_operation_delete
BEFORE DELETE ON forum_reply_range_move_operations
BEGIN
    SELECT RAISE(ABORT, 'forum reply range move audit is append-only');
END;
CREATE TRIGGER IF NOT EXISTS forum_reply_range_move_item_update
BEFORE UPDATE ON forum_reply_range_move_items
BEGIN
    SELECT RAISE(ABORT, 'forum reply range move audit is append-only');
END;
CREATE TRIGGER IF NOT EXISTS forum_reply_range_move_item_delete
BEFORE DELETE ON forum_reply_range_move_items
BEGIN
    SELECT RAISE(ABORT, 'forum reply range move audit is append-only');
END;
"#;

const SQLITE_DOWN: &str = r#"
DROP TRIGGER IF EXISTS forum_reply_range_move_item_delete;
DROP TRIGGER IF EXISTS forum_reply_range_move_item_update;
DROP TRIGGER IF EXISTS forum_reply_range_move_operation_delete;
DROP TRIGGER IF EXISTS forum_reply_range_move_operation_update;
DROP TABLE IF EXISTS forum_reply_range_move_items;
DROP TABLE IF EXISTS forum_reply_range_move_operations;
DROP TABLE IF EXISTS forum_reply_range_move_locks;
"#;
