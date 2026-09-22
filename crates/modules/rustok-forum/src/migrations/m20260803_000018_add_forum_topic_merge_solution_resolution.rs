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
                "rustok-forum topic merge solution resolution migration does not support {backend:?}"
            ))),
        }
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        match manager.get_database_backend() {
            DatabaseBackend::Postgres => execute(manager, POSTGRES_DOWN).await,
            DatabaseBackend::Sqlite => execute(manager, SQLITE_DOWN).await,
            backend => Err(DbErr::Custom(format!(
                "rustok-forum topic merge solution resolution migration does not support {backend:?}"
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
CREATE TABLE IF NOT EXISTS forum_topic_merge_solution_resolutions (
    tenant_id UUID NOT NULL,
    operation_id UUID NOT NULL,
    source_solution_reply_id UUID NOT NULL,
    target_solution_reply_id UUID NOT NULL,
    selected_solution_reply_id UUID NOT NULL,
    rejected_solution_reply_id UUID NOT NULL,
    rejected_solution_author_id UUID NULL,
    resolved_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    PRIMARY KEY (tenant_id, operation_id),
    CONSTRAINT fk_forum_topic_merge_solution_resolution_operation
        FOREIGN KEY (tenant_id, operation_id)
        REFERENCES forum_topic_merge_operations (tenant_id, operation_id)
        ON UPDATE CASCADE ON DELETE RESTRICT,
    CONSTRAINT fk_forum_topic_merge_solution_resolution_source_reply
        FOREIGN KEY (tenant_id, source_solution_reply_id)
        REFERENCES forum_replies (tenant_id, id)
        ON UPDATE CASCADE ON DELETE RESTRICT,
    CONSTRAINT fk_forum_topic_merge_solution_resolution_target_reply
        FOREIGN KEY (tenant_id, target_solution_reply_id)
        REFERENCES forum_replies (tenant_id, id)
        ON UPDATE CASCADE ON DELETE RESTRICT,
    CONSTRAINT fk_forum_topic_merge_solution_resolution_selected_reply
        FOREIGN KEY (tenant_id, selected_solution_reply_id)
        REFERENCES forum_replies (tenant_id, id)
        ON UPDATE CASCADE ON DELETE RESTRICT,
    CONSTRAINT fk_forum_topic_merge_solution_resolution_rejected_reply
        FOREIGN KEY (tenant_id, rejected_solution_reply_id)
        REFERENCES forum_replies (tenant_id, id)
        ON UPDATE CASCADE ON DELETE RESTRICT,
    CONSTRAINT fk_forum_topic_merge_solution_resolution_rejected_author
        FOREIGN KEY (tenant_id, rejected_solution_author_id)
        REFERENCES users (tenant_id, id)
        ON UPDATE CASCADE ON DELETE RESTRICT,
    CONSTRAINT ck_forum_topic_merge_solution_resolution_ids CHECK (
        operation_id <> '00000000-0000-0000-0000-000000000000'::uuid
        AND source_solution_reply_id <> '00000000-0000-0000-0000-000000000000'::uuid
        AND target_solution_reply_id <> '00000000-0000-0000-0000-000000000000'::uuid
        AND selected_solution_reply_id <> '00000000-0000-0000-0000-000000000000'::uuid
        AND rejected_solution_reply_id <> '00000000-0000-0000-0000-000000000000'::uuid
        AND source_solution_reply_id <> target_solution_reply_id
        AND selected_solution_reply_id <> rejected_solution_reply_id
        AND (
            (selected_solution_reply_id = source_solution_reply_id
             AND rejected_solution_reply_id = target_solution_reply_id)
            OR
            (selected_solution_reply_id = target_solution_reply_id
             AND rejected_solution_reply_id = source_solution_reply_id)
        )
    )
);

CREATE OR REPLACE FUNCTION forum_reject_topic_merge_solution_resolution_mutation()
RETURNS trigger AS $$
BEGIN
    RAISE EXCEPTION 'forum topic merge solution resolutions are append-only';
    RETURN NULL;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS forum_topic_merge_solution_resolution_update
    ON forum_topic_merge_solution_resolutions;
CREATE TRIGGER forum_topic_merge_solution_resolution_update
BEFORE UPDATE ON forum_topic_merge_solution_resolutions
FOR EACH ROW EXECUTE FUNCTION forum_reject_topic_merge_solution_resolution_mutation();

DROP TRIGGER IF EXISTS forum_topic_merge_solution_resolution_delete
    ON forum_topic_merge_solution_resolutions;
CREATE TRIGGER forum_topic_merge_solution_resolution_delete
BEFORE DELETE ON forum_topic_merge_solution_resolutions
FOR EACH ROW EXECUTE FUNCTION forum_reject_topic_merge_solution_resolution_mutation();
"#;

const POSTGRES_DOWN: &str = r#"
DROP TRIGGER IF EXISTS forum_topic_merge_solution_resolution_delete
    ON forum_topic_merge_solution_resolutions;
DROP TRIGGER IF EXISTS forum_topic_merge_solution_resolution_update
    ON forum_topic_merge_solution_resolutions;
DROP TABLE IF EXISTS forum_topic_merge_solution_resolutions;
DROP FUNCTION IF EXISTS forum_reject_topic_merge_solution_resolution_mutation();
"#;

const SQLITE_UP: &str = r#"
CREATE TABLE IF NOT EXISTS forum_topic_merge_solution_resolutions (
    tenant_id TEXT NOT NULL,
    operation_id TEXT NOT NULL,
    source_solution_reply_id TEXT NOT NULL,
    target_solution_reply_id TEXT NOT NULL,
    selected_solution_reply_id TEXT NOT NULL,
    rejected_solution_reply_id TEXT NOT NULL,
    rejected_solution_author_id TEXT NULL,
    resolved_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    PRIMARY KEY (tenant_id, operation_id),
    FOREIGN KEY (tenant_id, operation_id)
        REFERENCES forum_topic_merge_operations (tenant_id, operation_id)
        ON UPDATE CASCADE ON DELETE RESTRICT,
    FOREIGN KEY (tenant_id, source_solution_reply_id)
        REFERENCES forum_replies (tenant_id, id)
        ON UPDATE CASCADE ON DELETE RESTRICT,
    FOREIGN KEY (tenant_id, target_solution_reply_id)
        REFERENCES forum_replies (tenant_id, id)
        ON UPDATE CASCADE ON DELETE RESTRICT,
    FOREIGN KEY (tenant_id, selected_solution_reply_id)
        REFERENCES forum_replies (tenant_id, id)
        ON UPDATE CASCADE ON DELETE RESTRICT,
    FOREIGN KEY (tenant_id, rejected_solution_reply_id)
        REFERENCES forum_replies (tenant_id, id)
        ON UPDATE CASCADE ON DELETE RESTRICT,
    FOREIGN KEY (tenant_id, rejected_solution_author_id)
        REFERENCES users (tenant_id, id)
        ON UPDATE CASCADE ON DELETE RESTRICT,
    CHECK (
        operation_id <> '00000000-0000-0000-0000-000000000000'
        AND source_solution_reply_id <> '00000000-0000-0000-0000-000000000000'
        AND target_solution_reply_id <> '00000000-0000-0000-0000-000000000000'
        AND selected_solution_reply_id <> '00000000-0000-0000-0000-000000000000'
        AND rejected_solution_reply_id <> '00000000-0000-0000-0000-000000000000'
        AND source_solution_reply_id <> target_solution_reply_id
        AND selected_solution_reply_id <> rejected_solution_reply_id
        AND (
            (selected_solution_reply_id = source_solution_reply_id
             AND rejected_solution_reply_id = target_solution_reply_id)
            OR
            (selected_solution_reply_id = target_solution_reply_id
             AND rejected_solution_reply_id = source_solution_reply_id)
        )
    )
);

CREATE TRIGGER IF NOT EXISTS forum_topic_merge_solution_resolution_update
BEFORE UPDATE ON forum_topic_merge_solution_resolutions
BEGIN
    SELECT RAISE(ABORT, 'forum topic merge solution resolutions are append-only');
END;

CREATE TRIGGER IF NOT EXISTS forum_topic_merge_solution_resolution_delete
BEFORE DELETE ON forum_topic_merge_solution_resolutions
BEGIN
    SELECT RAISE(ABORT, 'forum topic merge solution resolutions are append-only');
END;
"#;

const SQLITE_DOWN: &str = r#"
DROP TRIGGER IF EXISTS forum_topic_merge_solution_resolution_delete;
DROP TRIGGER IF EXISTS forum_topic_merge_solution_resolution_update;
DROP TABLE IF EXISTS forum_topic_merge_solution_resolutions;
"#;
