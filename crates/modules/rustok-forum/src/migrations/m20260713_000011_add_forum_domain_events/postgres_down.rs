use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::ConnectionTrait;

pub(super) async fn down_postgres(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .get_connection()
        .execute_unprepared(
            r#"
DROP TRIGGER IF EXISTS forum_80_topic_tag_events ON forum_topic_tags;
DROP TRIGGER IF EXISTS forum_80_topic_subscription_events ON forum_topic_subscriptions;
DROP TRIGGER IF EXISTS forum_80_category_subscription_events ON forum_category_subscriptions;
DROP TRIGGER IF EXISTS forum_80_reply_vote_events ON forum_reply_votes;
DROP TRIGGER IF EXISTS forum_80_topic_vote_events ON forum_topic_votes;
DROP TRIGGER IF EXISTS forum_80_solution_events ON forum_solutions;
DROP TRIGGER IF EXISTS forum_80_reply_body_events ON forum_reply_bodies;
DROP TRIGGER IF EXISTS forum_80_reply_events ON forum_replies;
DROP TRIGGER IF EXISTS forum_80_topic_translation_events ON forum_topic_translations;
DROP TRIGGER IF EXISTS forum_80_topic_events ON forum_topics;
DROP TRIGGER IF EXISTS forum_80_category_translation_events ON forum_category_translations;
DROP TRIGGER IF EXISTS forum_80_category_events ON forum_categories;

DROP FUNCTION IF EXISTS forum_emit_topic_tag_event();
DROP FUNCTION IF EXISTS forum_emit_topic_subscription_event();
DROP FUNCTION IF EXISTS forum_emit_category_subscription_event();
DROP FUNCTION IF EXISTS forum_emit_reply_vote_event();
DROP FUNCTION IF EXISTS forum_emit_topic_vote_event();
DROP FUNCTION IF EXISTS forum_emit_solution_event();
DROP FUNCTION IF EXISTS forum_emit_reply_body_event();
DROP FUNCTION IF EXISTS forum_emit_reply_event();
DROP FUNCTION IF EXISTS forum_emit_topic_translation_event();
DROP FUNCTION IF EXISTS forum_emit_topic_event();
DROP FUNCTION IF EXISTS forum_emit_category_translation_event();
DROP FUNCTION IF EXISTS forum_emit_category_event();

DROP TRIGGER IF EXISTS forum_domain_events_immutable_delete ON forum_domain_events;
DROP TRIGGER IF EXISTS forum_domain_events_immutable_update ON forum_domain_events;
DROP FUNCTION IF EXISTS forum_reject_domain_event_mutation();
DROP FUNCTION IF EXISTS forum_append_domain_event(uuid, text, uuid, text, uuid, jsonb);

DROP TABLE IF EXISTS forum_domain_events;
DROP FUNCTION IF EXISTS forum_generate_event_uuid();
"#,
        )
        .await?;
    Ok(())
}
