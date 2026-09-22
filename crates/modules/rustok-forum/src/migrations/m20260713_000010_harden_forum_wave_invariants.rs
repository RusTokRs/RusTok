use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::{ConnectionTrait, DatabaseBackend};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        match manager.get_database_backend() {
            DatabaseBackend::Postgres => up_postgres(manager).await,
            DatabaseBackend::Sqlite => up_sqlite(manager).await,
            backend => Err(DbErr::Custom(format!(
                "rustok-forum wave invariant migration does not support {backend:?}"
            ))),
        }
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        match manager.get_database_backend() {
            DatabaseBackend::Postgres => down_postgres(manager).await,
            DatabaseBackend::Sqlite => down_sqlite(manager).await,
            backend => Err(DbErr::Custom(format!(
                "rustok-forum wave invariant migration does not support {backend:?}"
            ))),
        }
    }
}

async fn up_postgres(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .get_connection()
        .execute_unprepared(
            r#"
ALTER TABLE forum_topic_revisions
    ALTER COLUMN locale TYPE VARCHAR(32);
ALTER TABLE forum_reply_revisions
    ALTER COLUMN locale TYPE VARCHAR(32);

ALTER TABLE forum_topics
    ADD COLUMN IF NOT EXISTS next_reply_position BIGINT NOT NULL DEFAULT 1;

UPDATE forum_topics topic
SET next_reply_position = GREATEST(
    topic.next_reply_position,
    COALESCE((
        SELECT MAX(reply.position) + 1
        FROM forum_replies reply
        WHERE reply.tenant_id = topic.tenant_id
          AND reply.topic_id = topic.id
    ), 1)
);

ALTER TABLE forum_topics
    DROP CONSTRAINT IF EXISTS chk_forum_topics_next_reply_position_positive;
ALTER TABLE forum_topics
    ADD CONSTRAINT chk_forum_topics_next_reply_position_positive
    CHECK (next_reply_position > 0);

CREATE OR REPLACE FUNCTION forum_lock_reply_counter_mutation()
RETURNS trigger AS $$
DECLARE
    row_tenant_id uuid;
    row_category_id uuid;
    row_topic_id uuid;
    row_author_id uuid;
BEGIN
    IF TG_OP = 'DELETE' THEN
        row_tenant_id := OLD.tenant_id;
        row_topic_id := OLD.topic_id;
        row_author_id := OLD.author_id;
    ELSE
        row_tenant_id := NEW.tenant_id;
        row_topic_id := NEW.topic_id;
        row_author_id := NEW.author_id;
    END IF;

    SELECT category_id
      INTO row_category_id
      FROM forum_topics
     WHERE tenant_id = row_tenant_id
       AND id = row_topic_id;

    IF row_category_id IS NOT NULL THEN
        PERFORM forum_counter_lock(
            format('forum:category:%s:%s', row_tenant_id, row_category_id)
        );
    END IF;
    PERFORM forum_counter_lock(
        format('forum:topic:%s:%s', row_tenant_id, row_topic_id)
    );
    IF row_author_id IS NOT NULL THEN
        PERFORM forum_counter_lock(
            format('forum:user:%s:%s', row_tenant_id, row_author_id)
        );
    END IF;

    IF TG_OP = 'INSERT' THEN
        UPDATE forum_topics
           SET next_reply_position = next_reply_position + 1
         WHERE tenant_id = row_tenant_id
           AND id = row_topic_id
        RETURNING next_reply_position - 1 INTO NEW.position;

        IF NOT FOUND THEN
            RAISE EXCEPTION 'forum reply topic does not exist in tenant';
        END IF;
    END IF;

    IF TG_OP = 'DELETE' THEN
        RETURN OLD;
    END IF;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE OR REPLACE FUNCTION forum_reject_nonempty_category_delete()
RETURNS trigger AS $$
BEGIN
    IF EXISTS (
        SELECT 1
        FROM forum_categories child
        WHERE child.tenant_id = OLD.tenant_id
          AND child.parent_id = OLD.id
    ) OR EXISTS (
        SELECT 1
        FROM forum_topics topic
        WHERE topic.tenant_id = OLD.tenant_id
          AND topic.category_id = OLD.id
    ) THEN
        RAISE EXCEPTION 'non-empty forum category cannot be physically deleted';
    END IF;

    RETURN OLD;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS forum_00_reject_nonempty_category_delete ON forum_categories;
CREATE TRIGGER forum_00_reject_nonempty_category_delete
BEFORE DELETE ON forum_categories
FOR EACH ROW EXECUTE FUNCTION forum_reject_nonempty_category_delete();
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
DROP TRIGGER IF EXISTS forum_00_reject_nonempty_category_delete ON forum_categories;
DROP FUNCTION IF EXISTS forum_reject_nonempty_category_delete();

CREATE OR REPLACE FUNCTION forum_lock_reply_counter_mutation()
RETURNS trigger AS $$
DECLARE
    row_tenant_id uuid;
    row_category_id uuid;
    row_topic_id uuid;
    row_author_id uuid;
BEGIN
    IF TG_OP = 'DELETE' THEN
        row_tenant_id := OLD.tenant_id;
        row_topic_id := OLD.topic_id;
        row_author_id := OLD.author_id;
    ELSE
        row_tenant_id := NEW.tenant_id;
        row_topic_id := NEW.topic_id;
        row_author_id := NEW.author_id;
    END IF;

    SELECT category_id
      INTO row_category_id
      FROM forum_topics
     WHERE tenant_id = row_tenant_id
       AND id = row_topic_id;

    IF row_category_id IS NOT NULL THEN
        PERFORM forum_counter_lock(
            format('forum:category:%s:%s', row_tenant_id, row_category_id)
        );
    END IF;
    PERFORM forum_counter_lock(
        format('forum:topic:%s:%s', row_tenant_id, row_topic_id)
    );
    IF row_author_id IS NOT NULL THEN
        PERFORM forum_counter_lock(
            format('forum:user:%s:%s', row_tenant_id, row_author_id)
        );
    END IF;

    IF TG_OP = 'INSERT' THEN
        SELECT COALESCE(MAX(reply.position), 0) + 1
          INTO NEW.position
          FROM forum_replies reply
         WHERE reply.tenant_id = row_tenant_id
           AND reply.topic_id = row_topic_id;
    END IF;

    IF TG_OP = 'DELETE' THEN
        RETURN OLD;
    END IF;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

ALTER TABLE forum_topics
    DROP CONSTRAINT IF EXISTS chk_forum_topics_next_reply_position_positive;
ALTER TABLE forum_topics
    DROP COLUMN IF EXISTS next_reply_position;

-- Locale widths are intentionally not reduced during rollback. Narrowing
-- forum revision locale columns can truncate valid normalized tags.
"#,
        )
        .await?;
    Ok(())
}

async fn up_sqlite(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    let connection = manager.get_connection();

    for statement in [
        "ALTER TABLE forum_topics ADD COLUMN next_reply_position INTEGER NOT NULL DEFAULT 1",
        r#"UPDATE forum_topics
           SET next_reply_position = MAX(
               next_reply_position,
               COALESCE((
                   SELECT MAX(reply.position) + 1
                   FROM forum_replies reply
                   WHERE reply.tenant_id = forum_topics.tenant_id
                     AND reply.topic_id = forum_topics.id
               ), 1)
           )"#,
        "DROP TRIGGER IF EXISTS forum_topics_next_reply_position_positive_insert",
        "DROP TRIGGER IF EXISTS forum_topics_next_reply_position_positive_update",
        "DROP TRIGGER IF EXISTS forum_replies_advance_next_position",
        "DROP TRIGGER IF EXISTS forum_reject_nonempty_category_delete",
        r#"CREATE TRIGGER forum_topics_next_reply_position_positive_insert
           BEFORE INSERT ON forum_topics
           FOR EACH ROW
           WHEN NEW.next_reply_position < 1
           BEGIN
               SELECT RAISE(ABORT, 'forum next reply position must be positive');
           END"#,
        r#"CREATE TRIGGER forum_topics_next_reply_position_positive_update
           BEFORE UPDATE OF next_reply_position ON forum_topics
           FOR EACH ROW
           WHEN NEW.next_reply_position < 1
           BEGIN
               SELECT RAISE(ABORT, 'forum next reply position must be positive');
           END"#,
        r#"CREATE TRIGGER forum_replies_advance_next_position
           AFTER INSERT ON forum_replies
           FOR EACH ROW
           BEGIN
               UPDATE forum_topics
                  SET next_reply_position = MAX(next_reply_position, NEW.position + 1)
                WHERE tenant_id = NEW.tenant_id
                  AND id = NEW.topic_id;
           END"#,
        r#"CREATE TRIGGER forum_reject_nonempty_category_delete
           BEFORE DELETE ON forum_categories
           FOR EACH ROW
           WHEN EXISTS (
               SELECT 1
               FROM forum_categories child
               WHERE child.tenant_id = OLD.tenant_id
                 AND child.parent_id = OLD.id
           ) OR EXISTS (
               SELECT 1
               FROM forum_topics topic
               WHERE topic.tenant_id = OLD.tenant_id
                 AND topic.category_id = OLD.id
           )
           BEGIN
               SELECT RAISE(ABORT, 'non-empty forum category cannot be physically deleted');
           END"#,
    ] {
        connection.execute_unprepared(statement).await?;
    }

    Ok(())
}

async fn down_sqlite(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    let connection = manager.get_connection();

    for statement in [
        "DROP TRIGGER IF EXISTS forum_reject_nonempty_category_delete",
        "DROP TRIGGER IF EXISTS forum_replies_advance_next_position",
        "DROP TRIGGER IF EXISTS forum_topics_next_reply_position_positive_update",
        "DROP TRIGGER IF EXISTS forum_topics_next_reply_position_positive_insert",
        "ALTER TABLE forum_topics DROP COLUMN next_reply_position",
    ] {
        connection.execute_unprepared(statement).await?;
    }

    Ok(())
}
