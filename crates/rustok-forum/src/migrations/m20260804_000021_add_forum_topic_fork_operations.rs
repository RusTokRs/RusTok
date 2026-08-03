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
                "rustok-forum topic fork migration does not support {backend:?}"
            ))),
        }
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        match manager.get_database_backend() {
            DatabaseBackend::Postgres => execute(manager, POSTGRES_DOWN).await,
            DatabaseBackend::Sqlite => execute(manager, SQLITE_DOWN).await,
            backend => Err(DbErr::Custom(format!(
                "rustok-forum topic fork rollback does not support {backend:?}"
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
CREATE TABLE IF NOT EXISTS forum_topic_fork_locks (
    tenant_id UUID NOT NULL PRIMARY KEY,
    touched_at TIMESTAMPTZ NOT NULL
);

CREATE TABLE IF NOT EXISTS forum_topic_fork_operations (
    tenant_id UUID NOT NULL,
    operation_id UUID NOT NULL,
    source_topic_id UUID NOT NULL,
    target_topic_id UUID NOT NULL,
    root_reply_id UUID NOT NULL,
    category_id UUID NOT NULL,
    actor_id UUID NOT NULL,
    reason VARCHAR(500) NOT NULL,
    command_fingerprint VARCHAR(64) NOT NULL,
    copied_reply_count INTEGER NOT NULL,
    copied_published_reply_count INTEGER NOT NULL,
    copied_body_count INTEGER NOT NULL,
    copied_reply_revision_count INTEGER NOT NULL,
    copied_relation_revision_count INTEGER NOT NULL,
    copied_mention_count INTEGER NOT NULL,
    copied_quote_count INTEGER NOT NULL,
    event_id UUID NOT NULL,
    forked_at TIMESTAMPTZ NOT NULL,
    CONSTRAINT pk_forum_topic_fork_operations
        PRIMARY KEY (tenant_id, operation_id),
    CONSTRAINT uq_forum_topic_fork_target UNIQUE (tenant_id, target_topic_id),
    CONSTRAINT uq_forum_topic_fork_event UNIQUE (event_id),
    CONSTRAINT fk_forum_topic_fork_source_topic
        FOREIGN KEY (tenant_id, source_topic_id)
        REFERENCES forum_topics (tenant_id, id)
        ON UPDATE CASCADE ON DELETE RESTRICT,
    CONSTRAINT fk_forum_topic_fork_target_topic
        FOREIGN KEY (tenant_id, target_topic_id)
        REFERENCES forum_topics (tenant_id, id)
        ON UPDATE CASCADE ON DELETE RESTRICT,
    CONSTRAINT fk_forum_topic_fork_category
        FOREIGN KEY (tenant_id, category_id)
        REFERENCES forum_categories (tenant_id, id)
        ON UPDATE CASCADE ON DELETE RESTRICT,
    CONSTRAINT fk_forum_topic_fork_root_reply
        FOREIGN KEY (tenant_id, source_topic_id, root_reply_id)
        REFERENCES forum_replies (tenant_id, topic_id, id)
        ON UPDATE CASCADE ON DELETE RESTRICT,
    CONSTRAINT fk_forum_topic_fork_actor
        FOREIGN KEY (tenant_id, actor_id)
        REFERENCES users (tenant_id, id)
        ON UPDATE CASCADE ON DELETE RESTRICT,
    CONSTRAINT ck_forum_topic_fork_ids CHECK (
        operation_id <> '00000000-0000-0000-0000-000000000000'::uuid
        AND source_topic_id <> '00000000-0000-0000-0000-000000000000'::uuid
        AND target_topic_id <> '00000000-0000-0000-0000-000000000000'::uuid
        AND root_reply_id <> '00000000-0000-0000-0000-000000000000'::uuid
        AND category_id <> '00000000-0000-0000-0000-000000000000'::uuid
        AND actor_id <> '00000000-0000-0000-0000-000000000000'::uuid
        AND source_topic_id <> target_topic_id
        AND event_id = operation_id
    ),
    CONSTRAINT ck_forum_topic_fork_reason CHECK (
        length(reason) BETWEEN 1 AND 500
        AND reason = btrim(reason)
        AND position(E'\n' in reason) = 0
        AND position(E'\r' in reason) = 0
    ),
    CONSTRAINT ck_forum_topic_fork_fingerprint CHECK (
        length(command_fingerprint) = 64
        AND command_fingerprint = lower(command_fingerprint)
    ),
    CONSTRAINT ck_forum_topic_fork_counts CHECK (
        copied_reply_count BETWEEN 1 AND 500
        AND copied_published_reply_count BETWEEN 0 AND copied_reply_count
        AND copied_body_count BETWEEN copied_reply_count AND 2000
        AND copied_reply_revision_count BETWEEN 0 AND 5000
        AND copied_relation_revision_count BETWEEN 0 AND 5000
        AND copied_mention_count BETWEEN 0 AND 10000
        AND copied_quote_count BETWEEN 0 AND 5000
    )
);

CREATE INDEX IF NOT EXISTS idx_forum_topic_fork_source_history
    ON forum_topic_fork_operations (
        tenant_id, source_topic_id, forked_at DESC, operation_id
    );
CREATE INDEX IF NOT EXISTS idx_forum_topic_fork_root_history
    ON forum_topic_fork_operations (
        tenant_id, root_reply_id, forked_at DESC, operation_id
    );

CREATE TABLE IF NOT EXISTS forum_topic_fork_reply_items (
    tenant_id UUID NOT NULL,
    operation_id UUID NOT NULL,
    source_reply_id UUID NOT NULL,
    target_reply_id UUID NOT NULL,
    source_parent_reply_id UUID NULL,
    target_parent_reply_id UUID NULL,
    source_position BIGINT NOT NULL,
    target_position BIGINT NOT NULL,
    was_published BOOLEAN NOT NULL,
    copied_at TIMESTAMPTZ NOT NULL,
    CONSTRAINT pk_forum_topic_fork_reply_items
        PRIMARY KEY (tenant_id, operation_id, source_reply_id),
    CONSTRAINT uq_forum_topic_fork_target_reply
        UNIQUE (tenant_id, operation_id, target_reply_id),
    CONSTRAINT uq_forum_topic_fork_target_position
        UNIQUE (tenant_id, operation_id, target_position),
    CONSTRAINT fk_forum_topic_fork_reply_operation
        FOREIGN KEY (tenant_id, operation_id)
        REFERENCES forum_topic_fork_operations (tenant_id, operation_id)
        ON UPDATE CASCADE ON DELETE RESTRICT,
    CONSTRAINT fk_forum_topic_fork_source_reply
        FOREIGN KEY (tenant_id, source_reply_id)
        REFERENCES forum_replies (tenant_id, id)
        ON UPDATE CASCADE ON DELETE RESTRICT,
    CONSTRAINT fk_forum_topic_fork_target_reply_row
        FOREIGN KEY (tenant_id, target_reply_id)
        REFERENCES forum_replies (tenant_id, id)
        ON UPDATE CASCADE ON DELETE RESTRICT,
    CONSTRAINT ck_forum_topic_fork_reply_ids CHECK (
        source_reply_id <> '00000000-0000-0000-0000-000000000000'::uuid
        AND target_reply_id <> '00000000-0000-0000-0000-000000000000'::uuid
        AND source_reply_id <> target_reply_id
    ),
    CONSTRAINT ck_forum_topic_fork_reply_positions CHECK (
        source_position > 0 AND target_position > 0
    )
);

CREATE TABLE IF NOT EXISTS forum_topic_fork_revision_items (
    tenant_id UUID NOT NULL,
    operation_id UUID NOT NULL,
    revision_kind VARCHAR(16) NOT NULL,
    source_revision_id BIGINT NOT NULL,
    target_revision_id BIGINT NOT NULL,
    source_reply_id UUID NOT NULL,
    target_reply_id UUID NOT NULL,
    locale VARCHAR(32) NOT NULL,
    copied_at TIMESTAMPTZ NOT NULL,
    CONSTRAINT pk_forum_topic_fork_revision_items
        PRIMARY KEY (
            tenant_id, operation_id, revision_kind, source_revision_id
        ),
    CONSTRAINT uq_forum_topic_fork_target_revision
        UNIQUE (
            tenant_id, operation_id, revision_kind, target_revision_id
        ),
    CONSTRAINT fk_forum_topic_fork_revision_operation
        FOREIGN KEY (tenant_id, operation_id)
        REFERENCES forum_topic_fork_operations (tenant_id, operation_id)
        ON UPDATE CASCADE ON DELETE RESTRICT,
    CONSTRAINT fk_forum_topic_fork_revision_reply_item
        FOREIGN KEY (tenant_id, operation_id, source_reply_id)
        REFERENCES forum_topic_fork_reply_items (
            tenant_id, operation_id, source_reply_id
        )
        ON UPDATE CASCADE ON DELETE RESTRICT,
    CONSTRAINT ck_forum_topic_fork_revision_kind CHECK (
        revision_kind IN ('reply', 'relation')
    ),
    CONSTRAINT ck_forum_topic_fork_revision_ids CHECK (
        source_revision_id > 0
        AND target_revision_id > 0
        AND source_revision_id <> target_revision_id
        AND source_reply_id <> target_reply_id
    ),
    CONSTRAINT ck_forum_topic_fork_revision_locale CHECK (
        length(locale) BETWEEN 1 AND 32
        AND locale = btrim(locale)
    )
);

CREATE OR REPLACE FUNCTION forum_reject_topic_fork_audit_mutation()
RETURNS trigger AS $$
BEGIN
    RAISE EXCEPTION 'forum topic fork audit is append-only';
    RETURN NULL;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS forum_topic_fork_operation_update
    ON forum_topic_fork_operations;
CREATE TRIGGER forum_topic_fork_operation_update
BEFORE UPDATE ON forum_topic_fork_operations
FOR EACH ROW EXECUTE FUNCTION forum_reject_topic_fork_audit_mutation();

DROP TRIGGER IF EXISTS forum_topic_fork_operation_delete
    ON forum_topic_fork_operations;
CREATE TRIGGER forum_topic_fork_operation_delete
BEFORE DELETE ON forum_topic_fork_operations
FOR EACH ROW EXECUTE FUNCTION forum_reject_topic_fork_audit_mutation();

DROP TRIGGER IF EXISTS forum_topic_fork_reply_item_update
    ON forum_topic_fork_reply_items;
CREATE TRIGGER forum_topic_fork_reply_item_update
BEFORE UPDATE ON forum_topic_fork_reply_items
FOR EACH ROW EXECUTE FUNCTION forum_reject_topic_fork_audit_mutation();

DROP TRIGGER IF EXISTS forum_topic_fork_reply_item_delete
    ON forum_topic_fork_reply_items;
CREATE TRIGGER forum_topic_fork_reply_item_delete
BEFORE DELETE ON forum_topic_fork_reply_items
FOR EACH ROW EXECUTE FUNCTION forum_reject_topic_fork_audit_mutation();

DROP TRIGGER IF EXISTS forum_topic_fork_revision_item_update
    ON forum_topic_fork_revision_items;
CREATE TRIGGER forum_topic_fork_revision_item_update
BEFORE UPDATE ON forum_topic_fork_revision_items
FOR EACH ROW EXECUTE FUNCTION forum_reject_topic_fork_audit_mutation();

DROP TRIGGER IF EXISTS forum_topic_fork_revision_item_delete
    ON forum_topic_fork_revision_items;
CREATE TRIGGER forum_topic_fork_revision_item_delete
BEFORE DELETE ON forum_topic_fork_revision_items
FOR EACH ROW EXECUTE FUNCTION forum_reject_topic_fork_audit_mutation();
"#;

const POSTGRES_DOWN: &str = r#"
DROP TRIGGER IF EXISTS forum_topic_fork_revision_item_delete
    ON forum_topic_fork_revision_items;
DROP TRIGGER IF EXISTS forum_topic_fork_revision_item_update
    ON forum_topic_fork_revision_items;
DROP TRIGGER IF EXISTS forum_topic_fork_reply_item_delete
    ON forum_topic_fork_reply_items;
DROP TRIGGER IF EXISTS forum_topic_fork_reply_item_update
    ON forum_topic_fork_reply_items;
DROP TRIGGER IF EXISTS forum_topic_fork_operation_delete
    ON forum_topic_fork_operations;
DROP TRIGGER IF EXISTS forum_topic_fork_operation_update
    ON forum_topic_fork_operations;
DROP TABLE IF EXISTS forum_topic_fork_revision_items;
DROP TABLE IF EXISTS forum_topic_fork_reply_items;
DROP TABLE IF EXISTS forum_topic_fork_operations;
DROP TABLE IF EXISTS forum_topic_fork_locks;
DROP FUNCTION IF EXISTS forum_reject_topic_fork_audit_mutation();
"#;

const SQLITE_UP: &str = r#"
CREATE TABLE IF NOT EXISTS forum_topic_fork_locks (
    tenant_id TEXT NOT NULL PRIMARY KEY,
    touched_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS forum_topic_fork_operations (
    tenant_id TEXT NOT NULL,
    operation_id TEXT NOT NULL,
    source_topic_id TEXT NOT NULL,
    target_topic_id TEXT NOT NULL,
    root_reply_id TEXT NOT NULL,
    category_id TEXT NOT NULL,
    actor_id TEXT NOT NULL,
    reason TEXT NOT NULL,
    command_fingerprint TEXT NOT NULL,
    copied_reply_count INTEGER NOT NULL,
    copied_published_reply_count INTEGER NOT NULL,
    copied_body_count INTEGER NOT NULL,
    copied_reply_revision_count INTEGER NOT NULL,
    copied_relation_revision_count INTEGER NOT NULL,
    copied_mention_count INTEGER NOT NULL,
    copied_quote_count INTEGER NOT NULL,
    event_id TEXT NOT NULL,
    forked_at TEXT NOT NULL,
    PRIMARY KEY (tenant_id, operation_id),
    UNIQUE (tenant_id, target_topic_id),
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
    FOREIGN KEY (tenant_id, source_topic_id, root_reply_id)
        REFERENCES forum_replies (tenant_id, topic_id, id)
        ON UPDATE CASCADE ON DELETE RESTRICT,
    FOREIGN KEY (tenant_id, actor_id)
        REFERENCES users (tenant_id, id)
        ON UPDATE CASCADE ON DELETE RESTRICT,
    CHECK (
        operation_id <> '00000000-0000-0000-0000-000000000000'
        AND source_topic_id <> '00000000-0000-0000-0000-000000000000'
        AND target_topic_id <> '00000000-0000-0000-0000-000000000000'
        AND root_reply_id <> '00000000-0000-0000-0000-000000000000'
        AND category_id <> '00000000-0000-0000-0000-000000000000'
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
        copied_reply_count BETWEEN 1 AND 500
        AND copied_published_reply_count BETWEEN 0 AND copied_reply_count
        AND copied_body_count BETWEEN copied_reply_count AND 2000
        AND copied_reply_revision_count BETWEEN 0 AND 5000
        AND copied_relation_revision_count BETWEEN 0 AND 5000
        AND copied_mention_count BETWEEN 0 AND 10000
        AND copied_quote_count BETWEEN 0 AND 5000
    )
);

CREATE INDEX IF NOT EXISTS idx_forum_topic_fork_source_history
    ON forum_topic_fork_operations (
        tenant_id, source_topic_id, forked_at DESC, operation_id
    );
CREATE INDEX IF NOT EXISTS idx_forum_topic_fork_root_history
    ON forum_topic_fork_operations (
        tenant_id, root_reply_id, forked_at DESC, operation_id
    );

CREATE TABLE IF NOT EXISTS forum_topic_fork_reply_items (
    tenant_id TEXT NOT NULL,
    operation_id TEXT NOT NULL,
    source_reply_id TEXT NOT NULL,
    target_reply_id TEXT NOT NULL,
    source_parent_reply_id TEXT NULL,
    target_parent_reply_id TEXT NULL,
    source_position INTEGER NOT NULL,
    target_position INTEGER NOT NULL,
    was_published INTEGER NOT NULL,
    copied_at TEXT NOT NULL,
    PRIMARY KEY (tenant_id, operation_id, source_reply_id),
    UNIQUE (tenant_id, operation_id, target_reply_id),
    UNIQUE (tenant_id, operation_id, target_position),
    FOREIGN KEY (tenant_id, operation_id)
        REFERENCES forum_topic_fork_operations (tenant_id, operation_id)
        ON UPDATE CASCADE ON DELETE RESTRICT,
    FOREIGN KEY (tenant_id, source_reply_id)
        REFERENCES forum_replies (tenant_id, id)
        ON UPDATE CASCADE ON DELETE RESTRICT,
    FOREIGN KEY (tenant_id, target_reply_id)
        REFERENCES forum_replies (tenant_id, id)
        ON UPDATE CASCADE ON DELETE RESTRICT,
    CHECK (
        source_reply_id <> '00000000-0000-0000-0000-000000000000'
        AND target_reply_id <> '00000000-0000-0000-0000-000000000000'
        AND source_reply_id <> target_reply_id
    ),
    CHECK (source_position > 0 AND target_position > 0),
    CHECK (was_published IN (0, 1))
);

CREATE TABLE IF NOT EXISTS forum_topic_fork_revision_items (
    tenant_id TEXT NOT NULL,
    operation_id TEXT NOT NULL,
    revision_kind TEXT NOT NULL,
    source_revision_id INTEGER NOT NULL,
    target_revision_id INTEGER NOT NULL,
    source_reply_id TEXT NOT NULL,
    target_reply_id TEXT NOT NULL,
    locale TEXT NOT NULL,
    copied_at TEXT NOT NULL,
    PRIMARY KEY (
        tenant_id, operation_id, revision_kind, source_revision_id
    ),
    UNIQUE (
        tenant_id, operation_id, revision_kind, target_revision_id
    ),
    FOREIGN KEY (tenant_id, operation_id)
        REFERENCES forum_topic_fork_operations (tenant_id, operation_id)
        ON UPDATE CASCADE ON DELETE RESTRICT,
    FOREIGN KEY (tenant_id, operation_id, source_reply_id)
        REFERENCES forum_topic_fork_reply_items (
            tenant_id, operation_id, source_reply_id
        )
        ON UPDATE CASCADE ON DELETE RESTRICT,
    CHECK (revision_kind IN ('reply', 'relation')),
    CHECK (
        source_revision_id > 0
        AND target_revision_id > 0
        AND source_revision_id <> target_revision_id
        AND source_reply_id <> target_reply_id
    ),
    CHECK (
        length(locale) BETWEEN 1 AND 32
        AND locale = trim(locale)
    )
);

CREATE TRIGGER IF NOT EXISTS forum_topic_fork_operation_update
BEFORE UPDATE ON forum_topic_fork_operations
BEGIN
    SELECT RAISE(ABORT, 'forum topic fork audit is append-only');
END;
CREATE TRIGGER IF NOT EXISTS forum_topic_fork_operation_delete
BEFORE DELETE ON forum_topic_fork_operations
BEGIN
    SELECT RAISE(ABORT, 'forum topic fork audit is append-only');
END;
CREATE TRIGGER IF NOT EXISTS forum_topic_fork_reply_item_update
BEFORE UPDATE ON forum_topic_fork_reply_items
BEGIN
    SELECT RAISE(ABORT, 'forum topic fork audit is append-only');
END;
CREATE TRIGGER IF NOT EXISTS forum_topic_fork_reply_item_delete
BEFORE DELETE ON forum_topic_fork_reply_items
BEGIN
    SELECT RAISE(ABORT, 'forum topic fork audit is append-only');
END;
CREATE TRIGGER IF NOT EXISTS forum_topic_fork_revision_item_update
BEFORE UPDATE ON forum_topic_fork_revision_items
BEGIN
    SELECT RAISE(ABORT, 'forum topic fork audit is append-only');
END;
CREATE TRIGGER IF NOT EXISTS forum_topic_fork_revision_item_delete
BEFORE DELETE ON forum_topic_fork_revision_items
BEGIN
    SELECT RAISE(ABORT, 'forum topic fork audit is append-only');
END;
"#;

const SQLITE_DOWN: &str = r#"
DROP TRIGGER IF EXISTS forum_topic_fork_revision_item_delete;
DROP TRIGGER IF EXISTS forum_topic_fork_revision_item_update;
DROP TRIGGER IF EXISTS forum_topic_fork_reply_item_delete;
DROP TRIGGER IF EXISTS forum_topic_fork_reply_item_update;
DROP TRIGGER IF EXISTS forum_topic_fork_operation_delete;
DROP TRIGGER IF EXISTS forum_topic_fork_operation_update;
DROP TABLE IF EXISTS forum_topic_fork_revision_items;
DROP TABLE IF EXISTS forum_topic_fork_reply_items;
DROP TABLE IF EXISTS forum_topic_fork_operations;
DROP TABLE IF EXISTS forum_topic_fork_locks;
"#;
