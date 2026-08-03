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
                "rustok-forum merge tag reconciliation migration does not support {backend:?}"
            ))),
        }
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        match manager.get_database_backend() {
            DatabaseBackend::Postgres => execute(manager, POSTGRES_DOWN).await,
            DatabaseBackend::Sqlite => execute(manager, SQLITE_DOWN).await,
            backend => Err(DbErr::Custom(format!(
                "rustok-forum merge tag reconciliation migration does not support {backend:?}"
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
CREATE TABLE IF NOT EXISTS forum_topic_tag_locks (
    tenant_id UUID NOT NULL,
    topic_id UUID NOT NULL,
    touched_at TIMESTAMPTZ NOT NULL,
    PRIMARY KEY (tenant_id, topic_id),
    CONSTRAINT fk_forum_topic_tag_lock_topic
        FOREIGN KEY (tenant_id, topic_id)
        REFERENCES forum_topics (tenant_id, id)
        ON UPDATE CASCADE ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS forum_topic_merge_tag_reconciliation_locks (
    tenant_id UUID NOT NULL PRIMARY KEY,
    touched_at TIMESTAMPTZ NOT NULL
);

CREATE TABLE IF NOT EXISTS forum_topic_merge_tag_reconciliations (
    tenant_id UUID NOT NULL,
    operation_id UUID NOT NULL,
    merge_operation_id UUID NOT NULL,
    source_topic_id UUID NOT NULL,
    target_topic_id UUID NOT NULL,
    actor_id UUID NOT NULL,
    reason VARCHAR(500) NOT NULL,
    source_tag_count INTEGER NOT NULL,
    moved_source_only_count INTEGER NOT NULL,
    deduplicated_existing_count INTEGER NOT NULL,
    event_id UUID NOT NULL,
    reconciled_at TIMESTAMPTZ NOT NULL,
    CONSTRAINT pk_forum_topic_merge_tag_reconciliations
        PRIMARY KEY (tenant_id, operation_id),
    CONSTRAINT uq_forum_topic_merge_tag_reconciliation_event
        UNIQUE (event_id),
    CONSTRAINT uq_forum_topic_merge_tag_reconciliation_merge
        UNIQUE (tenant_id, merge_operation_id),
    CONSTRAINT fk_forum_topic_merge_tag_reconciliation_merge
        FOREIGN KEY (tenant_id, merge_operation_id)
        REFERENCES forum_topic_merge_operations (tenant_id, operation_id)
        ON UPDATE CASCADE ON DELETE RESTRICT,
    CONSTRAINT fk_forum_topic_merge_tag_reconciliation_source
        FOREIGN KEY (tenant_id, source_topic_id)
        REFERENCES forum_topics (tenant_id, id)
        ON UPDATE CASCADE ON DELETE RESTRICT,
    CONSTRAINT fk_forum_topic_merge_tag_reconciliation_target
        FOREIGN KEY (tenant_id, target_topic_id)
        REFERENCES forum_topics (tenant_id, id)
        ON UPDATE CASCADE ON DELETE RESTRICT,
    CONSTRAINT fk_forum_topic_merge_tag_reconciliation_actor
        FOREIGN KEY (tenant_id, actor_id)
        REFERENCES users (tenant_id, id)
        ON UPDATE CASCADE ON DELETE RESTRICT,
    CONSTRAINT ck_forum_topic_merge_tag_reconciliation_ids CHECK (
        operation_id <> '00000000-0000-0000-0000-000000000000'::uuid
        AND merge_operation_id <> '00000000-0000-0000-0000-000000000000'::uuid
        AND source_topic_id <> '00000000-0000-0000-0000-000000000000'::uuid
        AND target_topic_id <> '00000000-0000-0000-0000-000000000000'::uuid
        AND actor_id <> '00000000-0000-0000-0000-000000000000'::uuid
        AND source_topic_id <> target_topic_id
        AND event_id = operation_id
    ),
    CONSTRAINT ck_forum_topic_merge_tag_reconciliation_reason CHECK (
        length(reason) BETWEEN 1 AND 500
        AND reason = btrim(reason)
        AND position(E'\n' in reason) = 0
        AND position(E'\r' in reason) = 0
    ),
    CONSTRAINT ck_forum_topic_merge_tag_reconciliation_counts CHECK (
        source_tag_count BETWEEN 0 AND 500
        AND moved_source_only_count BETWEEN 0 AND 500
        AND deduplicated_existing_count BETWEEN 0 AND 500
        AND source_tag_count = moved_source_only_count + deduplicated_existing_count
    )
);

CREATE INDEX IF NOT EXISTS idx_forum_topic_merge_tag_reconciliation_history
    ON forum_topic_merge_tag_reconciliations (
        tenant_id, target_topic_id, reconciled_at DESC, operation_id
    );

CREATE OR REPLACE FUNCTION forum_lock_topic_tag_mutation()
RETURNS trigger AS $$
DECLARE
    row_tenant_id uuid;
    first_topic_id uuid;
    second_topic_id uuid;
BEGIN
    IF TG_OP = 'DELETE' THEN
        row_tenant_id := OLD.tenant_id;
        first_topic_id := OLD.topic_id;
    ELSIF TG_OP = 'INSERT' THEN
        row_tenant_id := NEW.tenant_id;
        first_topic_id := NEW.topic_id;
    ELSE
        IF OLD.tenant_id <> NEW.tenant_id THEN
            RAISE EXCEPTION 'forum topic tag tenant is immutable';
        END IF;
        row_tenant_id := NEW.tenant_id;
        IF OLD.topic_id <= NEW.topic_id THEN
            first_topic_id := OLD.topic_id;
            second_topic_id := NEW.topic_id;
        ELSE
            first_topic_id := NEW.topic_id;
            second_topic_id := OLD.topic_id;
        END IF;
    END IF;

    PERFORM pg_advisory_xact_lock(hashtextextended(
        format('forum-topic-tag:%s:%s', row_tenant_id, first_topic_id),
        26
    ));
    IF second_topic_id IS NOT NULL AND second_topic_id <> first_topic_id THEN
        PERFORM pg_advisory_xact_lock(hashtextextended(
            format('forum-topic-tag:%s:%s', row_tenant_id, second_topic_id),
            26
        ));
    END IF;

    IF TG_OP = 'DELETE' THEN
        RETURN OLD;
    END IF;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS forum_00_topic_tag_scope ON forum_topic_tags;
CREATE TRIGGER forum_00_topic_tag_scope
BEFORE INSERT OR UPDATE OR DELETE ON forum_topic_tags
FOR EACH ROW EXECUTE FUNCTION forum_lock_topic_tag_mutation();

CREATE OR REPLACE FUNCTION forum_reject_archived_topic_tag_write()
RETURNS trigger AS $$
DECLARE
    topic_status text;
BEGIN
    SELECT status::text
      INTO topic_status
      FROM forum_topics
     WHERE tenant_id = NEW.tenant_id
       AND id = NEW.topic_id;

    IF topic_status = 'archived' THEN
        RAISE EXCEPTION 'forum topic tags cannot target archived topics';
    END IF;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS forum_topic_tags_active_write ON forum_topic_tags;
DROP TRIGGER IF EXISTS forum_10_topic_tags_active_write ON forum_topic_tags;
CREATE TRIGGER forum_10_topic_tags_active_write
BEFORE INSERT OR UPDATE ON forum_topic_tags
FOR EACH ROW EXECUTE FUNCTION forum_reject_archived_topic_tag_write();

CREATE OR REPLACE FUNCTION forum_reject_topic_merge_tag_reconciliation_mutation()
RETURNS trigger AS $$
BEGIN
    RAISE EXCEPTION 'forum topic merge tag reconciliations are append-only';
    RETURN NULL;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS forum_topic_merge_tag_reconciliation_update
    ON forum_topic_merge_tag_reconciliations;
CREATE TRIGGER forum_topic_merge_tag_reconciliation_update
BEFORE UPDATE ON forum_topic_merge_tag_reconciliations
FOR EACH ROW EXECUTE FUNCTION forum_reject_topic_merge_tag_reconciliation_mutation();

DROP TRIGGER IF EXISTS forum_topic_merge_tag_reconciliation_delete
    ON forum_topic_merge_tag_reconciliations;
CREATE TRIGGER forum_topic_merge_tag_reconciliation_delete
BEFORE DELETE ON forum_topic_merge_tag_reconciliations
FOR EACH ROW EXECUTE FUNCTION forum_reject_topic_merge_tag_reconciliation_mutation();
"#;

const POSTGRES_DOWN: &str = r#"
DROP TRIGGER IF EXISTS forum_topic_merge_tag_reconciliation_delete
    ON forum_topic_merge_tag_reconciliations;
DROP TRIGGER IF EXISTS forum_topic_merge_tag_reconciliation_update
    ON forum_topic_merge_tag_reconciliations;
DROP TRIGGER IF EXISTS forum_10_topic_tags_active_write ON forum_topic_tags;
DROP TRIGGER IF EXISTS forum_topic_tags_active_write ON forum_topic_tags;
DROP TRIGGER IF EXISTS forum_00_topic_tag_scope ON forum_topic_tags;
DROP TABLE IF EXISTS forum_topic_merge_tag_reconciliations;
DROP TABLE IF EXISTS forum_topic_merge_tag_reconciliation_locks;
DROP TABLE IF EXISTS forum_topic_tag_locks;
DROP FUNCTION IF EXISTS forum_reject_topic_merge_tag_reconciliation_mutation();
DROP FUNCTION IF EXISTS forum_reject_archived_topic_tag_write();
DROP FUNCTION IF EXISTS forum_lock_topic_tag_mutation();
"#;

const SQLITE_UP: &str = r#"
CREATE TABLE IF NOT EXISTS forum_topic_tag_locks (
    tenant_id TEXT NOT NULL,
    topic_id TEXT NOT NULL,
    touched_at TEXT NOT NULL,
    PRIMARY KEY (tenant_id, topic_id),
    FOREIGN KEY (tenant_id, topic_id)
        REFERENCES forum_topics (tenant_id, id)
        ON UPDATE CASCADE ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS forum_topic_merge_tag_reconciliation_locks (
    tenant_id TEXT NOT NULL PRIMARY KEY,
    touched_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS forum_topic_merge_tag_reconciliations (
    tenant_id TEXT NOT NULL,
    operation_id TEXT NOT NULL,
    merge_operation_id TEXT NOT NULL,
    source_topic_id TEXT NOT NULL,
    target_topic_id TEXT NOT NULL,
    actor_id TEXT NOT NULL,
    reason TEXT NOT NULL,
    source_tag_count INTEGER NOT NULL,
    moved_source_only_count INTEGER NOT NULL,
    deduplicated_existing_count INTEGER NOT NULL,
    event_id TEXT NOT NULL,
    reconciled_at TEXT NOT NULL,
    PRIMARY KEY (tenant_id, operation_id),
    UNIQUE (event_id),
    UNIQUE (tenant_id, merge_operation_id),
    FOREIGN KEY (tenant_id, merge_operation_id)
        REFERENCES forum_topic_merge_operations (tenant_id, operation_id)
        ON UPDATE CASCADE ON DELETE RESTRICT,
    FOREIGN KEY (tenant_id, source_topic_id)
        REFERENCES forum_topics (tenant_id, id)
        ON UPDATE CASCADE ON DELETE RESTRICT,
    FOREIGN KEY (tenant_id, target_topic_id)
        REFERENCES forum_topics (tenant_id, id)
        ON UPDATE CASCADE ON DELETE RESTRICT,
    FOREIGN KEY (tenant_id, actor_id)
        REFERENCES users (tenant_id, id)
        ON UPDATE CASCADE ON DELETE RESTRICT,
    CHECK (
        operation_id <> '00000000-0000-0000-0000-000000000000'
        AND merge_operation_id <> '00000000-0000-0000-0000-000000000000'
        AND source_topic_id <> '00000000-0000-0000-0000-000000000000'
        AND target_topic_id <> '00000000-0000-0000-0000-000000000000'
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
        source_tag_count BETWEEN 0 AND 500
        AND moved_source_only_count BETWEEN 0 AND 500
        AND deduplicated_existing_count BETWEEN 0 AND 500
        AND source_tag_count = moved_source_only_count + deduplicated_existing_count
    )
);

CREATE INDEX IF NOT EXISTS idx_forum_topic_merge_tag_reconciliation_history
    ON forum_topic_merge_tag_reconciliations (
        tenant_id, target_topic_id, reconciled_at DESC, operation_id
    );

CREATE TRIGGER IF NOT EXISTS forum_topic_tags_active_insert
BEFORE INSERT ON forum_topic_tags
FOR EACH ROW
WHEN EXISTS (
    SELECT 1
    FROM forum_topics topic
    WHERE topic.tenant_id = NEW.tenant_id
      AND topic.id = NEW.topic_id
      AND topic.status = 'archived'
)
BEGIN
    SELECT RAISE(ABORT, 'forum topic tags cannot target archived topics');
END;

CREATE TRIGGER IF NOT EXISTS forum_topic_tags_active_update
BEFORE UPDATE ON forum_topic_tags
FOR EACH ROW
WHEN EXISTS (
    SELECT 1
    FROM forum_topics topic
    WHERE topic.tenant_id = NEW.tenant_id
      AND topic.id = NEW.topic_id
      AND topic.status = 'archived'
)
BEGIN
    SELECT RAISE(ABORT, 'forum topic tags cannot target archived topics');
END;

CREATE TRIGGER IF NOT EXISTS forum_topic_merge_tag_reconciliation_update
BEFORE UPDATE ON forum_topic_merge_tag_reconciliations
BEGIN
    SELECT RAISE(ABORT, 'forum topic merge tag reconciliations are append-only');
END;

CREATE TRIGGER IF NOT EXISTS forum_topic_merge_tag_reconciliation_delete
BEFORE DELETE ON forum_topic_merge_tag_reconciliations
BEGIN
    SELECT RAISE(ABORT, 'forum topic merge tag reconciliations are append-only');
END;
"#;

const SQLITE_DOWN: &str = r#"
DROP TRIGGER IF EXISTS forum_topic_merge_tag_reconciliation_delete;
DROP TRIGGER IF EXISTS forum_topic_merge_tag_reconciliation_update;
DROP TRIGGER IF EXISTS forum_topic_tags_active_update;
DROP TRIGGER IF EXISTS forum_topic_tags_active_insert;
DROP TABLE IF EXISTS forum_topic_merge_tag_reconciliations;
DROP TABLE IF EXISTS forum_topic_merge_tag_reconciliation_locks;
DROP TABLE IF EXISTS forum_topic_tag_locks;
"#;
