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
        install_target_guard(manager, true).await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        install_target_guard(manager, false).await
    }
}

async fn install_target_guard(
    manager: &SchemaManager<'_>,
    allow_snapshot_replay: bool,
) -> Result<(), DbErr> {
    let backend = manager.get_database_backend();
    let replay = if allow_snapshot_replay {
        match backend {
            // The PostgreSQL validator handles both INSERT and target UPDATE.
            DatabaseBackend::Postgres => format!("OR (TG_OP = 'INSERT' AND {SNAPSHOT_REPLAY})"),
            DatabaseBackend::Sqlite => format!("OR {SNAPSHOT_REPLAY}"),
            _ => String::new(),
        }
    } else {
        String::new()
    };
    let target = TARGET_QUERY.replace("{archived_replay}", &replay);
    let sql = match backend {
        DatabaseBackend::Postgres => POSTGRES_VALIDATOR.replace("{target_query}", &target),
        // Replace only INSERT. The existing SQLite target UPDATE guard remains
        // strict, as do the solution scope/tenant locks on both backends.
        DatabaseBackend::Sqlite => SQLITE_VALIDATOR.replace("{target_query}", &target),
        backend => {
            return Err(DbErr::Custom(format!(
                "Forum topic solution snapshot replay does not support {backend:?}"
            )));
        }
    };
    manager.get_connection().execute_unprepared(&sql).await?;
    Ok(())
}

// A topic restore may bring an already-archived thread back with its original
// solution. Only its exact thread snapshots admit this historical INSERT;
// ordinary archived-target commands and standalone reply restores do not.
const SNAPSHOT_REPLAY: &str = r#"EXISTS (
    SELECT 1
    FROM forum_topic_delete_snapshots snapshot
    JOIN forum_topic_reply_delete_snapshots child
      ON child.tenant_id = snapshot.tenant_id
     AND child.topic_id = snapshot.topic_id
     AND child.reply_id = snapshot.solution_reply_id
    WHERE snapshot.tenant_id = topic.tenant_id
      AND snapshot.topic_id = topic.id
      AND snapshot.previous_status = 'archived'
      AND snapshot.solution_reply_id = reply.id
      AND child.previous_status = 'approved'
)"#;

const TARGET_QUERY: &str = r#"SELECT 1
FROM forum_topics topic
JOIN forum_replies reply
  ON reply.tenant_id = topic.tenant_id
 AND reply.topic_id = topic.id
 AND reply.id = NEW.reply_id
WHERE topic.tenant_id = NEW.tenant_id
  AND topic.id = NEW.topic_id
  AND topic.deleted_at IS NULL
  AND (topic.status <> 'archived' {archived_replay})
  AND reply.deleted_at IS NULL
  AND reply.status = 'approved'"#;

const POSTGRES_VALIDATOR: &str = r#"
CREATE OR REPLACE FUNCTION forum_validate_topic_solution_target()
RETURNS trigger AS $$
BEGIN
    IF NOT EXISTS ({target_query}) THEN
        RAISE EXCEPTION 'forum solution requires an active topic and approved reply';
    END IF;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;
"#;

const SQLITE_VALIDATOR: &str = r#"
DROP TRIGGER forum_10_topic_solution_target_insert;
CREATE TRIGGER forum_10_topic_solution_target_insert
BEFORE INSERT ON forum_solutions
FOR EACH ROW
WHEN NOT EXISTS ({target_query})
BEGIN
    SELECT RAISE(ABORT, 'forum solution requires an active topic and approved reply');
END;
"#;
