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
        install_category_guard(manager, "AND topic.status <> 'archived'").await?;
        let db = manager.get_connection();
        // Earlier trusted hide/reject effects could leave an accepted solution
        // attached to a non-public reply. Remove only those invalid current
        // relations before rebuilding author statistics. Approved solutions on
        // live archived threads remain valid historical relations.
        db.execute_unprepared(
            "DELETE FROM forum_solutions
             WHERE EXISTS (
                 SELECT 1 FROM forum_replies reply
                 WHERE reply.tenant_id = forum_solutions.tenant_id
                   AND reply.topic_id = forum_solutions.topic_id
                   AND reply.id = forum_solutions.reply_id
                   AND (reply.status <> 'approved' OR reply.deleted_at IS NOT NULL)
             )",
        )
        .await?;
        // Re-enter the current guards after all canonical reply statuses have
        // been persisted. This repairs installations affected by edit re-pending;
        // it never changes content, status, ownership or historical revisions.
        // Tombstones are immutable, even for no-op aggregate writes. Their own
        // reply projection was cleared by deletion; only live topics need repair.
        db.execute_unprepared(
            "UPDATE forum_topics SET reply_count = reply_count,
                 last_reply_at = (
                     SELECT MAX(reply.created_at) FROM forum_replies reply
                     WHERE reply.tenant_id = forum_topics.tenant_id
                       AND reply.topic_id = forum_topics.id
                       AND reply.status = 'approved' AND reply.deleted_at IS NULL
                 )
             WHERE forum_topics.deleted_at IS NULL",
        )
        .await?;
        db.execute_unprepared("UPDATE forum_categories SET topic_count = topic_count")
            .await?;
        db.execute_unprepared("UPDATE forum_user_stats SET topic_count = topic_count")
            .await?;
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Restore the preceding SQLite schema, not its corrupted data. Derived
        // counts repaired by up are deliberately not reverted to stale values;
        // invalid accepted-solution relations are deliberately not resurrected.
        install_category_guard(manager, "").await
    }
}

async fn install_category_guard(
    manager: &SchemaManager<'_>,
    archive_filter: &str,
) -> Result<(), DbErr> {
    match manager.get_database_backend() {
        // Migration 45 already installed the correct PostgreSQL category guard.
        DatabaseBackend::Postgres => Ok(()),
        DatabaseBackend::Sqlite => {
            let db = manager.get_connection();
            let topics = format!(
                "(SELECT COUNT(*) FROM forum_topics topic
                  WHERE topic.tenant_id = NEW.tenant_id AND topic.category_id = NEW.id
                    AND topic.deleted_at IS NULL {archive_filter})"
            );
            let replies = format!(
                "(SELECT COUNT(*) FROM forum_replies reply
                  JOIN forum_topics topic ON topic.tenant_id = reply.tenant_id AND topic.id = reply.topic_id
                  WHERE topic.tenant_id = NEW.tenant_id AND topic.category_id = NEW.id
                    AND reply.status = 'approved' AND reply.deleted_at IS NULL
                    AND topic.deleted_at IS NULL {archive_filter})"
            );
            db.execute_unprepared("DROP TRIGGER forum_categories_public_reply_count_update")
                .await?;
            db.execute_unprepared(&format!(
                "CREATE TRIGGER forum_categories_public_reply_count_update
                 AFTER UPDATE OF topic_count, reply_count ON forum_categories
                 FOR EACH ROW
                 WHEN NEW.topic_count <> {topics} OR NEW.reply_count <> {replies}
                 BEGIN
                     UPDATE forum_categories SET topic_count = {topics}, reply_count = {replies}
                     WHERE tenant_id = NEW.tenant_id AND id = NEW.id;
                 END"
            ))
            .await?;
            Ok(())
        }
        backend => Err(DbErr::Custom(format!(
            "Forum publication counter reconciliation does not support {backend:?}"
        ))),
    }
}
