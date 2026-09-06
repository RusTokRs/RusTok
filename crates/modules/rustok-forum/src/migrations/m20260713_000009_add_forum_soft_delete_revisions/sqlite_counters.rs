use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::ConnectionTrait;

pub(super) async fn apply_counters(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    let connection = manager.get_connection();
    for statement in [
        "DROP TRIGGER IF EXISTS forum_topics_public_reply_count_update",
        "DROP TRIGGER IF EXISTS forum_categories_public_reply_count_update",
        "DROP TRIGGER IF EXISTS forum_user_stats_public_reply_count_insert",
        "DROP TRIGGER IF EXISTS forum_user_stats_public_reply_count_update",
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
                  AND reply.deleted_at IS NULL
            )
            OR COALESCE(NEW.last_reply_at, '') <> COALESCE((
                SELECT MAX(reply.created_at)
                FROM forum_replies reply
                WHERE reply.tenant_id = NEW.tenant_id
                  AND reply.topic_id = NEW.id
                  AND reply.status = 'approved'
                  AND reply.deleted_at IS NULL
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
                      AND reply.deleted_at IS NULL
                ),
                last_reply_at = (
                    SELECT MAX(reply.created_at)
                    FROM forum_replies reply
                    WHERE reply.tenant_id = NEW.tenant_id
                      AND reply.topic_id = NEW.id
                      AND reply.status = 'approved'
                      AND reply.deleted_at IS NULL
                )
            WHERE tenant_id = NEW.tenant_id
              AND id = NEW.id;
        END"#,
        r#"CREATE TRIGGER forum_categories_public_reply_count_update
        AFTER UPDATE OF topic_count, reply_count ON forum_categories
        FOR EACH ROW
        WHEN NEW.topic_count <> (
            SELECT COUNT(*)
            FROM forum_topics topic
            WHERE topic.tenant_id = NEW.tenant_id
              AND topic.category_id = NEW.id
              AND topic.deleted_at IS NULL
        )
        OR NEW.reply_count <> (
            SELECT COUNT(*)
            FROM forum_replies reply
            JOIN forum_topics topic
              ON topic.tenant_id = reply.tenant_id
             AND topic.id = reply.topic_id
            WHERE topic.tenant_id = NEW.tenant_id
              AND topic.category_id = NEW.id
              AND topic.deleted_at IS NULL
              AND reply.status = 'approved'
              AND reply.deleted_at IS NULL
        )
        BEGIN
            UPDATE forum_categories
            SET topic_count = (
                    SELECT COUNT(*)
                    FROM forum_topics topic
                    WHERE topic.tenant_id = NEW.tenant_id
                      AND topic.category_id = NEW.id
                      AND topic.deleted_at IS NULL
                ),
                reply_count = (
                    SELECT COUNT(*)
                    FROM forum_replies reply
                    JOIN forum_topics topic
                      ON topic.tenant_id = reply.tenant_id
                     AND topic.id = reply.topic_id
                    WHERE topic.tenant_id = NEW.tenant_id
                      AND topic.category_id = NEW.id
                      AND topic.deleted_at IS NULL
                      AND reply.status = 'approved'
                      AND reply.deleted_at IS NULL
                )
            WHERE tenant_id = NEW.tenant_id
              AND id = NEW.id;
        END"#,
        r#"CREATE TRIGGER forum_user_stats_public_reply_count_insert
        AFTER INSERT ON forum_user_stats
        FOR EACH ROW
        WHEN NEW.topic_count <> (
            SELECT COUNT(*)
            FROM forum_topics topic
            WHERE topic.tenant_id = NEW.tenant_id
              AND topic.author_id = NEW.user_id
              AND topic.deleted_at IS NULL
        )
        OR NEW.reply_count <> (
            SELECT COUNT(*)
            FROM forum_replies reply
            JOIN forum_topics topic
              ON topic.tenant_id = reply.tenant_id
             AND topic.id = reply.topic_id
            WHERE reply.tenant_id = NEW.tenant_id
              AND reply.author_id = NEW.user_id
              AND reply.status = 'approved'
              AND reply.deleted_at IS NULL
              AND topic.deleted_at IS NULL
        )
        OR NEW.solution_count <> (
            SELECT COUNT(*)
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
              AND topic.deleted_at IS NULL
        )
        BEGIN
            UPDATE forum_user_stats
            SET topic_count = (
                    SELECT COUNT(*)
                    FROM forum_topics topic
                    WHERE topic.tenant_id = NEW.tenant_id
                      AND topic.author_id = NEW.user_id
                      AND topic.deleted_at IS NULL
                ),
                reply_count = (
                    SELECT COUNT(*)
                    FROM forum_replies reply
                    JOIN forum_topics topic
                      ON topic.tenant_id = reply.tenant_id
                     AND topic.id = reply.topic_id
                    WHERE reply.tenant_id = NEW.tenant_id
                      AND reply.author_id = NEW.user_id
                      AND reply.status = 'approved'
                      AND reply.deleted_at IS NULL
                      AND topic.deleted_at IS NULL
                ),
                solution_count = (
                    SELECT COUNT(*)
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
                      AND topic.deleted_at IS NULL
                ),
                updated_at = CURRENT_TIMESTAMP
            WHERE tenant_id = NEW.tenant_id
              AND user_id = NEW.user_id;
        END"#,
        r#"CREATE TRIGGER forum_user_stats_public_reply_count_update
        AFTER UPDATE OF topic_count, reply_count, solution_count ON forum_user_stats
        FOR EACH ROW
        WHEN NEW.topic_count <> (
            SELECT COUNT(*)
            FROM forum_topics topic
            WHERE topic.tenant_id = NEW.tenant_id
              AND topic.author_id = NEW.user_id
              AND topic.deleted_at IS NULL
        )
        OR NEW.reply_count <> (
            SELECT COUNT(*)
            FROM forum_replies reply
            JOIN forum_topics topic
              ON topic.tenant_id = reply.tenant_id
             AND topic.id = reply.topic_id
            WHERE reply.tenant_id = NEW.tenant_id
              AND reply.author_id = NEW.user_id
              AND reply.status = 'approved'
              AND reply.deleted_at IS NULL
              AND topic.deleted_at IS NULL
        )
        OR NEW.solution_count <> (
            SELECT COUNT(*)
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
              AND topic.deleted_at IS NULL
        )
        BEGIN
            UPDATE forum_user_stats
            SET topic_count = (
                    SELECT COUNT(*)
                    FROM forum_topics topic
                    WHERE topic.tenant_id = NEW.tenant_id
                      AND topic.author_id = NEW.user_id
                      AND topic.deleted_at IS NULL
                ),
                reply_count = (
                    SELECT COUNT(*)
                    FROM forum_replies reply
                    JOIN forum_topics topic
                      ON topic.tenant_id = reply.tenant_id
                     AND topic.id = reply.topic_id
                    WHERE reply.tenant_id = NEW.tenant_id
                      AND reply.author_id = NEW.user_id
                      AND reply.status = 'approved'
                      AND reply.deleted_at IS NULL
                      AND topic.deleted_at IS NULL
                ),
                solution_count = (
                    SELECT COUNT(*)
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
                      AND topic.deleted_at IS NULL
                ),
                updated_at = CURRENT_TIMESTAMP
            WHERE tenant_id = NEW.tenant_id
              AND user_id = NEW.user_id;
        END"#,
    ] {
        connection.execute_unprepared(statement).await?;
    }
    Ok(())
}
