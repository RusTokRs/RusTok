use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::ConnectionTrait;

pub(super) async fn up_postgres(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .get_connection()
        .execute_unprepared(
            r#"
ALTER TABLE forum_topics
    ADD COLUMN IF NOT EXISTS deleted_at TIMESTAMPTZ;
ALTER TABLE forum_replies
    ADD COLUMN IF NOT EXISTS deleted_at TIMESTAMPTZ;

CREATE INDEX IF NOT EXISTS idx_forum_topics_tenant_deleted
    ON forum_topics (tenant_id, deleted_at, updated_at DESC);
CREATE INDEX IF NOT EXISTS idx_forum_replies_tenant_topic_deleted
    ON forum_replies (tenant_id, topic_id, deleted_at, position);

CREATE TABLE IF NOT EXISTS forum_topic_revisions (
    id BIGSERIAL PRIMARY KEY,
    tenant_id UUID NOT NULL,
    topic_id UUID NOT NULL,
    locale VARCHAR(16) NOT NULL,
    title TEXT NOT NULL,
    slug VARCHAR(255),
    body TEXT NOT NULL,
    body_format VARCHAR(32) NOT NULL,
    metadata JSONB NOT NULL DEFAULT '{}'::jsonb,
    revision_reason VARCHAR(16) NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    CONSTRAINT chk_forum_topic_revisions_reason
        CHECK (revision_reason IN ('edit', 'delete')),
    CONSTRAINT fk_forum_topic_revisions_topic_tenant
        FOREIGN KEY (tenant_id, topic_id)
        REFERENCES forum_topics (tenant_id, id)
        ON UPDATE CASCADE
        ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_forum_topic_revisions_tenant_topic_created
    ON forum_topic_revisions (tenant_id, topic_id, created_at DESC, id DESC);

CREATE TABLE IF NOT EXISTS forum_reply_revisions (
    id BIGSERIAL PRIMARY KEY,
    tenant_id UUID NOT NULL,
    reply_id UUID NOT NULL,
    locale VARCHAR(16) NOT NULL,
    body TEXT NOT NULL,
    body_format VARCHAR(32) NOT NULL,
    revision_reason VARCHAR(16) NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    CONSTRAINT chk_forum_reply_revisions_reason
        CHECK (revision_reason IN ('edit', 'delete')),
    CONSTRAINT fk_forum_reply_revisions_reply_tenant
        FOREIGN KEY (tenant_id, reply_id)
        REFERENCES forum_replies (tenant_id, id)
        ON UPDATE CASCADE
        ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_forum_reply_revisions_tenant_reply_created
    ON forum_reply_revisions (tenant_id, reply_id, created_at DESC, id DESC);

CREATE TABLE IF NOT EXISTS forum_hard_delete_context (
    backend_pid INTEGER NOT NULL,
    tenant_id UUID NOT NULL,
    category_id UUID NOT NULL,
    topic_id UUID NOT NULL,
    PRIMARY KEY (backend_pid, topic_id)
);

CREATE OR REPLACE FUNCTION forum_capture_topic_translation_revision()
RETURNS trigger AS $$
DECLARE
    topic_metadata JSONB;
    topic_deleted_at TIMESTAMPTZ;
BEGIN
    SELECT metadata, deleted_at
      INTO topic_metadata, topic_deleted_at
      FROM forum_topics
     WHERE tenant_id = OLD.tenant_id
       AND id = OLD.topic_id;

    IF topic_deleted_at IS NOT NULL THEN
        RAISE EXCEPTION 'deleted forum topic content is immutable';
    END IF;

    IF ROW(OLD.title, OLD.slug, OLD.body, OLD.body_format)
       IS DISTINCT FROM
       ROW(NEW.title, NEW.slug, NEW.body, NEW.body_format)
    THEN
        INSERT INTO forum_topic_revisions (
            tenant_id,
            topic_id,
            locale,
            title,
            slug,
            body,
            body_format,
            metadata,
            revision_reason
        )
        VALUES (
            OLD.tenant_id,
            OLD.topic_id,
            OLD.locale,
            OLD.title,
            OLD.slug,
            OLD.body,
            OLD.body_format,
            COALESCE(topic_metadata, '{}'::jsonb),
            CASE
                WHEN NEW.title = '[deleted]' AND NEW.body = '[deleted]'
                    THEN 'delete'
                ELSE 'edit'
            END
        );
    END IF;

    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE OR REPLACE FUNCTION forum_capture_topic_metadata_revision()
RETURNS trigger AS $$
BEGIN
    IF OLD.deleted_at IS NOT NULL THEN
        RAISE EXCEPTION 'deleted forum topic is immutable';
    END IF;

    IF OLD.metadata IS DISTINCT FROM NEW.metadata THEN
        INSERT INTO forum_topic_revisions (
            tenant_id,
            topic_id,
            locale,
            title,
            slug,
            body,
            body_format,
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
            translation.body_format,
            OLD.metadata,
            'edit'
        FROM forum_topic_translations translation
        WHERE translation.tenant_id = OLD.tenant_id
          AND translation.topic_id = OLD.id;
    END IF;

    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE OR REPLACE FUNCTION forum_capture_reply_body_revision()
RETURNS trigger AS $$
DECLARE
    reply_deleted_at TIMESTAMPTZ;
BEGIN
    SELECT deleted_at
      INTO reply_deleted_at
      FROM forum_replies
     WHERE tenant_id = OLD.tenant_id
       AND id = OLD.reply_id;

    IF reply_deleted_at IS NOT NULL THEN
        RAISE EXCEPTION 'deleted forum reply content is immutable';
    END IF;

    IF ROW(OLD.body, OLD.body_format)
       IS DISTINCT FROM
       ROW(NEW.body, NEW.body_format)
    THEN
        INSERT INTO forum_reply_revisions (
            tenant_id,
            reply_id,
            locale,
            body,
            body_format,
            revision_reason
        )
        VALUES (
            OLD.tenant_id,
            OLD.reply_id,
            OLD.locale,
            OLD.body,
            OLD.body_format,
            CASE
                WHEN NEW.body = '[deleted]' THEN 'delete'
                ELSE 'edit'
            END
        );
    END IF;

    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE OR REPLACE FUNCTION forum_guard_deleted_topic_update()
RETURNS trigger AS $$
BEGIN
    IF OLD.deleted_at IS NOT NULL THEN
        RAISE EXCEPTION 'deleted forum topic is immutable';
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
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE OR REPLACE FUNCTION forum_prepare_category_hard_delete()
RETURNS trigger AS $$
BEGIN
    INSERT INTO forum_hard_delete_context (
        backend_pid,
        tenant_id,
        category_id,
        topic_id
    )
    SELECT
        pg_backend_pid(),
        topic.tenant_id,
        OLD.id,
        topic.id
    FROM forum_topics topic
    WHERE topic.tenant_id = OLD.tenant_id
      AND topic.category_id = OLD.id
    ON CONFLICT (backend_pid, topic_id) DO NOTHING;

    RETURN OLD;
END;
$$ LANGUAGE plpgsql;

CREATE OR REPLACE FUNCTION forum_finish_category_hard_delete()
RETURNS trigger AS $$
BEGIN
    DELETE FROM forum_hard_delete_context
    WHERE backend_pid = pg_backend_pid()
      AND tenant_id = OLD.tenant_id
      AND category_id = OLD.id;
    RETURN OLD;
END;
$$ LANGUAGE plpgsql;

CREATE OR REPLACE FUNCTION forum_soft_delete_reply()
RETURNS trigger AS $$
DECLARE
    category_id_value UUID;
BEGIN
    IF EXISTS (
        SELECT 1
        FROM forum_hard_delete_context context
        WHERE context.backend_pid = pg_backend_pid()
          AND context.tenant_id = OLD.tenant_id
          AND context.topic_id = OLD.topic_id
    ) THEN
        RETURN OLD;
    END IF;

    IF OLD.deleted_at IS NOT NULL THEN
        RAISE EXCEPTION 'forum reply is already deleted';
    END IF;

    SELECT category_id
      INTO category_id_value
      FROM forum_topics
     WHERE tenant_id = OLD.tenant_id
       AND id = OLD.topic_id;

    UPDATE forum_reply_bodies
       SET body = '[deleted]',
           body_format = 'markdown',
           updated_at = CURRENT_TIMESTAMP
     WHERE tenant_id = OLD.tenant_id
       AND reply_id = OLD.id;

    DELETE FROM forum_solutions
     WHERE tenant_id = OLD.tenant_id
       AND reply_id = OLD.id;

    UPDATE forum_replies
       SET status = 'deleted',
           deleted_at = CURRENT_TIMESTAMP,
           updated_at = CURRENT_TIMESTAMP
     WHERE tenant_id = OLD.tenant_id
       AND id = OLD.id;

    UPDATE forum_topics
       SET reply_count = reply_count
     WHERE tenant_id = OLD.tenant_id
       AND id = OLD.topic_id;

    IF category_id_value IS NOT NULL THEN
        UPDATE forum_categories
           SET reply_count = reply_count
         WHERE tenant_id = OLD.tenant_id
           AND id = category_id_value;
    END IF;

    IF OLD.author_id IS NOT NULL THEN
        UPDATE forum_user_stats
           SET topic_count = topic_count,
               reply_count = reply_count,
               solution_count = solution_count
         WHERE tenant_id = OLD.tenant_id
           AND user_id = OLD.author_id;
    END IF;

    RETURN NULL;
END;
$$ LANGUAGE plpgsql;

CREATE OR REPLACE FUNCTION forum_soft_delete_topic()
RETURNS trigger AS $$
BEGIN
    IF EXISTS (
        SELECT 1
        FROM forum_hard_delete_context context
        WHERE context.backend_pid = pg_backend_pid()
          AND context.tenant_id = OLD.tenant_id
          AND context.topic_id = OLD.id
    ) THEN
        RETURN OLD;
    END IF;

    IF OLD.deleted_at IS NOT NULL THEN
        RAISE EXCEPTION 'forum topic is already deleted';
    END IF;

    UPDATE forum_topic_translations
       SET title = '[deleted]',
           slug = NULL,
           body = '[deleted]',
           body_format = 'markdown',
           updated_at = CURRENT_TIMESTAMP
     WHERE tenant_id = OLD.tenant_id
       AND topic_id = OLD.id;

    UPDATE forum_reply_bodies body
       SET body = '[deleted]',
           body_format = 'markdown',
           updated_at = CURRENT_TIMESTAMP
      FROM forum_replies reply
     WHERE reply.tenant_id = OLD.tenant_id
       AND reply.topic_id = OLD.id
       AND reply.deleted_at IS NULL
       AND body.tenant_id = reply.tenant_id
       AND body.reply_id = reply.id;

    DELETE FROM forum_solutions
     WHERE tenant_id = OLD.tenant_id
       AND topic_id = OLD.id;

    UPDATE forum_replies
       SET status = 'deleted',
           deleted_at = CURRENT_TIMESTAMP,
           updated_at = CURRENT_TIMESTAMP
     WHERE tenant_id = OLD.tenant_id
       AND topic_id = OLD.id
       AND deleted_at IS NULL;

    UPDATE forum_topics
       SET status = 'archived',
           is_locked = TRUE,
           reply_count = 0,
           last_reply_at = NULL,
           deleted_at = CURRENT_TIMESTAMP,
           updated_at = CURRENT_TIMESTAMP
     WHERE tenant_id = OLD.tenant_id
       AND id = OLD.id;

    UPDATE forum_categories
       SET topic_count = topic_count,
           reply_count = reply_count
     WHERE tenant_id = OLD.tenant_id
       AND id = OLD.category_id;

    UPDATE forum_user_stats
       SET topic_count = topic_count,
           reply_count = reply_count,
           solution_count = solution_count
     WHERE tenant_id = OLD.tenant_id
       AND user_id IN (
            SELECT OLD.author_id
            UNION
            SELECT reply.author_id
            FROM forum_replies reply
            WHERE reply.tenant_id = OLD.tenant_id
              AND reply.topic_id = OLD.id
       );

    RETURN NULL;
END;
$$ LANGUAGE plpgsql;

CREATE OR REPLACE FUNCTION forum_enforce_topic_public_reply_count()
RETURNS trigger AS $$
DECLARE
    actual_count INTEGER;
    actual_last_reply_at TIMESTAMPTZ;
BEGIN
    SELECT COUNT(*)::integer, MAX(created_at)
      INTO actual_count, actual_last_reply_at
      FROM forum_replies
     WHERE tenant_id = NEW.tenant_id
       AND topic_id = NEW.id
       AND status = 'approved'
       AND deleted_at IS NULL;

    NEW.reply_count := actual_count;
    NEW.last_reply_at := actual_last_reply_at;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE OR REPLACE FUNCTION forum_enforce_category_public_reply_count()
RETURNS trigger AS $$
DECLARE
    actual_topic_count INTEGER;
    actual_reply_count INTEGER;
BEGIN
    SELECT COUNT(*)::integer
      INTO actual_topic_count
      FROM forum_topics topic
     WHERE topic.tenant_id = NEW.tenant_id
       AND topic.category_id = NEW.id
       AND topic.deleted_at IS NULL;

    SELECT COUNT(*)::integer
      INTO actual_reply_count
      FROM forum_replies reply
      JOIN forum_topics topic
        ON topic.tenant_id = reply.tenant_id
       AND topic.id = reply.topic_id
     WHERE topic.tenant_id = NEW.tenant_id
       AND topic.category_id = NEW.id
       AND topic.deleted_at IS NULL
       AND reply.status = 'approved'
       AND reply.deleted_at IS NULL;

    NEW.topic_count := actual_topic_count;
    NEW.reply_count := actual_reply_count;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE OR REPLACE FUNCTION forum_enforce_user_public_reply_count()
RETURNS trigger AS $$
DECLARE
    actual_topic_count INTEGER;
    actual_reply_count INTEGER;
    actual_solution_count INTEGER;
BEGIN
    SELECT COUNT(*)::integer
      INTO actual_topic_count
      FROM forum_topics topic
     WHERE topic.tenant_id = NEW.tenant_id
       AND topic.author_id = NEW.user_id
       AND topic.deleted_at IS NULL;

    SELECT COUNT(*)::integer
      INTO actual_reply_count
      FROM forum_replies reply
      JOIN forum_topics topic
        ON topic.tenant_id = reply.tenant_id
       AND topic.id = reply.topic_id
     WHERE reply.tenant_id = NEW.tenant_id
       AND reply.author_id = NEW.user_id
       AND reply.status = 'approved'
       AND reply.deleted_at IS NULL
       AND topic.deleted_at IS NULL;

    SELECT COUNT(*)::integer
      INTO actual_solution_count
      FROM forum_solutions solution
      JOIN forum_replies reply
        ON reply.tenant_id = solution.tenant_id
       AND reply.id = solution.reply_id
      JOIN forum_topics topic
        ON topic.tenant_id = solution.tenant_id
       AND topic.id = solution.topic_id
     WHERE solution.tenant_id = NEW.tenant_id
       AND reply.author_id = NEW.user_id
       AND reply.deleted_at IS NULL
       AND topic.deleted_at IS NULL;

    NEW.topic_count := actual_topic_count;
    NEW.reply_count := actual_reply_count;
    NEW.solution_count := actual_solution_count;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS forum_02_topic_translation_revision
    ON forum_topic_translations;
CREATE TRIGGER forum_02_topic_translation_revision
BEFORE UPDATE OF title, slug, body, body_format
ON forum_topic_translations
FOR EACH ROW
EXECUTE FUNCTION forum_capture_topic_translation_revision();

DROP TRIGGER IF EXISTS forum_02_topic_metadata_revision ON forum_topics;
CREATE TRIGGER forum_02_topic_metadata_revision
BEFORE UPDATE OF metadata
ON forum_topics
FOR EACH ROW
EXECUTE FUNCTION forum_capture_topic_metadata_revision();

DROP TRIGGER IF EXISTS forum_02_reply_body_revision ON forum_reply_bodies;
CREATE TRIGGER forum_02_reply_body_revision
BEFORE UPDATE OF body, body_format
ON forum_reply_bodies
FOR EACH ROW
EXECUTE FUNCTION forum_capture_reply_body_revision();

DROP TRIGGER IF EXISTS forum_02_topics_deleted_guard ON forum_topics;
CREATE TRIGGER forum_02_topics_deleted_guard
BEFORE UPDATE ON forum_topics
FOR EACH ROW
EXECUTE FUNCTION forum_guard_deleted_topic_update();

DROP TRIGGER IF EXISTS forum_02_replies_deleted_guard ON forum_replies;
CREATE TRIGGER forum_02_replies_deleted_guard
BEFORE UPDATE ON forum_replies
FOR EACH ROW
EXECUTE FUNCTION forum_guard_deleted_reply_update();

DROP TRIGGER IF EXISTS forum_00_categories_hard_delete_context
    ON forum_categories;
CREATE TRIGGER forum_00_categories_hard_delete_context
BEFORE DELETE ON forum_categories
FOR EACH ROW
EXECUTE FUNCTION forum_prepare_category_hard_delete();

DROP TRIGGER IF EXISTS forum_99_categories_hard_delete_cleanup
    ON forum_categories;
CREATE TRIGGER forum_99_categories_hard_delete_cleanup
AFTER DELETE ON forum_categories
FOR EACH ROW
EXECUTE FUNCTION forum_finish_category_hard_delete();

DROP TRIGGER IF EXISTS forum_02_topics_soft_delete ON forum_topics;
CREATE TRIGGER forum_02_topics_soft_delete
BEFORE DELETE ON forum_topics
FOR EACH ROW
EXECUTE FUNCTION forum_soft_delete_topic();

DROP TRIGGER IF EXISTS forum_02_replies_soft_delete ON forum_replies;
CREATE TRIGGER forum_02_replies_soft_delete
BEFORE DELETE ON forum_replies
FOR EACH ROW
EXECUTE FUNCTION forum_soft_delete_reply();

DROP TRIGGER IF EXISTS forum_90_categories_public_reply_count
    ON forum_categories;
CREATE TRIGGER forum_90_categories_public_reply_count
BEFORE INSERT OR UPDATE OF topic_count, reply_count, tenant_id, id
ON forum_categories
FOR EACH ROW
EXECUTE FUNCTION forum_enforce_category_public_reply_count();

DROP TRIGGER IF EXISTS forum_90_user_stats_public_reply_count
    ON forum_user_stats;
CREATE TRIGGER forum_90_user_stats_public_reply_count
BEFORE INSERT OR UPDATE OF topic_count, reply_count, solution_count, tenant_id, user_id
ON forum_user_stats
FOR EACH ROW
EXECUTE FUNCTION forum_enforce_user_public_reply_count();
"#,
        )
        .await?;
    Ok(())
}
