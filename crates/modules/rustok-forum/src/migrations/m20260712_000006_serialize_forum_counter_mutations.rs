use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::{ConnectionTrait, DatabaseBackend};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        match manager.get_database_backend() {
            DatabaseBackend::Postgres => up_postgres(manager).await,
            // SQLite serializes writers at the database level. The service-level
            // transactions therefore already provide the critical section that
            // PostgreSQL needs explicit advisory locks for.
            DatabaseBackend::Sqlite => Ok(()),
            backend => Err(DbErr::Custom(format!(
                "rustok-forum counter serialization migration does not support {backend:?}"
            ))),
        }
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        match manager.get_database_backend() {
            DatabaseBackend::Postgres => down_postgres(manager).await,
            DatabaseBackend::Sqlite => Ok(()),
            backend => Err(DbErr::Custom(format!(
                "rustok-forum counter serialization migration does not support {backend:?}"
            ))),
        }
    }
}

async fn up_postgres(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .get_connection()
        .execute_unprepared(
            r#"
CREATE OR REPLACE FUNCTION forum_counter_lock(scope_key text)
RETURNS void AS $$
BEGIN
    PERFORM pg_advisory_xact_lock(hashtextextended(scope_key, 0));
END;
$$ LANGUAGE plpgsql;

CREATE OR REPLACE FUNCTION forum_lock_topic_counter_mutation()
RETURNS trigger AS $$
DECLARE
    row_tenant_id uuid;
    row_category_id uuid;
    row_topic_id uuid;
    row_author_id uuid;
BEGIN
    IF TG_OP = 'DELETE' THEN
        row_tenant_id := OLD.tenant_id;
        row_category_id := OLD.category_id;
        row_topic_id := OLD.id;
        row_author_id := OLD.author_id;
    ELSE
        row_tenant_id := NEW.tenant_id;
        row_category_id := NEW.category_id;
        row_topic_id := NEW.id;
        row_author_id := NEW.author_id;
    END IF;

    -- Every mutation takes locks in category -> topic -> user order. Keeping one
    -- global order prevents deadlocks while serializing all read-modify-write
    -- counter updates performed later in the same transaction.
    PERFORM forum_counter_lock(
        format('forum:category:%s:%s', row_tenant_id, row_category_id)
    );
    PERFORM forum_counter_lock(
        format('forum:topic:%s:%s', row_tenant_id, row_topic_id)
    );
    IF row_author_id IS NOT NULL THEN
        PERFORM forum_counter_lock(
            format('forum:user:%s:%s', row_tenant_id, row_author_id)
        );
    END IF;

    IF TG_OP = 'DELETE' THEN
        RETURN OLD;
    END IF;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

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

    -- During a cascading topic delete the parent row may already be invisible,
    -- but its topic trigger has already acquired the category and topic locks.
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

    IF TG_OP = 'DELETE' THEN
        RETURN OLD;
    END IF;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE OR REPLACE FUNCTION forum_lock_solution_counter_mutation()
RETURNS trigger AS $$
DECLARE
    row_tenant_id uuid;
    row_category_id uuid;
    row_topic_id uuid;
    row_reply_id uuid;
    row_author_id uuid;
BEGIN
    IF TG_OP = 'DELETE' THEN
        row_tenant_id := OLD.tenant_id;
        row_topic_id := OLD.topic_id;
        row_reply_id := OLD.reply_id;
    ELSE
        row_tenant_id := NEW.tenant_id;
        row_topic_id := NEW.topic_id;
        row_reply_id := NEW.reply_id;
    END IF;

    SELECT topic.category_id, reply.author_id
      INTO row_category_id, row_author_id
      FROM forum_topics topic
      LEFT JOIN forum_replies reply
        ON reply.tenant_id = row_tenant_id
       AND reply.id = row_reply_id
     WHERE topic.tenant_id = row_tenant_id
       AND topic.id = row_topic_id;

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

    IF TG_OP = 'DELETE' THEN
        RETURN OLD;
    END IF;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS forum_00_topics_counter_lock ON forum_topics;
CREATE TRIGGER forum_00_topics_counter_lock
BEFORE INSERT OR DELETE ON forum_topics
FOR EACH ROW EXECUTE FUNCTION forum_lock_topic_counter_mutation();

DROP TRIGGER IF EXISTS forum_00_replies_counter_lock ON forum_replies;
CREATE TRIGGER forum_00_replies_counter_lock
BEFORE INSERT OR DELETE ON forum_replies
FOR EACH ROW EXECUTE FUNCTION forum_lock_reply_counter_mutation();

DROP TRIGGER IF EXISTS forum_00_solutions_counter_lock ON forum_solutions;
CREATE TRIGGER forum_00_solutions_counter_lock
BEFORE INSERT OR DELETE ON forum_solutions
FOR EACH ROW EXECUTE FUNCTION forum_lock_solution_counter_mutation();
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
DROP TRIGGER IF EXISTS forum_00_solutions_counter_lock ON forum_solutions;
DROP TRIGGER IF EXISTS forum_00_replies_counter_lock ON forum_replies;
DROP TRIGGER IF EXISTS forum_00_topics_counter_lock ON forum_topics;
DROP FUNCTION IF EXISTS forum_lock_solution_counter_mutation();
DROP FUNCTION IF EXISTS forum_lock_reply_counter_mutation();
DROP FUNCTION IF EXISTS forum_lock_topic_counter_mutation();
DROP FUNCTION IF EXISTS forum_counter_lock(text);
"#,
        )
        .await?;
    Ok(())
}
