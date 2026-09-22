use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::ConnectionTrait;

pub(super) async fn apply_revisions(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    let connection = manager.get_connection();
    for statement in [
        r#"CREATE TRIGGER forum_topic_translation_revision_update
        BEFORE UPDATE OF title, slug, body
        ON forum_topic_translations
        FOR EACH ROW
        WHEN OLD.title IS NOT NEW.title
          OR OLD.slug IS NOT NEW.slug
          OR OLD.body IS NOT NEW.body
        BEGIN
            SELECT CASE
                WHEN EXISTS (
                    SELECT 1
                    FROM forum_topics topic
                    WHERE topic.tenant_id = OLD.tenant_id
                      AND topic.id = OLD.topic_id
                      AND topic.deleted_at IS NOT NULL
                )
                THEN RAISE(ABORT, 'deleted forum topic content is immutable')
            END;

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
                OLD.topic_id,
                OLD.locale,
                OLD.title,
                OLD.slug,
                OLD.body,
                topic.metadata,
                'edit'
            FROM forum_topics topic
            WHERE topic.tenant_id = OLD.tenant_id
              AND topic.id = OLD.topic_id;
        END"#,
        r#"CREATE TRIGGER forum_topic_metadata_revision_update
        BEFORE UPDATE OF metadata
        ON forum_topics
        FOR EACH ROW
        WHEN OLD.metadata IS NOT NEW.metadata
        BEGIN
            SELECT CASE
                WHEN OLD.deleted_at IS NOT NULL
                THEN RAISE(ABORT, 'deleted forum topic is immutable')
            END;

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
                'edit'
            FROM forum_topic_translations translation
            WHERE translation.tenant_id = OLD.tenant_id
              AND translation.topic_id = OLD.id;
        END"#,
        r#"CREATE TRIGGER forum_reply_body_revision_update
        BEFORE UPDATE OF body
        ON forum_reply_bodies
        FOR EACH ROW
        WHEN OLD.body IS NOT NEW.body
        BEGIN
            SELECT CASE
                WHEN EXISTS (
                    SELECT 1
                    FROM forum_replies reply
                    WHERE reply.tenant_id = OLD.tenant_id
                      AND reply.id = OLD.reply_id
                      AND reply.deleted_at IS NOT NULL
                )
                THEN RAISE(ABORT, 'deleted forum reply content is immutable')
            END;

            INSERT INTO forum_reply_revisions (
                tenant_id,
                reply_id,
                locale,
                body,
                revision_reason
            )
            VALUES (
                OLD.tenant_id,
                OLD.reply_id,
                OLD.locale,
                OLD.body,
                'edit'
            );
        END"#,
        r#"CREATE TRIGGER forum_topic_delete_revision_update
        BEFORE UPDATE OF deleted_at ON forum_topics
        FOR EACH ROW
        WHEN OLD.deleted_at IS NULL AND NEW.deleted_at IS NOT NULL
        BEGIN
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
        END"#,
        r#"CREATE TRIGGER forum_reply_delete_revision_update
        BEFORE UPDATE OF deleted_at ON forum_replies
        FOR EACH ROW
        WHEN OLD.deleted_at IS NULL AND NEW.deleted_at IS NOT NULL
        BEGIN
            INSERT INTO forum_reply_revisions (
                tenant_id,
                reply_id,
                locale,
                body,
                revision_reason
            )
            SELECT
                OLD.tenant_id,
                OLD.id,
                body.locale,
                body.body,
                'delete'
            FROM forum_reply_bodies body
            WHERE body.tenant_id = OLD.tenant_id
              AND body.reply_id = OLD.id;
        END"#,
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
    ] {
        connection.execute_unprepared(statement).await?;
    }
    Ok(())
}
