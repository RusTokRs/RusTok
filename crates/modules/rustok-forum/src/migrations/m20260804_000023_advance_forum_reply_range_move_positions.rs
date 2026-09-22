use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::DatabaseBackend;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        match manager.get_database_backend() {
            DatabaseBackend::Postgres => execute(manager, POSTGRES_UP).await,
            DatabaseBackend::Sqlite => execute(manager, SQLITE_UP).await,
            backend => Err(DbErr::Custom(format!(
                "rustok-forum reply range move position watermark migration does not support {backend:?}"
            ))),
        }
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        match manager.get_database_backend() {
            DatabaseBackend::Postgres => execute(manager, POSTGRES_DOWN).await,
            DatabaseBackend::Sqlite => execute(manager, SQLITE_DOWN).await,
            backend => Err(DbErr::Custom(format!(
                "rustok-forum reply range move position watermark rollback does not support {backend:?}"
            ))),
        }
    }
}

async fn execute(manager: &SchemaManager<'_>, sql: &str) -> Result<(), DbErr> {
    manager
        .get_connection()
        .execute_unprepared(sql)
        .await
        .map(|_| ())
}

const POSTGRES_UP: &str = r#"
CREATE OR REPLACE FUNCTION forum_advance_moved_reply_position_watermark()
RETURNS trigger AS $$
BEGIN
    UPDATE forum_topics
       SET next_reply_position = GREATEST(next_reply_position, NEW.position + 1)
     WHERE tenant_id = NEW.tenant_id
       AND id = NEW.topic_id;

    IF NOT FOUND THEN
        RAISE EXCEPTION 'forum moved reply target topic does not exist in tenant';
    END IF;

    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS forum_reply_range_move_advance_target_position
    ON forum_replies;
CREATE TRIGGER forum_reply_range_move_advance_target_position
AFTER UPDATE OF topic_id, position
ON forum_replies
FOR EACH ROW
WHEN (
    OLD.topic_id IS DISTINCT FROM NEW.topic_id
    OR OLD.position IS DISTINCT FROM NEW.position
)
EXECUTE FUNCTION forum_advance_moved_reply_position_watermark();
"#;

const POSTGRES_DOWN: &str = r#"
DROP TRIGGER IF EXISTS forum_reply_range_move_advance_target_position
    ON forum_replies;
DROP FUNCTION IF EXISTS forum_advance_moved_reply_position_watermark();
"#;

const SQLITE_UP: &str = r#"
DROP TRIGGER IF EXISTS forum_reply_range_move_advance_target_position;
CREATE TRIGGER forum_reply_range_move_advance_target_position
AFTER UPDATE OF topic_id, position
ON forum_replies
FOR EACH ROW
WHEN OLD.topic_id <> NEW.topic_id OR OLD.position <> NEW.position
BEGIN
    UPDATE forum_topics
       SET next_reply_position = MAX(next_reply_position, NEW.position + 1)
     WHERE tenant_id = NEW.tenant_id
       AND id = NEW.topic_id;

    SELECT CASE
        WHEN changes() <> 1 THEN
            RAISE(ABORT, 'forum moved reply target topic does not exist in tenant')
    END;
END;
"#;

const SQLITE_DOWN: &str = r#"
DROP TRIGGER IF EXISTS forum_reply_range_move_advance_target_position;
"#;
