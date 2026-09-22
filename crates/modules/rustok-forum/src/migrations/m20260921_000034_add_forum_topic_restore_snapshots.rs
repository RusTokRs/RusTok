use sea_orm::DatabaseBackend;
use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        match manager.get_database_backend() {
            DatabaseBackend::Postgres => up_postgres(manager).await,
            DatabaseBackend::Sqlite => up_sqlite(manager).await,
            backend => Err(DbErr::Custom(format!(
                "rustok-forum topic restore snapshots do not support {backend:?}"
            ))),
        }
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        match manager.get_database_backend() {
            DatabaseBackend::Postgres => down_postgres(manager).await,
            DatabaseBackend::Sqlite => down_sqlite(manager).await,
            backend => Err(DbErr::Custom(format!(
                "rustok-forum topic restore snapshots do not support {backend:?}"
            ))),
        }
    }
}

async fn up_postgres(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager.get_connection().execute_unprepared(
        r#"
CREATE TABLE forum_topic_delete_snapshots (
    tenant_id UUID NOT NULL,
    topic_id UUID NOT NULL,
    previous_status TEXT NOT NULL
        CHECK (previous_status IN ('open', 'closed', 'archived')),
    previous_is_locked BOOLEAN NOT NULL,
    solution_reply_id UUID,
    solution_author_id UUID,
    solution_marked_by_user_id UUID,
    solution_marked_at TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    PRIMARY KEY (tenant_id, topic_id),
    FOREIGN KEY (topic_id) REFERENCES forum_topics (id) ON DELETE CASCADE
);

CREATE TABLE forum_topic_reply_delete_snapshots (
    tenant_id UUID NOT NULL,
    topic_id UUID NOT NULL,
    reply_id UUID NOT NULL,
    author_id UUID,
    previous_status TEXT NOT NULL
        CHECK (previous_status IN ('pending', 'approved', 'rejected', 'hidden', 'flagged')),
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    PRIMARY KEY (tenant_id, topic_id, reply_id),
    FOREIGN KEY (topic_id) REFERENCES forum_topics (id) ON DELETE CASCADE,
    FOREIGN KEY (reply_id) REFERENCES forum_replies (id) ON DELETE CASCADE
);

CREATE INDEX idx_forum_topic_delete_snapshots_topic
    ON forum_topic_delete_snapshots (tenant_id, topic_id);
CREATE INDEX idx_forum_topic_reply_delete_snapshots_topic
    ON forum_topic_reply_delete_snapshots (tenant_id, topic_id);

CREATE OR REPLACE FUNCTION forum_guard_deleted_topic_update()
RETURNS trigger AS $$
BEGIN
    IF OLD.deleted_at IS NOT NULL THEN
        IF NEW.deleted_at IS NULL
           AND NEW.id = OLD.id
           AND NEW.tenant_id = OLD.tenant_id
           AND NEW.category_id = OLD.category_id
           AND NEW.author_id IS NOT DISTINCT FROM OLD.author_id
           AND NEW.metadata IS NOT DISTINCT FROM OLD.metadata
           AND NEW.is_pinned = OLD.is_pinned
           AND NEW.created_at = OLD.created_at
           AND NEW.status::text = (SELECT previous_status FROM forum_topic_delete_snapshots WHERE tenant_id = OLD.tenant_id AND topic_id = OLD.id)
           AND NEW.is_locked = (SELECT previous_is_locked FROM forum_topic_delete_snapshots WHERE tenant_id = OLD.tenant_id AND topic_id = OLD.id)
           AND EXISTS (SELECT 1 FROM forum_topic_delete_snapshots WHERE tenant_id = OLD.tenant_id AND topic_id = OLD.id)
        THEN
            RETURN NEW;
        END IF;
        RAISE EXCEPTION 'deleted forum topic is immutable';
    END IF;

    IF NEW.deleted_at IS NOT NULL THEN
        INSERT INTO forum_topic_revisions (
            tenant_id, topic_id, locale, title, slug, body, metadata, revision_reason
        )
        SELECT OLD.tenant_id, OLD.id, translation.locale, translation.title, translation.slug,
               translation.body, OLD.metadata, 'delete'
        FROM forum_topic_translations translation
        WHERE translation.tenant_id = OLD.tenant_id AND translation.topic_id = OLD.id;
    END IF;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE OR REPLACE FUNCTION forum_guard_deleted_reply_update()
RETURNS trigger AS $$
BEGIN
    IF OLD.deleted_at IS NOT NULL THEN
        IF NEW.deleted_at IS NULL
           AND NEW.id = OLD.id
           AND NEW.tenant_id = OLD.tenant_id
           AND NEW.topic_id = OLD.topic_id
           AND NEW.author_id IS NOT DISTINCT FROM OLD.author_id
           AND NEW.parent_reply_id IS NOT DISTINCT FROM OLD.parent_reply_id
           AND NEW.position = OLD.position
           AND NEW.created_at = OLD.created_at
           AND NEW.status::text = (SELECT previous_status FROM forum_topic_reply_delete_snapshots WHERE tenant_id = OLD.tenant_id AND topic_id = OLD.topic_id AND reply_id = OLD.id)
           AND EXISTS (SELECT 1 FROM forum_topic_reply_delete_snapshots WHERE tenant_id = OLD.tenant_id AND topic_id = OLD.topic_id AND reply_id = OLD.id)
        THEN
            RETURN NEW;
        END IF;
        RAISE EXCEPTION 'deleted forum reply is immutable';
    END IF;

    IF NEW.deleted_at IS NOT NULL THEN
        INSERT INTO forum_reply_revisions (
            tenant_id, reply_id, locale, body, revision_reason
        )
        SELECT OLD.tenant_id, OLD.id, body.locale, body.body, 'delete'
        FROM forum_reply_bodies body
        WHERE body.tenant_id = OLD.tenant_id AND body.reply_id = OLD.id;
    END IF;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;
"#).await?;
    Ok(())
}

async fn down_postgres(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager.get_connection().execute_unprepared(
        r#"
CREATE OR REPLACE FUNCTION forum_guard_deleted_topic_update()
RETURNS trigger AS $$
BEGIN
    IF OLD.deleted_at IS NOT NULL THEN
        RAISE EXCEPTION 'deleted forum topic is immutable';
    END IF;
    IF NEW.deleted_at IS NOT NULL THEN
        INSERT INTO forum_topic_revisions (
            tenant_id, topic_id, locale, title, slug, body, metadata, revision_reason
        )
        SELECT OLD.tenant_id, OLD.id, translation.locale, translation.title, translation.slug,
               translation.body, OLD.metadata, 'delete'
        FROM forum_topic_translations translation
        WHERE translation.tenant_id = OLD.tenant_id AND translation.topic_id = OLD.id;
    END IF;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE OR REPLACE FUNCTION forum_guard_deleted_reply_update()
RETURNS trigger AS $$
BEGIN
    IF OLD.deleted_at IS NOT NULL THEN
        RAISE EXCEPTION 'deleted forum reply is immutable';
    END IF;
    IF NEW.deleted_at IS NOT NULL THEN
        INSERT INTO forum_reply_revisions (
            tenant_id, reply_id, locale, body, revision_reason
        )
        SELECT OLD.tenant_id, OLD.id, body.locale, body.body, 'delete'
        FROM forum_reply_bodies body
        WHERE body.tenant_id = OLD.tenant_id AND body.reply_id = OLD.id;
    END IF;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TABLE forum_topic_reply_delete_snapshots;
DROP TABLE forum_topic_delete_snapshots;
"#).await?;
    Ok(())
}

async fn up_sqlite(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    let connection = manager.get_connection();
    for statement in [
        r#"CREATE TABLE forum_topic_delete_snapshots (
            tenant_id TEXT NOT NULL,
            topic_id TEXT NOT NULL,
            previous_status TEXT NOT NULL CHECK (previous_status IN ('open', 'closed', 'archived')),
            previous_is_locked INTEGER NOT NULL,
            solution_reply_id TEXT,
            solution_author_id TEXT,
            solution_marked_by_user_id TEXT,
            solution_marked_at TEXT,
            created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            PRIMARY KEY (tenant_id, topic_id),
            FOREIGN KEY (topic_id) REFERENCES forum_topics (id) ON DELETE CASCADE
        )"#,
        r#"CREATE TABLE forum_topic_reply_delete_snapshots (
            tenant_id TEXT NOT NULL,
            topic_id TEXT NOT NULL,
            reply_id TEXT NOT NULL,
            author_id TEXT,
            previous_status TEXT NOT NULL CHECK (previous_status IN ('pending', 'approved', 'rejected', 'hidden', 'flagged')),
            created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            PRIMARY KEY (tenant_id, topic_id, reply_id),
            FOREIGN KEY (topic_id) REFERENCES forum_topics (id) ON DELETE CASCADE,
            FOREIGN KEY (reply_id) REFERENCES forum_replies (id) ON DELETE CASCADE
        )"#,
        "CREATE INDEX idx_forum_topic_delete_snapshots_topic ON forum_topic_delete_snapshots (tenant_id, topic_id)",
        "CREATE INDEX idx_forum_topic_reply_delete_snapshots_topic ON forum_topic_reply_delete_snapshots (tenant_id, topic_id)",
        "DROP TRIGGER IF EXISTS forum_topics_deleted_update_guard",
        "DROP TRIGGER IF EXISTS forum_replies_deleted_update_guard",
        r#"CREATE TRIGGER forum_topics_deleted_update_guard
        BEFORE UPDATE ON forum_topics
        FOR EACH ROW
        WHEN OLD.deleted_at IS NOT NULL
        BEGIN
            SELECT CASE
                WHEN NEW.deleted_at IS NOT NULL
                  OR NEW.id IS NOT OLD.id
                  OR NEW.tenant_id IS NOT OLD.tenant_id
                  OR NEW.category_id IS NOT OLD.category_id
                  OR NEW.author_id IS NOT OLD.author_id
                  OR NEW.metadata IS NOT OLD.metadata
                  OR NEW.is_pinned IS NOT OLD.is_pinned
                  OR NEW.created_at IS NOT OLD.created_at
                  OR NEW.status IS NOT (SELECT previous_status FROM forum_topic_delete_snapshots WHERE tenant_id = OLD.tenant_id AND topic_id = OLD.id)
                  OR NEW.is_locked IS NOT (SELECT previous_is_locked FROM forum_topic_delete_snapshots WHERE tenant_id = OLD.tenant_id AND topic_id = OLD.id)
                  OR NOT EXISTS (SELECT 1 FROM forum_topic_delete_snapshots WHERE tenant_id = OLD.tenant_id AND topic_id = OLD.id)
                THEN RAISE(ABORT, 'deleted forum topic is immutable')
            END;
        END"#,
        r#"CREATE TRIGGER forum_replies_deleted_update_guard
        BEFORE UPDATE ON forum_replies
        FOR EACH ROW
        WHEN OLD.deleted_at IS NOT NULL
        BEGIN
            SELECT CASE
                WHEN NEW.deleted_at IS NOT NULL
                  OR NEW.id IS NOT OLD.id
                  OR NEW.tenant_id IS NOT OLD.tenant_id
                  OR NEW.topic_id IS NOT OLD.topic_id
                  OR NEW.author_id IS NOT OLD.author_id
                  OR NEW.parent_reply_id IS NOT OLD.parent_reply_id
                  OR NEW.position IS NOT OLD.position
                  OR NEW.created_at IS NOT OLD.created_at
                  OR NEW.status IS NOT (SELECT previous_status FROM forum_topic_reply_delete_snapshots WHERE tenant_id = OLD.tenant_id AND topic_id = OLD.topic_id AND reply_id = OLD.id)
                  OR NOT EXISTS (SELECT 1 FROM forum_topic_reply_delete_snapshots WHERE tenant_id = OLD.tenant_id AND topic_id = OLD.topic_id AND reply_id = OLD.id)
                THEN RAISE(ABORT, 'deleted forum reply is immutable')
            END;
        END"#,
    ] {
        connection.execute_unprepared(statement).await?;
    }
    Ok(())
}

async fn down_sqlite(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    let connection = manager.get_connection();
    for statement in [
        "DROP TRIGGER IF EXISTS forum_topics_deleted_update_guard",
        "DROP TRIGGER IF EXISTS forum_replies_deleted_update_guard",
        r#"CREATE TRIGGER forum_topics_deleted_update_guard
        BEFORE UPDATE ON forum_topics
        FOR EACH ROW
        WHEN OLD.deleted_at IS NOT NULL
        BEGIN
            SELECT RAISE(ABORT, 'deleted forum topic is immutable');
        END"#,
        r#"CREATE TRIGGER forum_replies_deleted_update_guard
        BEFORE UPDATE ON forum_replies
        FOR EACH ROW
        WHEN OLD.deleted_at IS NOT NULL
        BEGIN
            SELECT RAISE(ABORT, 'deleted forum reply is immutable');
        END"#,
        "DROP TABLE forum_topic_reply_delete_snapshots",
        "DROP TABLE forum_topic_delete_snapshots",
    ] {
        connection.execute_unprepared(statement).await?;
    }
    Ok(())
}
