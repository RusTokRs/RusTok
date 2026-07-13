use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::ConnectionTrait;

pub(super) async fn down_postgres(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .get_connection()
        .execute_unprepared(
            r#"
DROP TRIGGER IF EXISTS forum_02_replies_soft_delete ON forum_replies;
DROP TRIGGER IF EXISTS forum_02_topics_soft_delete ON forum_topics;
DROP TRIGGER IF EXISTS forum_99_categories_hard_delete_cleanup
    ON forum_categories;
DROP TRIGGER IF EXISTS forum_00_categories_hard_delete_context
    ON forum_categories;
DROP TRIGGER IF EXISTS forum_02_replies_deleted_guard ON forum_replies;
DROP TRIGGER IF EXISTS forum_02_topics_deleted_guard ON forum_topics;
DROP TRIGGER IF EXISTS forum_02_reply_body_revision ON forum_reply_bodies;
DROP TRIGGER IF EXISTS forum_02_topic_metadata_revision ON forum_topics;
DROP TRIGGER IF EXISTS forum_02_topic_translation_revision
    ON forum_topic_translations;

DROP FUNCTION IF EXISTS forum_soft_delete_reply();
DROP FUNCTION IF EXISTS forum_soft_delete_topic();
DROP FUNCTION IF EXISTS forum_finish_category_hard_delete();
DROP FUNCTION IF EXISTS forum_prepare_category_hard_delete();
DROP FUNCTION IF EXISTS forum_guard_deleted_reply_update();
DROP FUNCTION IF EXISTS forum_guard_deleted_topic_update();
DROP FUNCTION IF EXISTS forum_capture_reply_body_revision();
DROP FUNCTION IF EXISTS forum_capture_topic_metadata_revision();
DROP FUNCTION IF EXISTS forum_capture_topic_translation_revision();

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
       AND status = 'approved';

    NEW.reply_count := actual_count;
    NEW.last_reply_at := actual_last_reply_at;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE OR REPLACE FUNCTION forum_enforce_category_public_reply_count()
RETURNS trigger AS $$
DECLARE
    actual_count INTEGER;
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
    actual_count INTEGER;
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

DROP TRIGGER IF EXISTS forum_90_categories_public_reply_count
    ON forum_categories;
CREATE TRIGGER forum_90_categories_public_reply_count
BEFORE INSERT OR UPDATE OF reply_count
ON forum_categories
FOR EACH ROW
EXECUTE FUNCTION forum_enforce_category_public_reply_count();

DROP TRIGGER IF EXISTS forum_90_user_stats_public_reply_count
    ON forum_user_stats;
CREATE TRIGGER forum_90_user_stats_public_reply_count
BEFORE INSERT OR UPDATE OF reply_count, tenant_id, user_id
ON forum_user_stats
FOR EACH ROW
EXECUTE FUNCTION forum_enforce_user_public_reply_count();

DROP TABLE IF EXISTS forum_hard_delete_context;
DROP TABLE IF EXISTS forum_reply_revisions;
DROP TABLE IF EXISTS forum_topic_revisions;

DROP INDEX IF EXISTS idx_forum_replies_tenant_topic_deleted;
DROP INDEX IF EXISTS idx_forum_topics_tenant_deleted;

ALTER TABLE forum_replies DROP COLUMN IF EXISTS deleted_at;
ALTER TABLE forum_topics DROP COLUMN IF EXISTS deleted_at;
"#,
        )
        .await?;
    Ok(())
}
