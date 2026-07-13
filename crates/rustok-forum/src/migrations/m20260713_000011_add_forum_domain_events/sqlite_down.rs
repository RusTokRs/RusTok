use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::ConnectionTrait;

pub(super) async fn down_sqlite(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    let connection = manager.get_connection();

    for statement in [
        r##"DROP TRIGGER IF EXISTS forum_80_topic_tag_delete_event"##,
        r##"DROP TRIGGER IF EXISTS forum_80_topic_tag_insert_event"##,
        r##"DROP TRIGGER IF EXISTS forum_80_topic_subscription_delete_event"##,
        r##"DROP TRIGGER IF EXISTS forum_80_topic_subscription_insert_event"##,
        r##"DROP TRIGGER IF EXISTS forum_80_category_subscription_delete_event"##,
        r##"DROP TRIGGER IF EXISTS forum_80_category_subscription_insert_event"##,
        r##"DROP TRIGGER IF EXISTS forum_80_reply_vote_delete_event"##,
        r##"DROP TRIGGER IF EXISTS forum_80_reply_vote_update_event"##,
        r##"DROP TRIGGER IF EXISTS forum_80_reply_vote_insert_event"##,
        r##"DROP TRIGGER IF EXISTS forum_80_topic_vote_delete_event"##,
        r##"DROP TRIGGER IF EXISTS forum_80_topic_vote_update_event"##,
        r##"DROP TRIGGER IF EXISTS forum_80_topic_vote_insert_event"##,
        r##"DROP TRIGGER IF EXISTS forum_80_solution_unmarked_event"##,
        r##"DROP TRIGGER IF EXISTS forum_80_solution_marked_event"##,
        r##"DROP TRIGGER IF EXISTS forum_80_reply_body_update_event"##,
        r##"DROP TRIGGER IF EXISTS forum_80_reply_body_insert_event"##,
        r##"DROP TRIGGER IF EXISTS forum_80_reply_deleted_event"##,
        r##"DROP TRIGGER IF EXISTS forum_80_reply_status_event"##,
        r##"DROP TRIGGER IF EXISTS forum_80_reply_created_event"##,
        r##"DROP TRIGGER IF EXISTS forum_80_topic_translation_update_event"##,
        r##"DROP TRIGGER IF EXISTS forum_80_topic_translation_insert_event"##,
        r##"DROP TRIGGER IF EXISTS forum_80_topic_deleted_event"##,
        r##"DROP TRIGGER IF EXISTS forum_80_topic_lock_event"##,
        r##"DROP TRIGGER IF EXISTS forum_80_topic_pinned_event"##,
        r##"DROP TRIGGER IF EXISTS forum_80_topic_status_event"##,
        r##"DROP TRIGGER IF EXISTS forum_80_topic_updated_event"##,
        r##"DROP TRIGGER IF EXISTS forum_80_topic_created_event"##,
        r##"DROP TRIGGER IF EXISTS forum_80_category_translation_update_event"##,
        r##"DROP TRIGGER IF EXISTS forum_80_category_translation_insert_event"##,
        r##"DROP TRIGGER IF EXISTS forum_80_category_deleted_event"##,
        r##"DROP TRIGGER IF EXISTS forum_80_category_updated_event"##,
        r##"DROP TRIGGER IF EXISTS forum_80_category_created_event"##,
        r##"DROP TRIGGER IF EXISTS forum_domain_events_immutable_delete"##,
        r##"DROP TRIGGER IF EXISTS forum_domain_events_immutable_update"##,
        r##"DROP INDEX IF EXISTS idx_forum_domain_events_tenant_type"##,
        r##"DROP INDEX IF EXISTS idx_forum_domain_events_tenant_aggregate"##,
        r##"DROP INDEX IF EXISTS idx_forum_domain_events_tenant_sequence"##,
        r##"DROP TABLE IF EXISTS forum_domain_events"##,
    ] {
        connection.execute_unprepared(statement).await?;
    }

    Ok(())
}
