use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::ConnectionTrait;

pub(super) async fn apply(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    // Auto-subscribe is a Rust owner rule, not a database trigger: the topic,
    // reply and moderation services create the subscription row inside their
    // write transaction (see `services::subscription::auto_subscribe`).
    // Trigger-based auto-subscribe was removed because it upgraded an
    // explicit `normal` (cleared) row back to a subscribed level without the
    // user's consent and was invisible to the Rust owner contract.
    // Migration `m20261102_000040` drops the triggers on databases that
    // already applied the original version of this migration.
    let connection = manager.get_connection();
    for statement in [
        "DROP TRIGGER IF EXISTS forum_70_auto_subscribe_topic_author",
        "DROP TRIGGER IF EXISTS forum_70_auto_subscribe_reply_participant_insert",
        "DROP TRIGGER IF EXISTS forum_70_auto_subscribe_reply_participant_update",
    ] {
        connection.execute_unprepared(statement).await?;
    }
    Ok(())
}
