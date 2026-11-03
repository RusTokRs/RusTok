use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::{ConnectionTrait, DatabaseBackend};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Auto-subscribe moved from database triggers into the Rust owner
        // services (`services::subscription::auto_subscribe`), which run
        // inside the topic/reply/moderation write transactions and never
        // overwrite an existing explicit subscription row. Databases that
        // applied the original `m20260713_000013` still carry the
        // `forum_70_*` triggers, so drop them (and the backing functions)
        // here; fresh databases never create them.
        let sql = if manager.get_database_backend() == DatabaseBackend::Sqlite {
            r#"
DROP TRIGGER IF EXISTS forum_70_auto_subscribe_topic_author;
DROP TRIGGER IF EXISTS forum_70_auto_subscribe_reply_participant_insert;
DROP TRIGGER IF EXISTS forum_70_auto_subscribe_reply_participant_update;
DROP TRIGGER IF EXISTS forum_70_auto_subscribe_reply_participant;
"#
        } else {
            r#"
DROP TRIGGER IF EXISTS forum_70_auto_subscribe_topic_author ON forum_topics;
DROP TRIGGER IF EXISTS forum_70_auto_subscribe_reply_participant ON forum_replies;
DROP FUNCTION IF EXISTS forum_auto_subscribe_reply_participant();
DROP FUNCTION IF EXISTS forum_auto_subscribe_topic_author();
"#
        };
        manager.get_connection().execute_unprepared(sql).await?;
        Ok(())
    }

    async fn down(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
        // Irreversible cutover: the trigger implementation was removed from
        // the codebase (zero-legacy policy) and auto-subscribe is owned by
        // the Rust services. There is no supported downgrade path that
        // recreates the consent-violating trigger behavior.
        Ok(())
    }
}
