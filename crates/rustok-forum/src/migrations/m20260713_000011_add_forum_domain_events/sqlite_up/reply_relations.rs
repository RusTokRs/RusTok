use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::ConnectionTrait;

pub(super) async fn reply_relations(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    let connection = manager.get_connection();

    for statement in [
        r##"DROP TRIGGER IF EXISTS forum_80_reply_created_event"##,
        r##"CREATE TRIGGER forum_80_reply_created_event
AFTER INSERT ON forum_replies
FOR EACH ROW
BEGIN
INSERT INTO forum_domain_events (
        event_id, tenant_id, aggregate_type, aggregate_id,
        event_type, schema_version, actor_id, payload
    ) VALUES (
        lower(hex(randomblob(4))) || '-' ||
               lower(hex(randomblob(2))) || '-' ||
               lower(hex(randomblob(2))) || '-' ||
               lower(hex(randomblob(2))) || '-' ||
               lower(hex(randomblob(6))),
        NEW.tenant_id, 'reply', NEW.id,
        'forum.reply.created', 1, NEW.author_id, json_object('reply_id', NEW.id, 'topic_id', NEW.topic_id, 'author_id', NEW.author_id, 'parent_reply_id', NEW.parent_reply_id, 'status', NEW.status, 'position', NEW.position)
    );
END"##,
        r##"DROP TRIGGER IF EXISTS forum_80_reply_status_event"##,
        r##"CREATE TRIGGER forum_80_reply_status_event
AFTER UPDATE OF status ON forum_replies
FOR EACH ROW
WHEN OLD.status IS NOT NEW.status
BEGIN
INSERT INTO forum_domain_events (
        event_id, tenant_id, aggregate_type, aggregate_id,
        event_type, schema_version, actor_id, payload
    ) VALUES (
        lower(hex(randomblob(4))) || '-' ||
               lower(hex(randomblob(2))) || '-' ||
               lower(hex(randomblob(2))) || '-' ||
               lower(hex(randomblob(2))) || '-' ||
               lower(hex(randomblob(6))),
        NEW.tenant_id, 'reply', NEW.id,
        'forum.reply.status_changed', 1, NULL, json_object('reply_id', NEW.id, 'topic_id', NEW.topic_id, 'old_status', OLD.status, 'new_status', NEW.status)
    );
END"##,
        r##"DROP TRIGGER IF EXISTS forum_80_reply_deleted_event"##,
        r##"CREATE TRIGGER forum_80_reply_deleted_event
AFTER UPDATE OF deleted_at ON forum_replies
FOR EACH ROW
WHEN OLD.deleted_at IS NULL AND NEW.deleted_at IS NOT NULL
BEGIN
INSERT INTO forum_domain_events (
        event_id, tenant_id, aggregate_type, aggregate_id,
        event_type, schema_version, actor_id, payload
    ) VALUES (
        lower(hex(randomblob(4))) || '-' ||
               lower(hex(randomblob(2))) || '-' ||
               lower(hex(randomblob(2))) || '-' ||
               lower(hex(randomblob(2))) || '-' ||
               lower(hex(randomblob(6))),
        NEW.tenant_id, 'reply', NEW.id,
        'forum.reply.deleted', 1, NULL, json_object('reply_id', NEW.id, 'topic_id', NEW.topic_id, 'deleted_at', NEW.deleted_at)
    );
END"##,
        r##"DROP TRIGGER IF EXISTS forum_80_reply_body_insert_event"##,
        r##"CREATE TRIGGER forum_80_reply_body_insert_event
AFTER INSERT ON forum_reply_bodies
FOR EACH ROW
WHEN NEW.body <> '[deleted]'
 AND (
    SELECT COUNT(*)
    FROM forum_reply_bodies
    WHERE tenant_id = NEW.tenant_id
      AND reply_id = NEW.reply_id
 ) > 1
BEGIN
INSERT INTO forum_domain_events (
        event_id, tenant_id, aggregate_type, aggregate_id,
        event_type, schema_version, actor_id, payload
    ) VALUES (
        lower(hex(randomblob(4))) || '-' ||
               lower(hex(randomblob(2))) || '-' ||
               lower(hex(randomblob(2))) || '-' ||
               lower(hex(randomblob(2))) || '-' ||
               lower(hex(randomblob(6))),
        NEW.tenant_id, 'reply', NEW.reply_id,
        'forum.reply.updated', 1, NULL, json_object('reply_id', NEW.reply_id, 'change_scope', 'body', 'locale', NEW.locale)
    );
END"##,
        r##"DROP TRIGGER IF EXISTS forum_80_reply_body_update_event"##,
        r##"CREATE TRIGGER forum_80_reply_body_update_event
AFTER UPDATE ON forum_reply_bodies
FOR EACH ROW
WHEN NEW.body <> '[deleted]'
 AND (
    OLD.body IS NOT NEW.body
    OR OLD.body_format IS NOT NEW.body_format
 )
BEGIN
INSERT INTO forum_domain_events (
        event_id, tenant_id, aggregate_type, aggregate_id,
        event_type, schema_version, actor_id, payload
    ) VALUES (
        lower(hex(randomblob(4))) || '-' ||
               lower(hex(randomblob(2))) || '-' ||
               lower(hex(randomblob(2))) || '-' ||
               lower(hex(randomblob(2))) || '-' ||
               lower(hex(randomblob(6))),
        NEW.tenant_id, 'reply', NEW.reply_id,
        'forum.reply.updated', 1, NULL, json_object('reply_id', NEW.reply_id, 'change_scope', 'body', 'locale', NEW.locale)
    );
END"##,
        r##"DROP TRIGGER IF EXISTS forum_80_solution_marked_event"##,
        r##"CREATE TRIGGER forum_80_solution_marked_event
AFTER INSERT ON forum_solutions
FOR EACH ROW
BEGIN
INSERT INTO forum_domain_events (
        event_id, tenant_id, aggregate_type, aggregate_id,
        event_type, schema_version, actor_id, payload
    ) VALUES (
        lower(hex(randomblob(4))) || '-' ||
               lower(hex(randomblob(2))) || '-' ||
               lower(hex(randomblob(2))) || '-' ||
               lower(hex(randomblob(2))) || '-' ||
               lower(hex(randomblob(6))),
        NEW.tenant_id, 'topic', NEW.topic_id,
        'forum.solution.marked', 1, NEW.marked_by_user_id, json_object('topic_id', NEW.topic_id, 'reply_id', NEW.reply_id, 'marked_by_user_id', NEW.marked_by_user_id)
    );
END"##,
        r##"DROP TRIGGER IF EXISTS forum_80_solution_unmarked_event"##,
        r##"CREATE TRIGGER forum_80_solution_unmarked_event
AFTER DELETE ON forum_solutions
FOR EACH ROW
BEGIN
INSERT INTO forum_domain_events (
        event_id, tenant_id, aggregate_type, aggregate_id,
        event_type, schema_version, actor_id, payload
    ) VALUES (
        lower(hex(randomblob(4))) || '-' ||
               lower(hex(randomblob(2))) || '-' ||
               lower(hex(randomblob(2))) || '-' ||
               lower(hex(randomblob(2))) || '-' ||
               lower(hex(randomblob(6))),
        OLD.tenant_id, 'topic', OLD.topic_id,
        'forum.solution.unmarked', 1, OLD.marked_by_user_id, json_object('topic_id', OLD.topic_id, 'reply_id', OLD.reply_id, 'marked_by_user_id', OLD.marked_by_user_id)
    );
END"##,
        r##"DROP TRIGGER IF EXISTS forum_80_topic_vote_insert_event"##,
        r##"CREATE TRIGGER forum_80_topic_vote_insert_event
AFTER INSERT ON forum_topic_votes
FOR EACH ROW
BEGIN
INSERT INTO forum_domain_events (
        event_id, tenant_id, aggregate_type, aggregate_id,
        event_type, schema_version, actor_id, payload
    ) VALUES (
        lower(hex(randomblob(4))) || '-' ||
               lower(hex(randomblob(2))) || '-' ||
               lower(hex(randomblob(2))) || '-' ||
               lower(hex(randomblob(2))) || '-' ||
               lower(hex(randomblob(6))),
        NEW.tenant_id, 'topic', NEW.topic_id,
        'forum.topic.vote_changed', 1, NEW.user_id, json_object('topic_id', NEW.topic_id, 'user_id', NEW.user_id, 'previous_value', NULL, 'value', NEW.value)
    );
END"##,
        r##"DROP TRIGGER IF EXISTS forum_80_topic_vote_update_event"##,
        r##"CREATE TRIGGER forum_80_topic_vote_update_event
AFTER UPDATE OF value ON forum_topic_votes
FOR EACH ROW
WHEN OLD.value IS NOT NEW.value
BEGIN
INSERT INTO forum_domain_events (
        event_id, tenant_id, aggregate_type, aggregate_id,
        event_type, schema_version, actor_id, payload
    ) VALUES (
        lower(hex(randomblob(4))) || '-' ||
               lower(hex(randomblob(2))) || '-' ||
               lower(hex(randomblob(2))) || '-' ||
               lower(hex(randomblob(2))) || '-' ||
               lower(hex(randomblob(6))),
        NEW.tenant_id, 'topic', NEW.topic_id,
        'forum.topic.vote_changed', 1, NEW.user_id, json_object('topic_id', NEW.topic_id, 'user_id', NEW.user_id, 'previous_value', OLD.value, 'value', NEW.value)
    );
END"##,
        r##"DROP TRIGGER IF EXISTS forum_80_topic_vote_delete_event"##,
        r##"CREATE TRIGGER forum_80_topic_vote_delete_event
AFTER DELETE ON forum_topic_votes
FOR EACH ROW
BEGIN
INSERT INTO forum_domain_events (
        event_id, tenant_id, aggregate_type, aggregate_id,
        event_type, schema_version, actor_id, payload
    ) VALUES (
        lower(hex(randomblob(4))) || '-' ||
               lower(hex(randomblob(2))) || '-' ||
               lower(hex(randomblob(2))) || '-' ||
               lower(hex(randomblob(2))) || '-' ||
               lower(hex(randomblob(6))),
        OLD.tenant_id, 'topic', OLD.topic_id,
        'forum.topic.vote_changed', 1, OLD.user_id, json_object('topic_id', OLD.topic_id, 'user_id', OLD.user_id, 'previous_value', OLD.value, 'value', NULL)
    );
END"##,
        r##"DROP TRIGGER IF EXISTS forum_80_reply_vote_insert_event"##,
        r##"CREATE TRIGGER forum_80_reply_vote_insert_event
AFTER INSERT ON forum_reply_votes
FOR EACH ROW
BEGIN
INSERT INTO forum_domain_events (
        event_id, tenant_id, aggregate_type, aggregate_id,
        event_type, schema_version, actor_id, payload
    ) VALUES (
        lower(hex(randomblob(4))) || '-' ||
               lower(hex(randomblob(2))) || '-' ||
               lower(hex(randomblob(2))) || '-' ||
               lower(hex(randomblob(2))) || '-' ||
               lower(hex(randomblob(6))),
        NEW.tenant_id, 'reply', NEW.reply_id,
        'forum.reply.vote_changed', 1, NEW.user_id, json_object('reply_id', NEW.reply_id, 'user_id', NEW.user_id, 'previous_value', NULL, 'value', NEW.value)
    );
END"##,
        r##"DROP TRIGGER IF EXISTS forum_80_reply_vote_update_event"##,
        r##"CREATE TRIGGER forum_80_reply_vote_update_event
AFTER UPDATE OF value ON forum_reply_votes
FOR EACH ROW
WHEN OLD.value IS NOT NEW.value
BEGIN
INSERT INTO forum_domain_events (
        event_id, tenant_id, aggregate_type, aggregate_id,
        event_type, schema_version, actor_id, payload
    ) VALUES (
        lower(hex(randomblob(4))) || '-' ||
               lower(hex(randomblob(2))) || '-' ||
               lower(hex(randomblob(2))) || '-' ||
               lower(hex(randomblob(2))) || '-' ||
               lower(hex(randomblob(6))),
        NEW.tenant_id, 'reply', NEW.reply_id,
        'forum.reply.vote_changed', 1, NEW.user_id, json_object('reply_id', NEW.reply_id, 'user_id', NEW.user_id, 'previous_value', OLD.value, 'value', NEW.value)
    );
END"##,
        r##"DROP TRIGGER IF EXISTS forum_80_reply_vote_delete_event"##,
        r##"CREATE TRIGGER forum_80_reply_vote_delete_event
AFTER DELETE ON forum_reply_votes
FOR EACH ROW
BEGIN
INSERT INTO forum_domain_events (
        event_id, tenant_id, aggregate_type, aggregate_id,
        event_type, schema_version, actor_id, payload
    ) VALUES (
        lower(hex(randomblob(4))) || '-' ||
               lower(hex(randomblob(2))) || '-' ||
               lower(hex(randomblob(2))) || '-' ||
               lower(hex(randomblob(2))) || '-' ||
               lower(hex(randomblob(6))),
        OLD.tenant_id, 'reply', OLD.reply_id,
        'forum.reply.vote_changed', 1, OLD.user_id, json_object('reply_id', OLD.reply_id, 'user_id', OLD.user_id, 'previous_value', OLD.value, 'value', NULL)
    );
END"##,
        r##"DROP TRIGGER IF EXISTS forum_80_category_subscription_insert_event"##,
        r##"CREATE TRIGGER forum_80_category_subscription_insert_event
AFTER INSERT ON forum_category_subscriptions
FOR EACH ROW
BEGIN
INSERT INTO forum_domain_events (
        event_id, tenant_id, aggregate_type, aggregate_id,
        event_type, schema_version, actor_id, payload
    ) VALUES (
        lower(hex(randomblob(4))) || '-' ||
               lower(hex(randomblob(2))) || '-' ||
               lower(hex(randomblob(2))) || '-' ||
               lower(hex(randomblob(2))) || '-' ||
               lower(hex(randomblob(6))),
        NEW.tenant_id, 'category', NEW.category_id,
        'forum.category.subscription_changed', 1, NEW.user_id, json_object('category_id', NEW.category_id, 'user_id', NEW.user_id, 'subscribed', 1)
    );
END"##,
        r##"DROP TRIGGER IF EXISTS forum_80_category_subscription_delete_event"##,
        r##"CREATE TRIGGER forum_80_category_subscription_delete_event
AFTER DELETE ON forum_category_subscriptions
FOR EACH ROW
BEGIN
INSERT INTO forum_domain_events (
        event_id, tenant_id, aggregate_type, aggregate_id,
        event_type, schema_version, actor_id, payload
    ) VALUES (
        lower(hex(randomblob(4))) || '-' ||
               lower(hex(randomblob(2))) || '-' ||
               lower(hex(randomblob(2))) || '-' ||
               lower(hex(randomblob(2))) || '-' ||
               lower(hex(randomblob(6))),
        OLD.tenant_id, 'category', OLD.category_id,
        'forum.category.subscription_changed', 1, OLD.user_id, json_object('category_id', OLD.category_id, 'user_id', OLD.user_id, 'subscribed', 0)
    );
END"##,
        r##"DROP TRIGGER IF EXISTS forum_80_topic_subscription_insert_event"##,
        r##"CREATE TRIGGER forum_80_topic_subscription_insert_event
AFTER INSERT ON forum_topic_subscriptions
FOR EACH ROW
BEGIN
INSERT INTO forum_domain_events (
        event_id, tenant_id, aggregate_type, aggregate_id,
        event_type, schema_version, actor_id, payload
    ) VALUES (
        lower(hex(randomblob(4))) || '-' ||
               lower(hex(randomblob(2))) || '-' ||
               lower(hex(randomblob(2))) || '-' ||
               lower(hex(randomblob(2))) || '-' ||
               lower(hex(randomblob(6))),
        NEW.tenant_id, 'topic', NEW.topic_id,
        'forum.topic.subscription_changed', 1, NEW.user_id, json_object('topic_id', NEW.topic_id, 'user_id', NEW.user_id, 'subscribed', 1)
    );
END"##,
        r##"DROP TRIGGER IF EXISTS forum_80_topic_subscription_delete_event"##,
        r##"CREATE TRIGGER forum_80_topic_subscription_delete_event
AFTER DELETE ON forum_topic_subscriptions
FOR EACH ROW
BEGIN
INSERT INTO forum_domain_events (
        event_id, tenant_id, aggregate_type, aggregate_id,
        event_type, schema_version, actor_id, payload
    ) VALUES (
        lower(hex(randomblob(4))) || '-' ||
               lower(hex(randomblob(2))) || '-' ||
               lower(hex(randomblob(2))) || '-' ||
               lower(hex(randomblob(2))) || '-' ||
               lower(hex(randomblob(6))),
        OLD.tenant_id, 'topic', OLD.topic_id,
        'forum.topic.subscription_changed', 1, OLD.user_id, json_object('topic_id', OLD.topic_id, 'user_id', OLD.user_id, 'subscribed', 0)
    );
END"##,
        r##"DROP TRIGGER IF EXISTS forum_80_topic_tag_insert_event"##,
        r##"CREATE TRIGGER forum_80_topic_tag_insert_event
AFTER INSERT ON forum_topic_tags
FOR EACH ROW
BEGIN
INSERT INTO forum_domain_events (
        event_id, tenant_id, aggregate_type, aggregate_id,
        event_type, schema_version, actor_id, payload
    ) VALUES (
        lower(hex(randomblob(4))) || '-' ||
               lower(hex(randomblob(2))) || '-' ||
               lower(hex(randomblob(2))) || '-' ||
               lower(hex(randomblob(2))) || '-' ||
               lower(hex(randomblob(6))),
        NEW.tenant_id, 'topic', NEW.topic_id,
        'forum.topic.tags_changed', 1, NULL, json_object('topic_id', NEW.topic_id, 'term_id', NEW.term_id, 'attached', 1)
    );
END"##,
        r##"DROP TRIGGER IF EXISTS forum_80_topic_tag_delete_event"##,
        r##"CREATE TRIGGER forum_80_topic_tag_delete_event
AFTER DELETE ON forum_topic_tags
FOR EACH ROW
BEGIN
INSERT INTO forum_domain_events (
        event_id, tenant_id, aggregate_type, aggregate_id,
        event_type, schema_version, actor_id, payload
    ) VALUES (
        lower(hex(randomblob(4))) || '-' ||
               lower(hex(randomblob(2))) || '-' ||
               lower(hex(randomblob(2))) || '-' ||
               lower(hex(randomblob(2))) || '-' ||
               lower(hex(randomblob(6))),
        OLD.tenant_id, 'topic', OLD.topic_id,
        'forum.topic.tags_changed', 1, NULL, json_object('topic_id', OLD.topic_id, 'term_id', OLD.term_id, 'attached', 0)
    );
END"##,
    ] {
        connection.execute_unprepared(statement).await?;
    }

    Ok(())
}
