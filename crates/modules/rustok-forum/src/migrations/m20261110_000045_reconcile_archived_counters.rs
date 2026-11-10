use sea_orm::DatabaseBackend;
use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    fn use_transaction(&self) -> Option<bool> {
        Some(true)
    }

    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        replace_counter_guards(manager, "AND topic.status <> 'archived'").await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        replace_counter_guards(manager, "").await
    }
}

async fn replace_counter_guards(
    manager: &SchemaManager<'_>,
    archive_filter: &str,
) -> Result<(), DbErr> {
    let db = manager.get_connection();
    let topics = format!(
        "(SELECT COUNT(*) FROM forum_topics topic
          WHERE topic.tenant_id = NEW.tenant_id AND topic.author_id = NEW.user_id
            AND topic.deleted_at IS NULL {archive_filter})"
    );
    let replies = format!(
        "(SELECT COUNT(*) FROM forum_replies reply
          JOIN forum_topics topic ON topic.tenant_id = reply.tenant_id AND topic.id = reply.topic_id
          WHERE reply.tenant_id = NEW.tenant_id AND reply.author_id = NEW.user_id
            AND reply.status = 'approved' AND reply.deleted_at IS NULL
            AND topic.deleted_at IS NULL {archive_filter})"
    );
    let solutions = format!(
        "(SELECT COUNT(*) FROM forum_solutions solution
          JOIN forum_replies reply ON reply.tenant_id = solution.tenant_id AND reply.id = solution.reply_id
          JOIN forum_topics topic ON topic.tenant_id = solution.tenant_id AND topic.id = solution.topic_id
          WHERE solution.tenant_id = NEW.tenant_id AND reply.author_id = NEW.user_id
            AND reply.deleted_at IS NULL AND topic.deleted_at IS NULL {archive_filter})"
    );
    match manager.get_database_backend() {
        DatabaseBackend::Postgres => {
            db.execute_unprepared(&format!(
                r#"
ALTER TABLE forum_categories ENABLE TRIGGER forum_90_categories_public_reply_count;
ALTER TABLE forum_user_stats ENABLE TRIGGER forum_90_user_stats_public_reply_count;

CREATE OR REPLACE FUNCTION forum_enforce_category_public_reply_count()
RETURNS trigger AS $$
BEGIN
    NEW.topic_count := (SELECT COUNT(*) FROM forum_topics topic
        WHERE topic.tenant_id = NEW.tenant_id AND topic.category_id = NEW.id
          AND topic.deleted_at IS NULL {archive_filter});
    NEW.reply_count := (SELECT COUNT(*) FROM forum_replies reply
        JOIN forum_topics topic ON topic.tenant_id = reply.tenant_id AND topic.id = reply.topic_id
        WHERE topic.tenant_id = NEW.tenant_id AND topic.category_id = NEW.id
          AND reply.status = 'approved' AND reply.deleted_at IS NULL
          AND topic.deleted_at IS NULL {archive_filter});
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE OR REPLACE FUNCTION forum_enforce_user_public_reply_count()
RETURNS trigger AS $$
BEGIN
    NEW.topic_count := {topics};
    NEW.reply_count := {replies};
    NEW.solution_count := {solutions};
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

UPDATE forum_categories category SET topic_count = category.topic_count
WHERE EXISTS (
    SELECT 1 FROM forum_topics topic
    WHERE topic.tenant_id = category.tenant_id AND topic.category_id = category.id
      AND topic.status = 'archived' AND topic.deleted_at IS NULL
);
"#
            ))
            .await?;
        }
        DatabaseBackend::Sqlite => {
            for (suffix, action) in [
                ("insert", "INSERT"),
                (
                    "update",
                    "UPDATE OF topic_count, reply_count, solution_count",
                ),
            ] {
                let name = format!("forum_user_stats_public_reply_count_{suffix}");
                db.execute_unprepared(&format!("DROP TRIGGER {name}"))
                    .await?;
                db.execute_unprepared(&format!(
                    "CREATE TRIGGER {name}
                     AFTER {action} ON forum_user_stats
                     FOR EACH ROW
                     WHEN NEW.topic_count <> {topics} OR NEW.reply_count <> {replies}
                          OR NEW.solution_count <> {solutions}
                     BEGIN
                         UPDATE forum_user_stats
                         SET topic_count = {topics}, reply_count = {replies},
                             solution_count = {solutions}, updated_at = CURRENT_TIMESTAMP
                         WHERE tenant_id = NEW.tenant_id AND user_id = NEW.user_id;
                     END"
                ))
                .await?;
            }
        }
        backend => {
            return Err(DbErr::Custom(format!(
                "Forum archived counter reconciliation does not support {backend:?}"
            )));
        }
    }
    db.execute_unprepared(
        "UPDATE forum_user_stats AS stats SET topic_count = stats.topic_count
         WHERE EXISTS (
             SELECT 1 FROM forum_topics topic
             WHERE topic.tenant_id = stats.tenant_id AND topic.status = 'archived'
               AND topic.deleted_at IS NULL
               AND (topic.author_id = stats.user_id OR EXISTS (
                   SELECT 1 FROM forum_replies reply
                   WHERE reply.tenant_id = topic.tenant_id AND reply.topic_id = topic.id
                     AND reply.author_id = stats.user_id
               ))
         )",
    )
    .await?;
    Ok(())
}
