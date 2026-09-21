use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::DatabaseBackend;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        match manager.get_database_backend() {
            DatabaseBackend::Postgres => up_postgres(manager).await,
            DatabaseBackend::Sqlite => up_sqlite(manager).await,
            backend => Err(DbErr::Custom(format!(
                "rustok-forum topic restore migration does not support {backend:?}"
            ))),
        }
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        match manager.get_database_backend() {
            DatabaseBackend::Postgres => down_postgres(manager).await,
            DatabaseBackend::Sqlite => down_sqlite(manager).await,
            backend => Err(DbErr::Custom(format!(
                "rustok-forum topic restore rollback does not support {backend:?}"
            ))),
        }
    }
}

async fn up_postgres(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .get_connection()
        .execute_unprepared(
            r#"
ALTER TABLE forum_topics
    ADD COLUMN IF NOT EXISTS deleted_from_status VARCHAR(32),
    ADD COLUMN IF NOT EXISTS deleted_from_locked BOOLEAN;

CREATE OR REPLACE FUNCTION forum_guard_deleted_topic_update_with_restore()
RETURNS trigger AS $$
BEGIN
    IF OLD.deleted_at IS NOT NULL AND NEW.deleted_at IS NULL THEN
        IF NEW.status::text <> COALESCE(OLD.deleted_from_status, 'archived')
           OR NEW.is_locked <> COALESCE(OLD.deleted_from_locked, TRUE)
           OR NEW.deleted_from_status IS NOT NULL
           OR NEW.deleted_from_locked IS NOT NULL
           OR NEW.id <> OLD.id
           OR NEW.tenant_id <> OLD.tenant_id
           OR NEW.category_id <> OLD.category_id
           OR NEW.author_id IS DISTINCT FROM OLD.author_id
           OR NEW.metadata IS DISTINCT FROM OLD.metadata
           OR NEW.is_pinned <> OLD.is_pinned
           OR NEW.reply_count <> OLD.reply_count
           OR NEW.created_at <> OLD.created_at
           OR NEW.last_reply_at IS DISTINCT FROM OLD.last_reply_at
        THEN
            RAISE EXCEPTION 'deleted forum topic restore contains unsupported changes';
        END IF;
        RETURN NEW;
    END IF;

    IF OLD.deleted_at IS NOT NULL THEN
        RAISE EXCEPTION 'deleted forum topic is immutable';
    END IF;

    IF NEW.deleted_at IS NOT NULL THEN
        INSERT INTO forum_topic_revisions (
            tenant_id,
            topic_id,
            locale,
            title,
            slug,
            body,
            metadata,
            revision_reason
        )
        SELECT
            OLD.tenant_id,
            OLD.id,
            translation.locale,
            translation.title,
            translation.slug,
            translation.body,
            OLD.metadata,
            'delete'
        FROM forum_topic_translations translation
        WHERE translation.tenant_id = OLD.tenant_id
          AND translation.topic_id = OLD.id;
    END IF;

    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS forum_02_topics_deleted_guard
    ON forum_topics;
CREATE TRIGGER forum_02_topics_deleted_guard
BEFORE UPDATE ON forum_topics
FOR EACH ROW
EXECUTE FUNCTION forum_guard_deleted_topic_update_with_restore();
"#,
        )
        .await?;
    Ok(())
}

async fn down_postgres(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .get_connection()
        .execute_unprepared(
            r#"
DROP TRIGGER IF EXISTS forum_02_topics_deleted_guard
    ON forum_topics;

CREATE TRIGGER forum_02_topics_deleted_guard
BEFORE UPDATE ON forum_topics
FOR EACH ROW
EXECUTE FUNCTION forum_guard_deleted_topic_update();

DROP FUNCTION IF EXISTS forum_guard_deleted_topic_update_with_restore();

ALTER TABLE forum_topics
    DROP COLUMN IF EXISTS deleted_from_locked,
    DROP COLUMN IF EXISTS deleted_from_status;
"#,
        )
        .await?;
    Ok(())
}

async fn up_sqlite(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .get_connection()
        .execute_unprepared(
            r#"
ALTER TABLE forum_topics ADD COLUMN deleted_from_status TEXT;
ALTER TABLE forum_topics ADD COLUMN deleted_from_locked INTEGER;

DROP TRIGGER IF EXISTS forum_topics_deleted_guard;
CREATE TRIGGER forum_topics_deleted_guard
BEFORE UPDATE ON forum_topics
FOR EACH ROW
BEGIN
    SELECT CASE
        WHEN OLD.deleted_at IS NOT NULL
         AND NEW.deleted_at IS NULL
         AND (
            NEW.status <> COALESCE(OLD.deleted_from_status, 'archived')
            OR NEW.is_locked <> COALESCE(OLD.deleted_from_locked, 1)
            OR NEW.deleted_from_status IS NOT NULL
            OR NEW.deleted_from_locked IS NOT NULL
            OR NEW.id <> OLD.id
            OR NEW.tenant_id <> OLD.tenant_id
            OR NEW.category_id <> OLD.category_id
            OR NEW.author_id IS NOT OLD.author_id
            OR NEW.metadata IS NOT OLD.metadata
            OR NEW.is_pinned <> OLD.is_pinned
            OR NEW.reply_count <> OLD.reply_count
            OR NEW.created_at <> OLD.created_at
            OR NEW.last_reply_at IS NOT OLD.last_reply_at
         )
        THEN RAISE(ABORT, 'deleted forum topic restore contains unsupported changes')
    END;

    SELECT CASE
        WHEN OLD.deleted_at IS NOT NULL
         AND NEW.deleted_at IS NOT NULL
        THEN RAISE(ABORT, 'deleted forum topic is immutable')
    END;

    SELECT CASE
        WHEN OLD.deleted_at IS NULL
         AND NEW.deleted_at IS NOT NULL
        THEN NULL
    END;
END;
"#,
        )
        .await?;
    Ok(())
}

async fn down_sqlite(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .get_connection()
        .execute_unprepared(
            r#"
DROP TRIGGER IF EXISTS forum_topics_deleted_guard;
CREATE TRIGGER forum_topics_deleted_guard
BEFORE UPDATE ON forum_topics
FOR EACH ROW
BEGIN
    SELECT CASE
        WHEN OLD.deleted_at IS NOT NULL
        THEN RAISE(ABORT, 'deleted forum topic is immutable')
    END;

    SELECT CASE
        WHEN NEW.deleted_at IS NOT NULL
        THEN NULL
    END;
END;

ALTER TABLE forum_topics DROP COLUMN deleted_from_locked;
ALTER TABLE forum_topics DROP COLUMN deleted_from_status;
"#,
        )
        .await?;
    Ok(())
}
