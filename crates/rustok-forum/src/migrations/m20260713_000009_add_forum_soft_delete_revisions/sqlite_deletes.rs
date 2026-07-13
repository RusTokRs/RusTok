use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::ConnectionTrait;

pub(super) async fn apply_deletes(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    let connection = manager.get_connection();
    for statement in [
        r#"CREATE TRIGGER forum_replies_soft_delete
        BEFORE DELETE ON forum_replies
        FOR EACH ROW
        WHEN NOT EXISTS (
            SELECT 1
            FROM forum_hard_delete_context context
            WHERE context.topic_id = OLD.topic_id
        )
        BEGIN
            SELECT CASE
                WHEN OLD.deleted_at IS NOT NULL
                THEN RAISE(ABORT, 'forum reply is already deleted')
            END;

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

            UPDATE forum_categories
            SET reply_count = reply_count
            WHERE tenant_id = OLD.tenant_id
              AND id = (
                  SELECT topic.category_id
                  FROM forum_topics topic
                  WHERE topic.tenant_id = OLD.tenant_id
                    AND topic.id = OLD.topic_id
              );

            UPDATE forum_user_stats
            SET topic_count = topic_count,
                reply_count = reply_count,
                solution_count = solution_count
            WHERE tenant_id = OLD.tenant_id
              AND user_id = OLD.author_id;

            SELECT RAISE(IGNORE);
        END"#,
        r#"CREATE TRIGGER forum_topics_soft_delete
        BEFORE DELETE ON forum_topics
        FOR EACH ROW
        WHEN NOT EXISTS (
            SELECT 1
            FROM forum_hard_delete_context context
            WHERE context.topic_id = OLD.id
        )
        BEGIN
            SELECT CASE
                WHEN OLD.deleted_at IS NOT NULL
                THEN RAISE(ABORT, 'forum topic is already deleted')
            END;

            UPDATE forum_topic_translations
            SET title = '[deleted]',
                slug = NULL,
                body = '[deleted]',
                body_format = 'markdown',
                updated_at = CURRENT_TIMESTAMP
            WHERE tenant_id = OLD.tenant_id
              AND topic_id = OLD.id;

            UPDATE forum_reply_bodies
            SET body = '[deleted]',
                body_format = 'markdown',
                updated_at = CURRENT_TIMESTAMP
            WHERE tenant_id = OLD.tenant_id
              AND reply_id IN (
                  SELECT reply.id
                  FROM forum_replies reply
                  WHERE reply.tenant_id = OLD.tenant_id
                    AND reply.topic_id = OLD.id
                    AND reply.deleted_at IS NULL
              );

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
                is_locked = 1,
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

            SELECT RAISE(IGNORE);
        END"#,
    ] {
        connection.execute_unprepared(statement).await?;
    }
    Ok(())
}
