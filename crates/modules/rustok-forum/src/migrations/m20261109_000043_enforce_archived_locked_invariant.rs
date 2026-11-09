use sea_orm::{DatabaseBackend, Statement};
use sea_orm_migration::prelude::*;

/// Require archived topics and their restore snapshots to remain locked.
#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    fn use_transaction(&self) -> Option<bool> {
        Some(true)
    }

    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        match manager.get_database_backend() {
            DatabaseBackend::Postgres => up_postgres(manager).await,
            DatabaseBackend::Sqlite => up_sqlite(manager).await,
            backend => Err(DbErr::Custom(format!(
                "archived locked invariant does not support {backend:?}"
            ))),
        }
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        match manager.get_database_backend() {
            DatabaseBackend::Postgres => down_postgres(manager).await,
            DatabaseBackend::Sqlite => down_sqlite(manager).await,
            backend => Err(DbErr::Custom(format!(
                "archived locked invariant rollback does not support {backend:?}"
            ))),
        }
    }
}

async fn up_postgres(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    let db = manager.get_connection();
    db.execute_unprepared(
        r#"
            ALTER TABLE forum_topics
                ADD CONSTRAINT forum_topics_archived_implies_locked
                CHECK (status != 'archived' OR is_locked = TRUE) NOT VALID;
            "#,
    )
    .await?;
    normalize_archived_locks(manager).await?;
    db.execute_unprepared(
        "ALTER TABLE forum_topics VALIDATE CONSTRAINT forum_topics_archived_implies_locked",
    )
    .await?;
    Ok(())
}

async fn up_sqlite(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    normalize_archived_locks(manager).await?;
    let conn = manager.get_connection();
    conn.execute_unprepared(
        r#"
        CREATE TRIGGER forum_topics_archived_locked_check
        BEFORE INSERT ON forum_topics
        FOR EACH ROW WHEN NEW.status = 'archived' AND NEW.is_locked = 0
        BEGIN
            SELECT RAISE(ABORT, 'archived forum topic must be locked');
        END
        "#,
    )
    .await?;
    conn.execute_unprepared(
        r#"
        CREATE TRIGGER forum_topics_archived_locked_update_check
        BEFORE UPDATE ON forum_topics
        FOR EACH ROW WHEN NEW.status = 'archived' AND NEW.is_locked = 0
        BEGIN
            SELECT RAISE(ABORT, 'archived forum topic must be locked');
        END
        "#,
    )
    .await?;
    Ok(())
}

async fn normalize_archived_locks(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    let db = manager.get_connection();
    let unlocked_deleted = Statement::from_string(
        manager.get_database_backend(),
        "SELECT 1 FROM forum_topics WHERE deleted_at IS NOT NULL AND NOT is_locked LIMIT 1",
    );
    if db.query_one_raw(unlocked_deleted).await?.is_some() {
        return Err(DbErr::Custom(
            "cannot normalize a deleted Forum topic without changing its immutable state".into(),
        ));
    }
    db.execute_unprepared(
        "UPDATE forum_topics SET is_locked = TRUE WHERE status = 'archived' AND NOT is_locked AND deleted_at IS NULL",
    )
    .await?;
    db.execute_unprepared(
        "UPDATE forum_topic_delete_snapshots SET previous_is_locked = TRUE WHERE previous_status = 'archived' AND NOT previous_is_locked",
    )
    .await?;
    Ok(())
}

// Rollback removes enforcement. It preserves normalized locks because their previous values
// cannot be recovered without overwriting later moderation decisions.
async fn down_postgres(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .get_connection()
        .execute_unprepared(
            "ALTER TABLE forum_topics DROP CONSTRAINT IF EXISTS forum_topics_archived_implies_locked;",
        )
        .await?;
    Ok(())
}

async fn down_sqlite(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    let conn = manager.get_connection();
    conn.execute_unprepared("DROP TRIGGER IF EXISTS forum_topics_archived_locked_check")
        .await?;
    conn.execute_unprepared("DROP TRIGGER IF EXISTS forum_topics_archived_locked_update_check")
        .await?;
    Ok(())
}
