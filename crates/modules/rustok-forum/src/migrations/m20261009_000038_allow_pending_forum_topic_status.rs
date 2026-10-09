//! Allows the `pending` topic status used by topic pre-moderation.
//!
//! The status lifecycle migration pinned `forum_topics.status` to `open`, `closed`, and
//! `archived`. This migration adds `pending` to the Postgres check constraint and to the SQLite
//! insert and update triggers. Down migration refuses to run while any topic is pending, because
//! the previous schema cannot represent that state.

use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::{ConnectionTrait, DatabaseBackend, Statement};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        match manager.get_database_backend() {
            DatabaseBackend::Postgres => up_postgres(manager).await,
            DatabaseBackend::Sqlite => replace_sqlite_triggers(manager, true).await,
            backend => Err(DbErr::Custom(format!(
                "rustok-forum pending topic status migration does not support {backend:?}"
            ))),
        }
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        ensure_no_pending_topics(manager).await?;
        match manager.get_database_backend() {
            DatabaseBackend::Postgres => down_postgres(manager).await,
            DatabaseBackend::Sqlite => replace_sqlite_triggers(manager, false).await,
            backend => Err(DbErr::Custom(format!(
                "rustok-forum pending topic status migration does not support {backend:?}"
            ))),
        }
    }
}

async fn up_postgres(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .get_connection()
        .execute_unprepared(
            r#"
ALTER TABLE forum_topics
    DROP CONSTRAINT IF EXISTS chk_forum_topics_status;
ALTER TABLE forum_topics
    ADD CONSTRAINT chk_forum_topics_status
    CHECK (status IN ('open', 'closed', 'archived', 'pending'));
"#,
        )
        .await?;
    Ok(())
}

async fn down_postgres(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .get_connection()
        .execute_unprepared(
            r#"
ALTER TABLE forum_topics
    DROP CONSTRAINT IF EXISTS chk_forum_topics_status;
ALTER TABLE forum_topics
    ADD CONSTRAINT chk_forum_topics_status
    CHECK (status IN ('open', 'closed', 'archived'));
"#,
        )
        .await?;
    Ok(())
}

async fn replace_sqlite_triggers(
    manager: &SchemaManager<'_>,
    allow_pending: bool,
) -> Result<(), DbErr> {
    let allowed = if allow_pending {
        "'open', 'closed', 'archived', 'pending'"
    } else {
        "'open', 'closed', 'archived'"
    };
    let connection = manager.get_connection();
    connection
        .execute_unprepared("DROP TRIGGER IF EXISTS forum_topics_status_insert")
        .await?;
    connection
        .execute_unprepared("DROP TRIGGER IF EXISTS forum_topics_status_update")
        .await?;
    connection
        .execute_unprepared(&format!(
            r#"CREATE TRIGGER forum_topics_status_insert
           BEFORE INSERT ON forum_topics
           FOR EACH ROW
           WHEN NEW.status NOT IN ({allowed})
           BEGIN
               SELECT RAISE(ABORT, 'invalid forum topic status');
           END"#
        ))
        .await?;
    connection
        .execute_unprepared(&format!(
            r#"CREATE TRIGGER forum_topics_status_update
           BEFORE UPDATE OF status ON forum_topics
           FOR EACH ROW
           WHEN NEW.status NOT IN ({allowed})
           BEGIN
               SELECT RAISE(ABORT, 'invalid forum topic status');
           END"#
        ))
        .await?;
    Ok(())
}

async fn ensure_no_pending_topics(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    let backend = manager.get_database_backend();
    let row = manager
        .get_connection()
        .query_one_raw(Statement::from_string(
            backend,
            "SELECT COUNT(*) AS pending_count FROM forum_topics WHERE status = 'pending'"
                .to_string(),
        ))
        .await?
        .ok_or_else(|| DbErr::Custom("failed to count pending forum topics".to_string()))?;
    let pending_count: i64 = row.try_get("", "pending_count")?;
    if pending_count != 0 {
        return Err(DbErr::Custom(
            "pending topic status migration cannot be reverted while topics are pending"
                .to_string(),
        ));
    }
    Ok(())
}
