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
                "rustok-forum cross-category topic merge redirect migration does not support {backend:?}"
            ))),
        }
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        match manager.get_database_backend() {
            DatabaseBackend::Postgres => execute(manager, POSTGRES_DOWN).await,
            DatabaseBackend::Sqlite => execute(manager, SQLITE_DOWN).await,
            backend => Err(DbErr::Custom(format!(
                "rustok-forum cross-category topic merge redirect rollback does not support {backend:?}"
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
CREATE OR REPLACE FUNCTION forum_validate_topic_merge_redirect_edge()
RETURNS trigger AS $$
BEGIN
    IF NOT EXISTS (
        SELECT 1
        FROM forum_topics source
        WHERE source.tenant_id = NEW.tenant_id
          AND source.id = NEW.source_topic_id
          AND source.deleted_at IS NULL
          AND source.status::text = 'archived'
          AND source.is_locked = TRUE
          AND source.reply_count = 0
    ) THEN
        RAISE EXCEPTION 'forum topic merge redirect source is not an archived tombstone';
    END IF;

    IF NOT EXISTS (
        SELECT 1
        FROM forum_topics target
        WHERE target.tenant_id = NEW.tenant_id
          AND target.id = NEW.target_topic_id
          AND target.category_id = NEW.category_id
          AND target.deleted_at IS NULL
          AND target.status::text <> 'archived'
    ) THEN
        RAISE EXCEPTION 'forum topic merge redirect target is not active in the receipt category';
    END IF;

    RETURN NEW;
END;
$$ LANGUAGE plpgsql;
"#;

const POSTGRES_DOWN: &str = r#"
CREATE OR REPLACE FUNCTION forum_validate_topic_merge_redirect_edge()
RETURNS trigger AS $$
BEGIN
    IF NOT EXISTS (
        SELECT 1
        FROM forum_topics source
        WHERE source.tenant_id = NEW.tenant_id
          AND source.id = NEW.source_topic_id
          AND source.category_id = NEW.category_id
          AND source.deleted_at IS NULL
          AND source.status::text = 'archived'
          AND source.is_locked = TRUE
          AND source.reply_count = 0
    ) THEN
        RAISE EXCEPTION 'forum topic merge redirect source is not an archived tombstone';
    END IF;

    IF NOT EXISTS (
        SELECT 1
        FROM forum_topics target
        WHERE target.tenant_id = NEW.tenant_id
          AND target.id = NEW.target_topic_id
          AND target.category_id = NEW.category_id
          AND target.deleted_at IS NULL
          AND target.status::text <> 'archived'
    ) THEN
        RAISE EXCEPTION 'forum topic merge redirect target is not active';
    END IF;

    RETURN NEW;
END;
$$ LANGUAGE plpgsql;
"#;

const SQLITE_UP: &str = r#"
DROP TRIGGER IF EXISTS forum_05_topic_merge_redirect_edge;

CREATE TRIGGER forum_05_topic_merge_redirect_edge
BEFORE INSERT ON forum_topic_merge_operations
FOR EACH ROW
WHEN NOT EXISTS (
    SELECT 1
    FROM forum_topics source
    WHERE source.tenant_id = NEW.tenant_id
      AND source.id = NEW.source_topic_id
      AND source.deleted_at IS NULL
      AND source.status = 'archived'
      AND source.is_locked = TRUE
      AND source.reply_count = 0
) OR NOT EXISTS (
    SELECT 1
    FROM forum_topics target
    WHERE target.tenant_id = NEW.tenant_id
      AND target.id = NEW.target_topic_id
      AND target.category_id = NEW.category_id
      AND target.deleted_at IS NULL
      AND target.status <> 'archived'
)
BEGIN
    SELECT RAISE(ABORT, 'forum topic merge redirect edge is invalid');
END;
"#;

const SQLITE_DOWN: &str = r#"
DROP TRIGGER IF EXISTS forum_05_topic_merge_redirect_edge;

CREATE TRIGGER forum_05_topic_merge_redirect_edge
BEFORE INSERT ON forum_topic_merge_operations
FOR EACH ROW
WHEN NOT EXISTS (
    SELECT 1
    FROM forum_topics source
    WHERE source.tenant_id = NEW.tenant_id
      AND source.id = NEW.source_topic_id
      AND source.category_id = NEW.category_id
      AND source.deleted_at IS NULL
      AND source.status = 'archived'
      AND source.is_locked = TRUE
      AND source.reply_count = 0
) OR NOT EXISTS (
    SELECT 1
    FROM forum_topics target
    WHERE target.tenant_id = NEW.tenant_id
      AND target.id = NEW.target_topic_id
      AND target.category_id = NEW.category_id
      AND target.deleted_at IS NULL
      AND target.status <> 'archived'
)
BEGIN
    SELECT RAISE(ABORT, 'forum topic merge redirect edge is invalid');
END;
"#;
