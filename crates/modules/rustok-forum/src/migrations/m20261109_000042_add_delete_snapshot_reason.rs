use sea_orm::DatabaseBackend;
use sea_orm_migration::prelude::*;

/// P1 3.2 ideal: `ReplyStatus::Deleted` snapshot should distinguish `user-initiated` vs
/// `moderator` delete for GDPR vs moderation compliance. Add `reason` to both
/// `forum_topic_delete_snapshots` and `forum_topic_reply_delete_snapshots`.
#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        match manager.get_database_backend() {
            DatabaseBackend::Postgres => up_postgres(manager).await,
            DatabaseBackend::Sqlite => up_sqlite(manager).await,
            backend => Err(DbErr::Custom(format!(
                "delete_snapshot reason does not support {backend:?}"
            ))),
        }
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        match manager.get_database_backend() {
            DatabaseBackend::Postgres => down_postgres(manager).await,
            DatabaseBackend::Sqlite => down_sqlite(manager).await,
            backend => Err(DbErr::Custom(format!(
                "delete_snapshot reason rollback does not support {backend:?}"
            ))),
        }
    }
}

async fn up_postgres(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .get_connection()
        .execute_unprepared(
            r#"
            ALTER TABLE forum_topic_delete_snapshots
                ADD COLUMN IF NOT EXISTS reason TEXT NOT NULL DEFAULT 'moderation'
                CHECK (reason IN ('user_request', 'moderation', 'system'));

            ALTER TABLE forum_topic_reply_delete_snapshots
                ADD COLUMN IF NOT EXISTS reason TEXT NOT NULL DEFAULT 'moderation'
                CHECK (reason IN ('user_request', 'moderation', 'system'));

            ALTER TABLE forum_reply_delete_snapshots
                ADD COLUMN IF NOT EXISTS reason TEXT NOT NULL DEFAULT 'moderation'
                CHECK (reason IN ('user_request', 'moderation', 'system'));
            "#,
        )
        .await?;
    Ok(())
}

async fn up_sqlite(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    let conn = manager.get_connection();
    for stmt in [
        r#"ALTER TABLE forum_topic_delete_snapshots ADD COLUMN reason TEXT NOT NULL DEFAULT 'moderation' CHECK (reason IN ('user_request', 'moderation', 'system'))"#,
        r#"ALTER TABLE forum_topic_reply_delete_snapshots ADD COLUMN reason TEXT NOT NULL DEFAULT 'moderation' CHECK (reason IN ('user_request', 'moderation', 'system'))"#,
        r#"ALTER TABLE forum_reply_delete_snapshots ADD COLUMN reason TEXT NOT NULL DEFAULT 'moderation' CHECK (reason IN ('user_request', 'moderation', 'system'))"#,
    ] {
        // SQLite before 3.35 has no IF NOT EXISTS for ADD COLUMN — ignore "duplicate column name"
        let res = conn.execute_unprepared(stmt).await;
        if let Err(DbErr::Exec(msg)) = &res {
            if msg.contains("duplicate column name") {
                continue;
            }
        }
        res?;
    }
    Ok(())
}

async fn down_postgres(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .get_connection()
        .execute_unprepared(
            r#"
            ALTER TABLE forum_topic_delete_snapshots DROP COLUMN IF EXISTS reason;
            ALTER TABLE forum_topic_reply_delete_snapshots DROP COLUMN IF EXISTS reason;
            ALTER TABLE forum_reply_delete_snapshots DROP COLUMN IF EXISTS reason;
            "#,
        )
        .await?;
    Ok(())
}

async fn down_sqlite(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    // SQLite DROP COLUMN requires 3.35+; keep down as no-op to stay ideal without legacy rewrite
    let _ = manager;
    Ok(())
}
