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
                "rustok-forum moderation subject revision migration does not support {backend:?}"
            ))),
        }
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        match manager.get_database_backend() {
            DatabaseBackend::Postgres => execute(manager, POSTGRES_DOWN).await,
            DatabaseBackend::Sqlite => execute(manager, SQLITE_DOWN).await,
            backend => Err(DbErr::Custom(format!(
                "rustok-forum moderation subject revision rollback does not support {backend:?}"
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
CREATE TABLE IF NOT EXISTS forum_topic_moderation_subject_revisions (
    tenant_id UUID NOT NULL,
    topic_id UUID NOT NULL,
    revision BIGINT NOT NULL,
    CONSTRAINT pk_forum_topic_moderation_subject_revisions
        PRIMARY KEY (tenant_id, topic_id),
    CONSTRAINT fk_forum_topic_moderation_subject_revision_topic
        FOREIGN KEY (tenant_id, topic_id)
        REFERENCES forum_topics (tenant_id, id)
        ON UPDATE CASCADE ON DELETE CASCADE,
    CONSTRAINT ck_forum_topic_moderation_subject_revision_positive
        CHECK (revision > 0)
);

CREATE TABLE IF NOT EXISTS forum_reply_moderation_subject_revisions (
    tenant_id UUID NOT NULL,
    reply_id UUID NOT NULL,
    revision BIGINT NOT NULL,
    CONSTRAINT pk_forum_reply_moderation_subject_revisions
        PRIMARY KEY (tenant_id, reply_id),
    CONSTRAINT fk_forum_reply_moderation_subject_revision_reply
        FOREIGN KEY (tenant_id, reply_id)
        REFERENCES forum_replies (tenant_id, id)
        ON UPDATE CASCADE ON DELETE CASCADE,
    CONSTRAINT ck_forum_reply_moderation_subject_revision_positive
        CHECK (revision > 0)
);

INSERT INTO forum_topic_moderation_subject_revisions (tenant_id, topic_id, revision)
SELECT tenant_id, id, 1
FROM forum_topics
ON CONFLICT (tenant_id, topic_id) DO NOTHING;

INSERT INTO forum_reply_moderation_subject_revisions (tenant_id, reply_id, revision)
SELECT tenant_id, id, 1
FROM forum_replies
ON CONFLICT (tenant_id, reply_id) DO NOTHING;

CREATE OR REPLACE FUNCTION forum_initialize_topic_moderation_subject_revision()
RETURNS trigger AS $$
BEGIN
    INSERT INTO forum_topic_moderation_subject_revisions (tenant_id, topic_id, revision)
    VALUES (NEW.tenant_id, NEW.id, 1)
    ON CONFLICT (tenant_id, topic_id) DO NOTHING;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE OR REPLACE FUNCTION forum_initialize_reply_moderation_subject_revision()
RETURNS trigger AS $$
BEGIN
    INSERT INTO forum_reply_moderation_subject_revisions (tenant_id, reply_id, revision)
    VALUES (NEW.tenant_id, NEW.id, 1)
    ON CONFLICT (tenant_id, reply_id) DO NOTHING;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE OR REPLACE FUNCTION forum_bump_topic_moderation_subject_revision()
RETURNS trigger AS $$
DECLARE
    target_tenant UUID;
    target_topic UUID;
BEGIN
    IF TG_OP = 'DELETE' THEN
        target_tenant := OLD.tenant_id;
        target_topic := OLD.topic_id;
    ELSE
        target_tenant := NEW.tenant_id;
        target_topic := NEW.topic_id;
    END IF;

    UPDATE forum_topic_moderation_subject_revisions
       SET revision = revision + 1
     WHERE tenant_id = target_tenant
       AND topic_id = target_topic;
    RETURN COALESCE(NEW, OLD);
END;
$$ LANGUAGE plpgsql;

CREATE OR REPLACE FUNCTION forum_bump_reply_moderation_subject_revision()
RETURNS trigger AS $$
DECLARE
    target_tenant UUID;
    target_reply UUID;
BEGIN
    IF TG_OP = 'DELETE' THEN
        target_tenant := OLD.tenant_id;
        target_reply := OLD.reply_id;
    ELSE
        target_tenant := NEW.tenant_id;
        target_reply := NEW.reply_id;
    END IF;

    UPDATE forum_reply_moderation_subject_revisions
       SET revision = revision + 1
     WHERE tenant_id = target_tenant
       AND reply_id = target_reply;
    RETURN COALESCE(NEW, OLD);
END;
$$ LANGUAGE plpgsql;

CREATE OR REPLACE FUNCTION forum_bump_topic_moderation_subject_revision_on_owner_update()
RETURNS trigger AS $$
BEGIN
    IF ROW(
        OLD.category_id,
        OLD.author_id,
        OLD.status,
        OLD.metadata,
        OLD.is_pinned,
        OLD.is_locked,
        OLD.deleted_at
    ) IS DISTINCT FROM ROW(
        NEW.category_id,
        NEW.author_id,
        NEW.status,
        NEW.metadata,
        NEW.is_pinned,
        NEW.is_locked,
        NEW.deleted_at
    ) THEN
        UPDATE forum_topic_moderation_subject_revisions
           SET revision = revision + 1
         WHERE tenant_id = NEW.tenant_id
           AND topic_id = NEW.id;
    END IF;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE OR REPLACE FUNCTION forum_bump_reply_moderation_subject_revision_on_owner_update()
RETURNS trigger AS $$
BEGIN
    IF ROW(
        OLD.topic_id,
        OLD.author_id,
        OLD.parent_reply_id,
        OLD.status,
        OLD.position,
        OLD.deleted_at
    ) IS DISTINCT FROM ROW(
        NEW.topic_id,
        NEW.author_id,
        NEW.parent_reply_id,
        NEW.status,
        NEW.position,
        NEW.deleted_at
    ) THEN
        UPDATE forum_reply_moderation_subject_revisions
           SET revision = revision + 1
         WHERE tenant_id = NEW.tenant_id
           AND reply_id = NEW.id;
    END IF;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE OR REPLACE FUNCTION forum_bump_topic_moderation_subject_revision_on_translation_update()
RETURNS trigger AS $$
BEGIN
    IF ROW(OLD.locale, OLD.title, OLD.slug, OLD.body)
       IS DISTINCT FROM ROW(NEW.locale, NEW.title, NEW.slug, NEW.body)
    THEN
        UPDATE forum_topic_moderation_subject_revisions
           SET revision = revision + 1
         WHERE tenant_id = NEW.tenant_id
           AND topic_id = NEW.topic_id;
    END IF;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE OR REPLACE FUNCTION forum_bump_reply_moderation_subject_revision_on_body_update()
RETURNS trigger AS $$
BEGIN
    IF ROW(OLD.locale, OLD.body) IS DISTINCT FROM ROW(NEW.locale, NEW.body) THEN
        UPDATE forum_reply_moderation_subject_revisions
           SET revision = revision + 1
         WHERE tenant_id = NEW.tenant_id
           AND reply_id = NEW.reply_id;
    END IF;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS forum_topic_moderation_subject_revision_insert ON forum_topics;
CREATE TRIGGER forum_topic_moderation_subject_revision_insert
AFTER INSERT ON forum_topics
FOR EACH ROW EXECUTE FUNCTION forum_initialize_topic_moderation_subject_revision();

DROP TRIGGER IF EXISTS forum_reply_moderation_subject_revision_insert ON forum_replies;
CREATE TRIGGER forum_reply_moderation_subject_revision_insert
AFTER INSERT ON forum_replies
FOR EACH ROW EXECUTE FUNCTION forum_initialize_reply_moderation_subject_revision();

DROP TRIGGER IF EXISTS forum_topic_moderation_subject_revision_owner_update ON forum_topics;
CREATE TRIGGER forum_topic_moderation_subject_revision_owner_update
AFTER UPDATE ON forum_topics
FOR EACH ROW EXECUTE FUNCTION forum_bump_topic_moderation_subject_revision_on_owner_update();

DROP TRIGGER IF EXISTS forum_reply_moderation_subject_revision_owner_update ON forum_replies;
CREATE TRIGGER forum_reply_moderation_subject_revision_owner_update
AFTER UPDATE ON forum_replies
FOR EACH ROW EXECUTE FUNCTION forum_bump_reply_moderation_subject_revision_on_owner_update();

DROP TRIGGER IF EXISTS forum_topic_moderation_subject_revision_translation_insert ON forum_topic_translations;
CREATE TRIGGER forum_topic_moderation_subject_revision_translation_insert
AFTER INSERT ON forum_topic_translations
FOR EACH ROW EXECUTE FUNCTION forum_bump_topic_moderation_subject_revision();

DROP TRIGGER IF EXISTS forum_topic_moderation_subject_revision_translation_update ON forum_topic_translations;
CREATE TRIGGER forum_topic_moderation_subject_revision_translation_update
AFTER UPDATE ON forum_topic_translations
FOR EACH ROW EXECUTE FUNCTION forum_bump_topic_moderation_subject_revision_on_translation_update();

DROP TRIGGER IF EXISTS forum_topic_moderation_subject_revision_translation_delete ON forum_topic_translations;
CREATE TRIGGER forum_topic_moderation_subject_revision_translation_delete
AFTER DELETE ON forum_topic_translations
FOR EACH ROW EXECUTE FUNCTION forum_bump_topic_moderation_subject_revision();

DROP TRIGGER IF EXISTS forum_reply_moderation_subject_revision_body_insert ON forum_reply_bodies;
CREATE TRIGGER forum_reply_moderation_subject_revision_body_insert
AFTER INSERT ON forum_reply_bodies
FOR EACH ROW EXECUTE FUNCTION forum_bump_reply_moderation_subject_revision();

DROP TRIGGER IF EXISTS forum_reply_moderation_subject_revision_body_update ON forum_reply_bodies;
CREATE TRIGGER forum_reply_moderation_subject_revision_body_update
AFTER UPDATE ON forum_reply_bodies
FOR EACH ROW EXECUTE FUNCTION forum_bump_reply_moderation_subject_revision_on_body_update();

DROP TRIGGER IF EXISTS forum_reply_moderation_subject_revision_body_delete ON forum_reply_bodies;
CREATE TRIGGER forum_reply_moderation_subject_revision_body_delete
AFTER DELETE ON forum_reply_bodies
FOR EACH ROW EXECUTE FUNCTION forum_bump_reply_moderation_subject_revision();
"#;

const POSTGRES_DOWN: &str = r#"
DROP TRIGGER IF EXISTS forum_reply_moderation_subject_revision_body_delete ON forum_reply_bodies;
DROP TRIGGER IF EXISTS forum_reply_moderation_subject_revision_body_update ON forum_reply_bodies;
DROP TRIGGER IF EXISTS forum_reply_moderation_subject_revision_body_insert ON forum_reply_bodies;
DROP TRIGGER IF EXISTS forum_topic_moderation_subject_revision_translation_delete ON forum_topic_translations;
DROP TRIGGER IF EXISTS forum_topic_moderation_subject_revision_translation_update ON forum_topic_translations;
DROP TRIGGER IF EXISTS forum_topic_moderation_subject_revision_translation_insert ON forum_topic_translations;
DROP TRIGGER IF EXISTS forum_reply_moderation_subject_revision_owner_update ON forum_replies;
DROP TRIGGER IF EXISTS forum_topic_moderation_subject_revision_owner_update ON forum_topics;
DROP TRIGGER IF EXISTS forum_reply_moderation_subject_revision_insert ON forum_replies;
DROP TRIGGER IF EXISTS forum_topic_moderation_subject_revision_insert ON forum_topics;
DROP FUNCTION IF EXISTS forum_bump_reply_moderation_subject_revision_on_body_update();
DROP FUNCTION IF EXISTS forum_bump_topic_moderation_subject_revision_on_translation_update();
DROP FUNCTION IF EXISTS forum_bump_reply_moderation_subject_revision_on_owner_update();
DROP FUNCTION IF EXISTS forum_bump_topic_moderation_subject_revision_on_owner_update();
DROP FUNCTION IF EXISTS forum_bump_reply_moderation_subject_revision();
DROP FUNCTION IF EXISTS forum_bump_topic_moderation_subject_revision();
DROP FUNCTION IF EXISTS forum_initialize_reply_moderation_subject_revision();
DROP FUNCTION IF EXISTS forum_initialize_topic_moderation_subject_revision();
DROP TABLE IF EXISTS forum_reply_moderation_subject_revisions;
DROP TABLE IF EXISTS forum_topic_moderation_subject_revisions;
"#;

const SQLITE_UP: &str = r#"
CREATE TABLE IF NOT EXISTS forum_topic_moderation_subject_revisions (
    tenant_id TEXT NOT NULL,
    topic_id TEXT NOT NULL,
    revision INTEGER NOT NULL,
    PRIMARY KEY (tenant_id, topic_id),
    FOREIGN KEY (tenant_id, topic_id)
        REFERENCES forum_topics (tenant_id, id)
        ON UPDATE CASCADE ON DELETE CASCADE,
    CHECK (revision > 0)
);

CREATE TABLE IF NOT EXISTS forum_reply_moderation_subject_revisions (
    tenant_id TEXT NOT NULL,
    reply_id TEXT NOT NULL,
    revision INTEGER NOT NULL,
    PRIMARY KEY (tenant_id, reply_id),
    FOREIGN KEY (tenant_id, reply_id)
        REFERENCES forum_replies (tenant_id, id)
        ON UPDATE CASCADE ON DELETE CASCADE,
    CHECK (revision > 0)
);

INSERT OR IGNORE INTO forum_topic_moderation_subject_revisions (tenant_id, topic_id, revision)
SELECT tenant_id, id, 1 FROM forum_topics;

INSERT OR IGNORE INTO forum_reply_moderation_subject_revisions (tenant_id, reply_id, revision)
SELECT tenant_id, id, 1 FROM forum_replies;

CREATE TRIGGER IF NOT EXISTS forum_topic_moderation_subject_revision_insert
AFTER INSERT ON forum_topics
FOR EACH ROW
BEGIN
    INSERT OR IGNORE INTO forum_topic_moderation_subject_revisions (tenant_id, topic_id, revision)
    VALUES (NEW.tenant_id, NEW.id, 1);
END;

CREATE TRIGGER IF NOT EXISTS forum_reply_moderation_subject_revision_insert
AFTER INSERT ON forum_replies
FOR EACH ROW
BEGIN
    INSERT OR IGNORE INTO forum_reply_moderation_subject_revisions (tenant_id, reply_id, revision)
    VALUES (NEW.tenant_id, NEW.id, 1);
END;

CREATE TRIGGER IF NOT EXISTS forum_topic_moderation_subject_revision_owner_update
AFTER UPDATE ON forum_topics
FOR EACH ROW
WHEN OLD.category_id IS NOT NEW.category_id
  OR OLD.author_id IS NOT NEW.author_id
  OR OLD.status IS NOT NEW.status
  OR OLD.metadata IS NOT NEW.metadata
  OR OLD.is_pinned IS NOT NEW.is_pinned
  OR OLD.is_locked IS NOT NEW.is_locked
  OR OLD.deleted_at IS NOT NEW.deleted_at
BEGIN
    UPDATE forum_topic_moderation_subject_revisions
       SET revision = revision + 1
     WHERE tenant_id = NEW.tenant_id
       AND topic_id = NEW.id;
END;

CREATE TRIGGER IF NOT EXISTS forum_reply_moderation_subject_revision_owner_update
AFTER UPDATE ON forum_replies
FOR EACH ROW
WHEN OLD.topic_id IS NOT NEW.topic_id
  OR OLD.author_id IS NOT NEW.author_id
  OR OLD.parent_reply_id IS NOT NEW.parent_reply_id
  OR OLD.status IS NOT NEW.status
  OR OLD.position IS NOT NEW.position
  OR OLD.deleted_at IS NOT NEW.deleted_at
BEGIN
    UPDATE forum_reply_moderation_subject_revisions
       SET revision = revision + 1
     WHERE tenant_id = NEW.tenant_id
       AND reply_id = NEW.id;
END;

CREATE TRIGGER IF NOT EXISTS forum_topic_moderation_subject_revision_translation_insert
AFTER INSERT ON forum_topic_translations
FOR EACH ROW
BEGIN
    UPDATE forum_topic_moderation_subject_revisions
       SET revision = revision + 1
     WHERE tenant_id = NEW.tenant_id
       AND topic_id = NEW.topic_id;
END;

CREATE TRIGGER IF NOT EXISTS forum_topic_moderation_subject_revision_translation_update
AFTER UPDATE ON forum_topic_translations
FOR EACH ROW
WHEN OLD.locale IS NOT NEW.locale
  OR OLD.title IS NOT NEW.title
  OR OLD.slug IS NOT NEW.slug
  OR OLD.body IS NOT NEW.body
BEGIN
    UPDATE forum_topic_moderation_subject_revisions
       SET revision = revision + 1
     WHERE tenant_id = NEW.tenant_id
       AND topic_id = NEW.topic_id;
END;

CREATE TRIGGER IF NOT EXISTS forum_topic_moderation_subject_revision_translation_delete
AFTER DELETE ON forum_topic_translations
FOR EACH ROW
BEGIN
    UPDATE forum_topic_moderation_subject_revisions
       SET revision = revision + 1
     WHERE tenant_id = OLD.tenant_id
       AND topic_id = OLD.topic_id;
END;

CREATE TRIGGER IF NOT EXISTS forum_reply_moderation_subject_revision_body_insert
AFTER INSERT ON forum_reply_bodies
FOR EACH ROW
BEGIN
    UPDATE forum_reply_moderation_subject_revisions
       SET revision = revision + 1
     WHERE tenant_id = NEW.tenant_id
       AND reply_id = NEW.reply_id;
END;

CREATE TRIGGER IF NOT EXISTS forum_reply_moderation_subject_revision_body_update
AFTER UPDATE ON forum_reply_bodies
FOR EACH ROW
WHEN OLD.locale IS NOT NEW.locale OR OLD.body IS NOT NEW.body
BEGIN
    UPDATE forum_reply_moderation_subject_revisions
       SET revision = revision + 1
     WHERE tenant_id = NEW.tenant_id
       AND reply_id = NEW.reply_id;
END;

CREATE TRIGGER IF NOT EXISTS forum_reply_moderation_subject_revision_body_delete
AFTER DELETE ON forum_reply_bodies
FOR EACH ROW
BEGIN
    UPDATE forum_reply_moderation_subject_revisions
       SET revision = revision + 1
     WHERE tenant_id = OLD.tenant_id
       AND reply_id = OLD.reply_id;
END;
"#;

const SQLITE_DOWN: &str = r#"
DROP TRIGGER IF EXISTS forum_reply_moderation_subject_revision_body_delete;
DROP TRIGGER IF EXISTS forum_reply_moderation_subject_revision_body_update;
DROP TRIGGER IF EXISTS forum_reply_moderation_subject_revision_body_insert;
DROP TRIGGER IF EXISTS forum_topic_moderation_subject_revision_translation_delete;
DROP TRIGGER IF EXISTS forum_topic_moderation_subject_revision_translation_update;
DROP TRIGGER IF EXISTS forum_topic_moderation_subject_revision_translation_insert;
DROP TRIGGER IF EXISTS forum_reply_moderation_subject_revision_owner_update;
DROP TRIGGER IF EXISTS forum_topic_moderation_subject_revision_owner_update;
DROP TRIGGER IF EXISTS forum_reply_moderation_subject_revision_insert;
DROP TRIGGER IF EXISTS forum_topic_moderation_subject_revision_insert;
DROP TABLE IF EXISTS forum_reply_moderation_subject_revisions;
DROP TABLE IF EXISTS forum_topic_moderation_subject_revisions;
"#;
