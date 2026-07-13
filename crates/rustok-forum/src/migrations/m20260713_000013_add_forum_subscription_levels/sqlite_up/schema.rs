use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::ConnectionTrait;

pub(super) async fn apply(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    let connection = manager.get_connection();
    for statement in [
        "ALTER TABLE forum_category_subscriptions ADD COLUMN level TEXT NOT NULL DEFAULT 'watching'",
        "ALTER TABLE forum_category_subscriptions ADD COLUMN notify_mentions INTEGER NOT NULL DEFAULT 1",
        "ALTER TABLE forum_category_subscriptions ADD COLUMN notify_replies INTEGER NOT NULL DEFAULT 1",
        "ALTER TABLE forum_category_subscriptions ADD COLUMN notify_new_topics INTEGER NOT NULL DEFAULT 1",
        "ALTER TABLE forum_category_subscriptions ADD COLUMN digest_mode TEXT NOT NULL DEFAULT 'immediate'",
        "ALTER TABLE forum_category_subscriptions ADD COLUMN last_notified_at TEXT NULL",
        "ALTER TABLE forum_category_subscriptions ADD COLUMN revision INTEGER NOT NULL DEFAULT 1",
        "ALTER TABLE forum_category_subscriptions ADD COLUMN updated_at TEXT NULL",
        "UPDATE forum_category_subscriptions SET updated_at = COALESCE(updated_at, created_at, CURRENT_TIMESTAMP)",
        "ALTER TABLE forum_topic_subscriptions ADD COLUMN level TEXT NOT NULL DEFAULT 'watching'",
        "ALTER TABLE forum_topic_subscriptions ADD COLUMN notify_mentions INTEGER NOT NULL DEFAULT 1",
        "ALTER TABLE forum_topic_subscriptions ADD COLUMN notify_replies INTEGER NOT NULL DEFAULT 1",
        "ALTER TABLE forum_topic_subscriptions ADD COLUMN notify_new_topics INTEGER NOT NULL DEFAULT 1",
        "ALTER TABLE forum_topic_subscriptions ADD COLUMN digest_mode TEXT NOT NULL DEFAULT 'immediate'",
        "ALTER TABLE forum_topic_subscriptions ADD COLUMN last_notified_at TEXT NULL",
        "ALTER TABLE forum_topic_subscriptions ADD COLUMN revision INTEGER NOT NULL DEFAULT 1",
        "ALTER TABLE forum_topic_subscriptions ADD COLUMN updated_at TEXT NULL",
        "UPDATE forum_topic_subscriptions SET updated_at = COALESCE(updated_at, created_at, CURRENT_TIMESTAMP)",
        r#"CREATE TABLE IF NOT EXISTS forum_subscription_policies (
            tenant_id TEXT PRIMARY KEY NOT NULL,
            auto_subscribe_topic_authors INTEGER NOT NULL DEFAULT 1,
            topic_author_level TEXT NOT NULL DEFAULT 'watching',
            auto_subscribe_reply_participants INTEGER NOT NULL DEFAULT 1,
            reply_participant_level TEXT NOT NULL DEFAULT 'tracking',
            revision INTEGER NOT NULL DEFAULT 1,
            created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
        )"#,
        "CREATE INDEX IF NOT EXISTS idx_forum_category_subscriptions_user_level ON forum_category_subscriptions (tenant_id, user_id, level, category_id)",
        "CREATE INDEX IF NOT EXISTS idx_forum_topic_subscriptions_user_level ON forum_topic_subscriptions (tenant_id, user_id, level, topic_id)",
    ] {
        connection.execute_unprepared(statement).await?;
    }
    Ok(())
}
