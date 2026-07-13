use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::ConnectionTrait;

pub(super) async fn down_sqlite(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    let connection = manager.get_connection();
    for statement in [
        "DROP TRIGGER IF EXISTS forum_user_stats_public_reply_count_update",
        "DROP TRIGGER IF EXISTS forum_user_stats_public_reply_count_insert",
        "DROP TRIGGER IF EXISTS forum_categories_public_reply_count_update",
        "DROP TRIGGER IF EXISTS forum_topics_public_reply_count_update",
        "DROP TRIGGER IF EXISTS forum_replies_soft_delete",
        "DROP TRIGGER IF EXISTS forum_topics_soft_delete",
        "DROP TRIGGER IF EXISTS forum_categories_hard_delete_context_after",
        "DROP TRIGGER IF EXISTS forum_categories_hard_delete_context_before",
        "DROP TRIGGER IF EXISTS forum_replies_deleted_update_guard",
        "DROP TRIGGER IF EXISTS forum_topics_deleted_update_guard",
        "DROP TRIGGER IF EXISTS forum_reply_body_revision_update",
        "DROP TRIGGER IF EXISTS forum_topic_metadata_revision_update",
        "DROP TRIGGER IF EXISTS forum_topic_translation_revision_update",
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
        "DROP TABLE IF EXISTS forum_hard_delete_context",
        "DROP TABLE IF EXISTS forum_reply_revisions",
        "DROP TABLE IF EXISTS forum_topic_revisions",
        "DROP INDEX IF EXISTS idx_forum_replies_tenant_topic_deleted",
        "DROP INDEX IF EXISTS idx_forum_topics_tenant_deleted",
        "ALTER TABLE forum_replies DROP COLUMN deleted_at",
        "ALTER TABLE forum_topics DROP COLUMN deleted_at",
    ] {
        connection.execute_unprepared(statement).await?;
    }
    Ok(())
}
