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
                "rustok-forum topic merge solution policy migration does not support {backend:?}"
            ))),
        }
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        match manager.get_database_backend() {
            DatabaseBackend::Postgres => execute(manager, POSTGRES_DOWN).await,
            DatabaseBackend::Sqlite => execute(manager, SQLITE_DOWN).await,
            backend => Err(DbErr::Custom(format!(
                "rustok-forum topic merge solution policy migration does not support {backend:?}"
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
CREATE TABLE IF NOT EXISTS forum_topic_solution_locks (
    tenant_id UUID NOT NULL,
    topic_id UUID NOT NULL,
    touched_at TIMESTAMPTZ NOT NULL,
    PRIMARY KEY (tenant_id, topic_id),
    CONSTRAINT fk_forum_topic_solution_locks_topic
        FOREIGN KEY (tenant_id, topic_id)
        REFERENCES forum_topics (tenant_id, id)
        ON UPDATE CASCADE ON DELETE CASCADE
);

CREATE OR REPLACE FUNCTION forum_lock_topic_solution_mutation()
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
            RAISE EXCEPTION 'forum solution tenant is immutable';
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

    PERFORM 1
      FROM forum_topics
     WHERE tenant_id = row_tenant_id
       AND id = first_topic_id
     FOR SHARE;
    IF NOT FOUND THEN
        RAISE EXCEPTION 'forum solution topic is unavailable';
    END IF;
    IF second_topic_id IS NOT NULL AND second_topic_id <> first_topic_id THEN
        PERFORM 1
          FROM forum_topics
         WHERE tenant_id = row_tenant_id
           AND id = second_topic_id
         FOR SHARE;
        IF NOT FOUND THEN
            RAISE EXCEPTION 'forum solution topic is unavailable';
        END IF;
    END IF;

    PERFORM pg_advisory_xact_lock(hashtextextended(
        format('%s:%s', row_tenant_id, first_topic_id),
        31
    ));
    IF second_topic_id IS NOT NULL AND second_topic_id <> first_topic_id THEN
        PERFORM pg_advisory_xact_lock(hashtextextended(
            format('%s:%s', row_tenant_id, second_topic_id),
            31
        ));
    END IF;

    IF TG_OP = 'DELETE' THEN
        RETURN OLD;
    END IF;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE OR REPLACE FUNCTION forum_validate_topic_solution_target()
RETURNS trigger AS $$
BEGIN
    IF NOT EXISTS (
        SELECT 1
        FROM forum_topics topic
        JOIN forum_replies reply
          ON reply.tenant_id = topic.tenant_id
         AND reply.topic_id = topic.id
         AND reply.id = NEW.reply_id
        WHERE topic.tenant_id = NEW.tenant_id
          AND topic.id = NEW.topic_id
          AND topic.deleted_at IS NULL
          AND topic.status::text <> 'archived'
          AND reply.deleted_at IS NULL
          AND reply.status::text = 'approved'
    ) THEN
        RAISE EXCEPTION 'forum solution requires an active topic and approved reply';
    END IF;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS forum_00_topic_solution_scope ON forum_solutions;
CREATE TRIGGER forum_00_topic_solution_scope
BEFORE INSERT OR UPDATE OR DELETE ON forum_solutions
FOR EACH ROW EXECUTE FUNCTION forum_lock_topic_solution_mutation();

DROP TRIGGER IF EXISTS forum_10_topic_solution_target ON forum_solutions;
CREATE TRIGGER forum_10_topic_solution_target
BEFORE INSERT OR UPDATE OF tenant_id, topic_id, reply_id ON forum_solutions
FOR EACH ROW EXECUTE FUNCTION forum_validate_topic_solution_target();
"#;

const POSTGRES_DOWN: &str = r#"
DROP TRIGGER IF EXISTS forum_10_topic_solution_target ON forum_solutions;
DROP TRIGGER IF EXISTS forum_00_topic_solution_scope ON forum_solutions;
DROP FUNCTION IF EXISTS forum_validate_topic_solution_target();
DROP FUNCTION IF EXISTS forum_lock_topic_solution_mutation();
DROP TABLE IF EXISTS forum_topic_solution_locks;
"#;

const SQLITE_UP: &str = r#"
CREATE TABLE IF NOT EXISTS forum_topic_solution_locks (
    tenant_id TEXT NOT NULL,
    topic_id TEXT NOT NULL,
    touched_at TEXT NOT NULL,
    PRIMARY KEY (tenant_id, topic_id),
    FOREIGN KEY (tenant_id, topic_id)
        REFERENCES forum_topics (tenant_id, id)
        ON UPDATE CASCADE ON DELETE CASCADE
);

CREATE TRIGGER IF NOT EXISTS forum_00_topic_solution_scope_insert
BEFORE INSERT ON forum_solutions
FOR EACH ROW
BEGIN
    INSERT INTO forum_topic_solution_locks (tenant_id, topic_id, touched_at)
    VALUES (NEW.tenant_id, NEW.topic_id, CURRENT_TIMESTAMP)
    ON CONFLICT(tenant_id, topic_id) DO UPDATE SET touched_at = CURRENT_TIMESTAMP;
END;

CREATE TRIGGER IF NOT EXISTS forum_00_topic_solution_scope_update
BEFORE UPDATE OF tenant_id, topic_id, reply_id ON forum_solutions
FOR EACH ROW
BEGIN
    INSERT INTO forum_topic_solution_locks (tenant_id, topic_id, touched_at)
    VALUES (OLD.tenant_id, OLD.topic_id, CURRENT_TIMESTAMP)
    ON CONFLICT(tenant_id, topic_id) DO UPDATE SET touched_at = CURRENT_TIMESTAMP;
    INSERT INTO forum_topic_solution_locks (tenant_id, topic_id, touched_at)
    VALUES (NEW.tenant_id, NEW.topic_id, CURRENT_TIMESTAMP)
    ON CONFLICT(tenant_id, topic_id) DO UPDATE SET touched_at = CURRENT_TIMESTAMP;
END;

CREATE TRIGGER IF NOT EXISTS forum_00_topic_solution_scope_delete
BEFORE DELETE ON forum_solutions
FOR EACH ROW
BEGIN
    INSERT INTO forum_topic_solution_locks (tenant_id, topic_id, touched_at)
    VALUES (OLD.tenant_id, OLD.topic_id, CURRENT_TIMESTAMP)
    ON CONFLICT(tenant_id, topic_id) DO UPDATE SET touched_at = CURRENT_TIMESTAMP;
END;

CREATE TRIGGER IF NOT EXISTS forum_10_topic_solution_target_insert
BEFORE INSERT ON forum_solutions
FOR EACH ROW
WHEN NOT EXISTS (
    SELECT 1
    FROM forum_topics topic
    JOIN forum_replies reply
      ON reply.tenant_id = topic.tenant_id
     AND reply.topic_id = topic.id
     AND reply.id = NEW.reply_id
    WHERE topic.tenant_id = NEW.tenant_id
      AND topic.id = NEW.topic_id
      AND topic.deleted_at IS NULL
      AND topic.status <> 'archived'
      AND reply.deleted_at IS NULL
      AND reply.status = 'approved'
)
BEGIN
    SELECT RAISE(ABORT, 'forum solution requires an active topic and approved reply');
END;

CREATE TRIGGER IF NOT EXISTS forum_10_topic_solution_target_update
BEFORE UPDATE OF tenant_id, topic_id, reply_id ON forum_solutions
FOR EACH ROW
WHEN NOT EXISTS (
    SELECT 1
    FROM forum_topics topic
    JOIN forum_replies reply
      ON reply.tenant_id = topic.tenant_id
     AND reply.topic_id = topic.id
     AND reply.id = NEW.reply_id
    WHERE topic.tenant_id = NEW.tenant_id
      AND topic.id = NEW.topic_id
      AND topic.deleted_at IS NULL
      AND topic.status <> 'archived'
      AND reply.deleted_at IS NULL
      AND reply.status = 'approved'
)
BEGIN
    SELECT RAISE(ABORT, 'forum solution requires an active topic and approved reply');
END;
"#;

const SQLITE_DOWN: &str = r#"
DROP TRIGGER IF EXISTS forum_10_topic_solution_target_update;
DROP TRIGGER IF EXISTS forum_10_topic_solution_target_insert;
DROP TRIGGER IF EXISTS forum_00_topic_solution_scope_delete;
DROP TRIGGER IF EXISTS forum_00_topic_solution_scope_update;
DROP TRIGGER IF EXISTS forum_00_topic_solution_scope_insert;
DROP TABLE IF EXISTS forum_topic_solution_locks;
"#;
