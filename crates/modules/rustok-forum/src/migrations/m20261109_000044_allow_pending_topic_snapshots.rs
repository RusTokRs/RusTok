use sea_orm::{DatabaseBackend, Statement};
use sea_orm_migration::prelude::*;

const PREVIOUS_STATUSES: &str = "'open', 'closed', 'archived'";
const PENDING_STATUSES: &str = "'open', 'closed', 'archived', 'pending'";

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    fn use_transaction(&self) -> Option<bool> {
        Some(true)
    }

    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        change_status_constraint(manager, PENDING_STATUSES).await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();
        let backend = manager.get_database_backend();
        if backend == DatabaseBackend::Postgres {
            db.execute_unprepared(
                "LOCK TABLE forum_topic_delete_snapshots IN ACCESS EXCLUSIVE MODE",
            )
            .await?;
        }
        let pending = Statement::from_string(
            backend,
            "SELECT 1 FROM forum_topic_delete_snapshots WHERE previous_status = 'pending' LIMIT 1",
        );
        if db.query_one_raw(pending).await?.is_some() {
            return Err(DbErr::Custom(
                "pending Forum topic snapshots must be retained for exact restoration".into(),
            ));
        }
        change_status_constraint(manager, PREVIOUS_STATUSES).await
    }
}

async fn change_status_constraint(
    manager: &SchemaManager<'_>,
    statuses: &str,
) -> Result<(), DbErr> {
    let db = manager.get_connection();
    match manager.get_database_backend() {
        DatabaseBackend::Postgres => {
            db.execute_unprepared(&format!(
                "ALTER TABLE forum_topic_delete_snapshots
                 DROP CONSTRAINT forum_topic_delete_snapshots_previous_status_check,
                 ADD CONSTRAINT forum_topic_delete_snapshots_previous_status_check
                 CHECK (previous_status IN ({statuses}))"
            ))
            .await?;
        }
        DatabaseBackend::Sqlite => {
            db.execute_unprepared(
                "CREATE TABLE forum_topic_delete_snapshots_backup AS
                 SELECT tenant_id, topic_id, previous_status, previous_is_locked, solution_reply_id,
                    solution_author_id, solution_marked_by_user_id, solution_marked_at, created_at,
                    reason
                 FROM forum_topic_delete_snapshots",
            )
            .await?;
            db.execute_unprepared("DROP TRIGGER forum_topics_deleted_update_guard")
                .await?;
            db.execute_unprepared("DROP TABLE forum_topic_delete_snapshots")
                .await?;
            db.execute_unprepared(&format!(
                "CREATE TABLE forum_topic_delete_snapshots (
                    tenant_id TEXT NOT NULL,
                    topic_id TEXT NOT NULL,
                    previous_status TEXT NOT NULL CHECK (previous_status IN ({statuses})),
                    previous_is_locked INTEGER NOT NULL,
                    solution_reply_id TEXT,
                    solution_author_id TEXT,
                    solution_marked_by_user_id TEXT,
                    solution_marked_at TEXT,
                    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
                    reason TEXT CHECK (reason IN ('user_request', 'moderation', 'system')),
                    PRIMARY KEY (tenant_id, topic_id),
                    FOREIGN KEY (topic_id) REFERENCES forum_topics (id) ON DELETE CASCADE
                )"
            ))
            .await?;
            for sql in [
                "INSERT INTO forum_topic_delete_snapshots (
                    tenant_id, topic_id, previous_status, previous_is_locked, solution_reply_id,
                    solution_author_id, solution_marked_by_user_id, solution_marked_at, created_at,
                    reason
                 )
                 SELECT tenant_id, topic_id, previous_status, previous_is_locked, solution_reply_id,
                    solution_author_id, solution_marked_by_user_id, solution_marked_at, created_at,
                    reason
                 FROM forum_topic_delete_snapshots_backup",
                "DROP TABLE forum_topic_delete_snapshots_backup",
                "CREATE INDEX idx_forum_topic_delete_snapshots_topic
                 ON forum_topic_delete_snapshots (tenant_id, topic_id)",
                SQLITE_DELETED_TOPIC_UPDATE_GUARD,
            ] {
                db.execute_unprepared(sql).await?;
            }
        }
        backend => {
            return Err(DbErr::Custom(format!(
                "Forum topic snapshot status constraint does not support {backend:?}"
            )));
        }
    }
    Ok(())
}

// SQLite requires recreating the restore guard from migration 000034 around the table swap.
const SQLITE_DELETED_TOPIC_UPDATE_GUARD: &str = r#"
CREATE TRIGGER forum_topics_deleted_update_guard
BEFORE UPDATE ON forum_topics
FOR EACH ROW
WHEN OLD.deleted_at IS NOT NULL
BEGIN
    SELECT CASE
        WHEN NEW.deleted_at IS NOT NULL
          OR NEW.id IS NOT OLD.id
          OR NEW.tenant_id IS NOT OLD.tenant_id
          OR NEW.category_id IS NOT OLD.category_id
          OR NEW.author_id IS NOT OLD.author_id
          OR NEW.metadata IS NOT OLD.metadata
          OR NEW.is_pinned IS NOT OLD.is_pinned
          OR NEW.created_at IS NOT OLD.created_at
          OR NEW.status IS NOT (SELECT previous_status FROM forum_topic_delete_snapshots WHERE tenant_id = OLD.tenant_id AND topic_id = OLD.id)
          OR NEW.is_locked IS NOT (SELECT previous_is_locked FROM forum_topic_delete_snapshots WHERE tenant_id = OLD.tenant_id AND topic_id = OLD.id)
          OR NOT EXISTS (SELECT 1 FROM forum_topic_delete_snapshots WHERE tenant_id = OLD.tenant_id AND topic_id = OLD.id)
        THEN RAISE(ABORT, 'deleted forum topic is immutable')
    END;
END"#;
