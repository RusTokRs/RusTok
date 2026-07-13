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
                "rustok-forum reply publication migration does not support {backend:?}"
            ))),
        }
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        match manager.get_database_backend() {
            DatabaseBackend::Postgres => down_postgres(manager).await,
            DatabaseBackend::Sqlite => down_sqlite(manager).await,
            backend => Err(DbErr::Custom(format!(
                "rustok-forum reply publication migration does not support {backend:?}"
            ))),
        }
    }
}

async fn up_postgres(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .get_connection()
        .execute_unprepared(
            r#"
CREATE OR REPLACE FUNCTION forum_validate_reply_creation()
RETURNS trigger AS $$
DECLARE
    topic_status text;
    topic_locked boolean;
BEGIN
    SELECT status::text, is_locked
      INTO topic_status, topic_locked
      FROM forum_topics
     WHERE tenant_id = NEW.tenant_id
       AND id = NEW.topic_id;

    IF topic_locked THEN
        RAISE EXCEPTION 'forum topic is locked';
    END IF;
    IF topic_status IS NOT NULL AND topic_status <> 'open' THEN
        RAISE EXCEPTION 'forum topic is not open';
    END IF;

    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE OR REPLACE FUNCTION forum_enforce_topic_public_reply_count()
RETURNS trigger AS $$
DECLARE
    actual_count integer;
    actual_last_reply_at timestamptz;
BEGIN
    SELECT COUNT(*)::integer, MAX(created_at)
      INTO actual_count, actual_last_reply_at
      FROM forum_replies
     WHERE tenant_id = NEW.tenant_id
       AND topic_id = NEW.id
       AND status = 'approved';

    NEW.reply_count := actual_count;
    NEW.last_reply_at := actual_last_reply_at;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE OR REPLACE FUNCTION forum_enforce_category_public_reply_count()
RETURNS trigger AS $$
DECLARE
    actual_count integer;
BEGIN
    SELECT COUNT(*)::integer
      INTO actual_count
      FROM forum_replies reply
      JOIN forum_topics topic
        ON topic.tenant_id = reply.tenant_id
       AND topic.id = reply.topic_id
     WHERE topic.tenant_id = NEW.tenant_id
       AND topic.category_id = NEW.id
       AND reply.status = 'approved';

    NEW.reply_count := actual_count;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE OR REPLACE FUNCTION forum_enforce_user_public_reply_count()
RETURNS trigger AS $$
DECLARE
    actual_count integer;
BEGIN
    SELECT COUNT(*)::integer
      INTO actual_count
      FROM forum_replies
     WHERE tenant_id = NEW.tenant_id
       AND author_id = NEW.user_id
       AND status = 'approved';

    NEW.reply_count := actual_count;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE OR REPLACE FUNCTION forum_filter_topic_replied_event()
RETURNS trigger AS $$
DECLARE
    event_reply_id uuid;
    event_tenant_id uuid;
BEGIN
    IF NEW.event_type <> 'forum.topic.replied' THEN
        RETURN NEW;
    END IF;

    event_reply_id := NULLIF(
        NEW.payload #>> '{event,data,reply_id}',
        ''
    )::uuid;
    event_tenant_id := NULLIF(
        NEW.payload ->> 'tenant_id',
        ''
    )::uuid;

    IF event_reply_id IS NULL
       OR event_tenant_id IS NULL
       OR NOT EXISTS (
            SELECT 1
            FROM forum_replies reply
            WHERE reply.tenant_id = event_tenant_id
              AND reply.id = event_reply_id
              AND reply.status = 'approved'
       )
    THEN
        RETURN NULL;
    END IF;

    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS forum_01_reply_creation_guard ON forum_replies;
CREATE TRIGGER forum_01_reply_creation_guard
BEFORE INSERT ON forum_replies
FOR EACH ROW EXECUTE FUNCTION forum_validate_reply_creation();

DROP TRIGGER IF EXISTS forum_00_replies_publication_lock ON forum_replies;
CREATE TRIGGER forum_00_replies_publication_lock
BEFORE UPDATE OF status, tenant_id, topic_id, author_id ON forum_replies
FOR EACH ROW EXECUTE FUNCTION forum_lock_reply_counter_mutation();

DROP TRIGGER IF EXISTS forum_90_topics_public_reply_count ON forum_topics;
CREATE TRIGGER forum_90_topics_public_reply_count
BEFORE INSERT OR UPDATE OF reply_count ON forum_topics
FOR EACH ROW EXECUTE FUNCTION forum_enforce_topic_public_reply_count();

DROP TRIGGER IF EXISTS forum_90_categories_public_reply_count ON forum_categories;
CREATE TRIGGER forum_90_categories_public_reply_count
BEFORE INSERT OR UPDATE OF reply_count ON forum_categories
FOR EACH ROW EXECUTE FUNCTION forum_enforce_category_public_reply_count();

DROP TRIGGER IF EXISTS forum_90_user_stats_public_reply_count ON forum_user_stats;
CREATE TRIGGER forum_90_user_stats_public_reply_count
BEFORE INSERT OR UPDATE OF reply_count, tenant_id, user_id ON forum_user_stats
FOR EACH ROW EXECUTE FUNCTION forum_enforce_user_public_reply_count();

DROP TRIGGER IF EXISTS forum_01_topic_replied_visibility ON sys_events;
CREATE TRIGGER forum_01_topic_replied_visibility
BEFORE INSERT ON sys_events
FOR EACH ROW EXECUTE FUNCTION forum_filter_topic_replied_event();
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
DROP TRIGGER IF EXISTS forum_01_topic_replied_visibility ON sys_events;
DROP TRIGGER IF EXISTS forum_90_user_stats_public_reply_count ON forum_user_stats;
DROP TRIGGER IF EXISTS forum_90_categories_public_reply_count ON forum_categories;
DROP TRIGGER IF EXISTS forum_90_topics_public_reply_count ON forum_topics;
DROP TRIGGER IF EXISTS forum_00_replies_publication_lock ON forum_replies;
DROP TRIGGER IF EXISTS forum_01_reply_creation_guard ON forum_replies;
DROP FUNCTION IF EXISTS forum_filter_topic_replied_event();
DROP FUNCTION IF EXISTS forum_enforce_user_public_reply_count();
DROP FUNCTION IF EXISTS forum_enforce_category_public_reply_count();
DROP FUNCTION IF EXISTS forum_enforce_topic_public_reply_count();
DROP FUNCTION IF EXISTS forum_validate_reply_creation();
"#,
        )
        .await?;
    Ok(())
}

async fn up_sqlite(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    let connection = manager.get_connection();
    for statement in [
        r#"DROP TRIGGER IF EXISTS forum_replies_locked_topic_insert"#,
        r#"DROP TRIGGER IF EXISTS forum_replies_inactive_topic_insert"#,
        r#"DROP TRIGGER IF EXISTS forum_topics_public_reply_count_update"#,
        r#"DROP TRIGGER IF EXISTS forum_categories_public_reply_count_update"#,
        r#"DROP TRIGGER IF EXISTS forum_user_stats_public_reply_count_insert"#,
        r#"DROP TRIGGER IF EXISTS forum_user_stats_public_reply_count_update"#,
        r#"DROP TRIGGER IF EXISTS forum_replies_publication_insert"#,
        r#"DROP TRIGGER IF EXISTS forum_replies_publication_delete"#,
        r#"DROP TRIGGER IF EXISTS forum_replies_publication_update"#,
        r#"DROP TRIGGER IF EXISTS forum_topic_replied_visibility_insert"#,
        r#"CREATE TRIGGER forum_replies_locked_topic_insert
BEFORE INSERT ON forum_replies
FOR EACH ROW
WHEN EXISTS (
    SELECT 1
    FROM forum_topics topic
    WHERE topic.tenant_id = NEW.tenant_id
      AND topic.id = NEW.topic_id
      AND topic.is_locked = 1
)
BEGIN
    SELECT RAISE(ABORT, 'forum topic is locked');
END"#,
        r#"CREATE TRIGGER forum_replies_inactive_topic_insert
BEFORE INSERT ON forum_replies
FOR EACH ROW
WHEN EXISTS (
    SELECT 1
    FROM forum_topics topic
    WHERE topic.tenant_id = NEW.tenant_id
      AND topic.id = NEW.topic_id
      AND topic.status <> 'open'
)
BEGIN
    SELECT RAISE(ABORT, 'forum topic is not open');
END"#,
        r#"CREATE TRIGGER forum_topics_public_reply_count_update
AFTER UPDATE OF reply_count ON forum_topics
FOR EACH ROW
WHEN (
    NEW.reply_count <> (
        SELECT COUNT(*)
        FROM forum_replies reply
        WHERE reply.tenant_id = NEW.tenant_id
          AND reply.topic_id = NEW.id
          AND reply.status = 'approved'
    )
    OR COALESCE(NEW.last_reply_at, '') <> COALESCE((
        SELECT MAX(reply.created_at)
        FROM forum_replies reply
        WHERE reply.tenant_id = NEW.tenant_id
          AND reply.topic_id = NEW.id
          AND reply.status = 'approved'
    ), '')
)
BEGIN
    UPDATE forum_topics
    SET reply_count = (
            SELECT COUNT(*)
            FROM forum_replies reply
            WHERE reply.tenant_id = NEW.tenant_id
              AND reply.topic_id = NEW.id
              AND reply.status = 'approved'
        ),
        last_reply_at = (
            SELECT MAX(reply.created_at)
            FROM forum_replies reply
            WHERE reply.tenant_id = NEW.tenant_id
              AND reply.topic_id = NEW.id
              AND reply.status = 'approved'
        )
    WHERE tenant_id = NEW.tenant_id
      AND id = NEW.id;
END"#,
        r#"CREATE TRIGGER forum_categories_public_reply_count_update
AFTER UPDATE OF reply_count ON forum_categories
FOR EACH ROW
WHEN NEW.reply_count <> (
    SELECT COUNT(*)
    FROM forum_replies reply
    JOIN forum_topics topic
      ON topic.tenant_id = reply.tenant_id
     AND topic.id = reply.topic_id
    WHERE topic.tenant_id = NEW.tenant_id
      AND topic.category_id = NEW.id
      AND reply.status = 'approved'
)
BEGIN
    UPDATE forum_categories
    SET reply_count = (
        SELECT COUNT(*)
        FROM forum_replies reply
        JOIN forum_topics topic
          ON topic.tenant_id = reply.tenant_id
         AND topic.id = reply.topic_id
        WHERE topic.tenant_id = NEW.tenant_id
          AND topic.category_id = NEW.id
          AND reply.status = 'approved'
    )
    WHERE tenant_id = NEW.tenant_id
      AND id = NEW.id;
END"#,
        r#"CREATE TRIGGER forum_user_stats_public_reply_count_insert
AFTER INSERT ON forum_user_stats
FOR EACH ROW
WHEN NEW.reply_count <> (
    SELECT COUNT(*)
    FROM forum_replies reply
    WHERE reply.tenant_id = NEW.tenant_id
      AND reply.author_id = NEW.user_id
      AND reply.status = 'approved'
)
BEGIN
    UPDATE forum_user_stats
    SET reply_count = (
            SELECT COUNT(*)
            FROM forum_replies reply
            WHERE reply.tenant_id = NEW.tenant_id
              AND reply.author_id = NEW.user_id
              AND reply.status = 'approved'
        ),
        updated_at = CURRENT_TIMESTAMP
    WHERE tenant_id = NEW.tenant_id
      AND user_id = NEW.user_id;
END"#,
        r#"CREATE TRIGGER forum_user_stats_public_reply_count_update
AFTER UPDATE OF reply_count ON forum_user_stats
FOR EACH ROW
WHEN NEW.reply_count <> (
    SELECT COUNT(*)
    FROM forum_replies reply
    WHERE reply.tenant_id = NEW.tenant_id
      AND reply.author_id = NEW.user_id
      AND reply.status = 'approved'
)
BEGIN
    UPDATE forum_user_stats
    SET reply_count = (
            SELECT COUNT(*)
            FROM forum_replies reply
            WHERE reply.tenant_id = NEW.tenant_id
              AND reply.author_id = NEW.user_id
              AND reply.status = 'approved'
        ),
        updated_at = CURRENT_TIMESTAMP
    WHERE tenant_id = NEW.tenant_id
      AND user_id = NEW.user_id;
END"#,
        r#"CREATE TRIGGER forum_topic_replied_visibility_insert
BEFORE INSERT ON sys_events
FOR EACH ROW
WHEN NEW.event_type = 'forum.topic.replied'
 AND NOT EXISTS (
    SELECT 1
    FROM forum_replies reply
    WHERE reply.id = json_extract(NEW.payload, '$.event.data.reply_id')
      AND reply.tenant_id = json_extract(NEW.payload, '$.tenant_id')
      AND reply.status = 'approved'
 )
BEGIN
    SELECT RAISE(IGNORE);
END"#
    ] {
        connection.execute_unprepared(statement).await?;
    }
    Ok(())
}

async fn down_sqlite(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    let connection = manager.get_connection();
    for statement in [
        "DROP TRIGGER IF EXISTS forum_topic_replied_visibility_insert",
        "DROP TRIGGER IF EXISTS forum_user_stats_public_reply_count_update",
        "DROP TRIGGER IF EXISTS forum_user_stats_public_reply_count_insert",
        "DROP TRIGGER IF EXISTS forum_categories_public_reply_count_update",
        "DROP TRIGGER IF EXISTS forum_topics_public_reply_count_update",
        "DROP TRIGGER IF EXISTS forum_replies_inactive_topic_insert",
        "DROP TRIGGER IF EXISTS forum_replies_locked_topic_insert"
    ] {
        connection.execute_unprepared(statement).await?;
    }
    Ok(())
}
