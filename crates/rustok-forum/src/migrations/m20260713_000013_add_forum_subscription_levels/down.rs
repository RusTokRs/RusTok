use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::ConnectionTrait;

pub(super) async fn postgres(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .get_connection()
        .execute_unprepared(
            r#"
DROP TRIGGER IF EXISTS forum_70_auto_subscribe_reply_participant ON forum_replies;
DROP TRIGGER IF EXISTS forum_70_auto_subscribe_topic_author ON forum_topics;
DROP TRIGGER IF EXISTS forum_60_subscription_policy_revision ON forum_subscription_policies;
DROP TRIGGER IF EXISTS forum_60_topic_subscription_revision ON forum_topic_subscriptions;
DROP TRIGGER IF EXISTS forum_60_category_subscription_revision ON forum_category_subscriptions;
DROP FUNCTION IF EXISTS forum_auto_subscribe_reply_participant();
DROP FUNCTION IF EXISTS forum_auto_subscribe_topic_author();
DROP FUNCTION IF EXISTS forum_validate_subscription_policy_revision();
DROP FUNCTION IF EXISTS forum_validate_subscription_revision();
DROP TABLE IF EXISTS forum_subscription_policies;
DROP INDEX IF EXISTS idx_forum_topic_subscriptions_user_level;
DROP INDEX IF EXISTS idx_forum_category_subscriptions_user_level;

ALTER TABLE forum_topic_subscriptions
    DROP COLUMN IF EXISTS updated_at, DROP COLUMN IF EXISTS revision,
    DROP COLUMN IF EXISTS last_notified_at, DROP COLUMN IF EXISTS digest_mode,
    DROP COLUMN IF EXISTS notify_new_topics, DROP COLUMN IF EXISTS notify_replies,
    DROP COLUMN IF EXISTS notify_mentions, DROP COLUMN IF EXISTS level;
ALTER TABLE forum_category_subscriptions
    DROP COLUMN IF EXISTS updated_at, DROP COLUMN IF EXISTS revision,
    DROP COLUMN IF EXISTS last_notified_at, DROP COLUMN IF EXISTS digest_mode,
    DROP COLUMN IF EXISTS notify_new_topics, DROP COLUMN IF EXISTS notify_replies,
    DROP COLUMN IF EXISTS notify_mentions, DROP COLUMN IF EXISTS level;

CREATE OR REPLACE FUNCTION forum_emit_category_subscription_event()
RETURNS trigger AS $$
BEGIN
    IF TG_OP = 'INSERT' THEN
        PERFORM forum_append_domain_event(
            NEW.tenant_id, 'category', NEW.category_id,
            'forum.category.subscription_changed', NEW.user_id,
            jsonb_build_object('category_id', NEW.category_id, 'user_id', NEW.user_id, 'subscribed', TRUE)
        );
        RETURN NEW;
    END IF;
    PERFORM forum_append_domain_event(
        OLD.tenant_id, 'category', OLD.category_id,
        'forum.category.subscription_changed', OLD.user_id,
        jsonb_build_object('category_id', OLD.category_id, 'user_id', OLD.user_id, 'subscribed', FALSE)
    );
    RETURN OLD;
END;
$$ LANGUAGE plpgsql;

CREATE OR REPLACE FUNCTION forum_emit_topic_subscription_event()
RETURNS trigger AS $$
BEGIN
    IF TG_OP = 'INSERT' THEN
        PERFORM forum_append_domain_event(
            NEW.tenant_id, 'topic', NEW.topic_id,
            'forum.topic.subscription_changed', NEW.user_id,
            jsonb_build_object('topic_id', NEW.topic_id, 'user_id', NEW.user_id, 'subscribed', TRUE)
        );
        RETURN NEW;
    END IF;
    PERFORM forum_append_domain_event(
        OLD.tenant_id, 'topic', OLD.topic_id,
        'forum.topic.subscription_changed', OLD.user_id,
        jsonb_build_object('topic_id', OLD.topic_id, 'user_id', OLD.user_id, 'subscribed', FALSE)
    );
    RETURN OLD;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS forum_80_category_subscription_events ON forum_category_subscriptions;
CREATE TRIGGER forum_80_category_subscription_events
AFTER INSERT OR DELETE ON forum_category_subscriptions
FOR EACH ROW EXECUTE FUNCTION forum_emit_category_subscription_event();
DROP TRIGGER IF EXISTS forum_80_topic_subscription_events ON forum_topic_subscriptions;
CREATE TRIGGER forum_80_topic_subscription_events
AFTER INSERT OR DELETE ON forum_topic_subscriptions
FOR EACH ROW EXECUTE FUNCTION forum_emit_topic_subscription_event();
"#,
        )
        .await?;
    Ok(())
}

pub(super) async fn sqlite(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    let connection = manager.get_connection();
    for statement in [
        "DROP TRIGGER IF EXISTS forum_70_auto_subscribe_reply_participant_update",
        "DROP TRIGGER IF EXISTS forum_70_auto_subscribe_reply_participant_insert",
        "DROP TRIGGER IF EXISTS forum_70_auto_subscribe_topic_author",
        "DROP TRIGGER IF EXISTS forum_validate_subscription_policy_update",
        "DROP TRIGGER IF EXISTS forum_validate_subscription_policy",
        "DROP TRIGGER IF EXISTS forum_validate_topic_subscription_update",
        "DROP TRIGGER IF EXISTS forum_validate_topic_subscription_insert",
        "DROP TRIGGER IF EXISTS forum_validate_category_subscription_update",
        "DROP TRIGGER IF EXISTS forum_validate_category_subscription_insert",
        "DROP TRIGGER IF EXISTS forum_80_topic_subscription_insert_event",
        "DROP TRIGGER IF EXISTS forum_80_topic_subscription_update_event",
        "DROP TRIGGER IF EXISTS forum_80_topic_subscription_delete_event",
        "DROP TRIGGER IF EXISTS forum_80_category_subscription_insert_event",
        "DROP TRIGGER IF EXISTS forum_80_category_subscription_update_event",
        "DROP TRIGGER IF EXISTS forum_80_category_subscription_delete_event",
        r#"CREATE TRIGGER forum_80_category_subscription_insert_event AFTER INSERT ON forum_category_subscriptions
        FOR EACH ROW BEGIN INSERT INTO forum_domain_events
        (event_id,tenant_id,aggregate_type,aggregate_id,event_type,schema_version,actor_id,payload)
        VALUES (lower(hex(randomblob(4)))||'-'||lower(hex(randomblob(2)))||'-'||lower(hex(randomblob(2)))||'-'||lower(hex(randomblob(2)))||'-'||lower(hex(randomblob(6))),
        NEW.tenant_id,'category',NEW.category_id,'forum.category.subscription_changed',1,NEW.user_id,
        json_object('category_id',NEW.category_id,'user_id',NEW.user_id,'subscribed',1)); END"#,
        r#"CREATE TRIGGER forum_80_category_subscription_delete_event AFTER DELETE ON forum_category_subscriptions
        FOR EACH ROW BEGIN INSERT INTO forum_domain_events
        (event_id,tenant_id,aggregate_type,aggregate_id,event_type,schema_version,actor_id,payload)
        VALUES (lower(hex(randomblob(4)))||'-'||lower(hex(randomblob(2)))||'-'||lower(hex(randomblob(2)))||'-'||lower(hex(randomblob(2)))||'-'||lower(hex(randomblob(6))),
        OLD.tenant_id,'category',OLD.category_id,'forum.category.subscription_changed',1,OLD.user_id,
        json_object('category_id',OLD.category_id,'user_id',OLD.user_id,'subscribed',0)); END"#,
        r#"CREATE TRIGGER forum_80_topic_subscription_insert_event AFTER INSERT ON forum_topic_subscriptions
        FOR EACH ROW BEGIN INSERT INTO forum_domain_events
        (event_id,tenant_id,aggregate_type,aggregate_id,event_type,schema_version,actor_id,payload)
        VALUES (lower(hex(randomblob(4)))||'-'||lower(hex(randomblob(2)))||'-'||lower(hex(randomblob(2)))||'-'||lower(hex(randomblob(2)))||'-'||lower(hex(randomblob(6))),
        NEW.tenant_id,'topic',NEW.topic_id,'forum.topic.subscription_changed',1,NEW.user_id,
        json_object('topic_id',NEW.topic_id,'user_id',NEW.user_id,'subscribed',1)); END"#,
        r#"CREATE TRIGGER forum_80_topic_subscription_delete_event AFTER DELETE ON forum_topic_subscriptions
        FOR EACH ROW BEGIN INSERT INTO forum_domain_events
        (event_id,tenant_id,aggregate_type,aggregate_id,event_type,schema_version,actor_id,payload)
        VALUES (lower(hex(randomblob(4)))||'-'||lower(hex(randomblob(2)))||'-'||lower(hex(randomblob(2)))||'-'||lower(hex(randomblob(2)))||'-'||lower(hex(randomblob(6))),
        OLD.tenant_id,'topic',OLD.topic_id,'forum.topic.subscription_changed',1,OLD.user_id,
        json_object('topic_id',OLD.topic_id,'user_id',OLD.user_id,'subscribed',0)); END"#,
        "DROP TABLE IF EXISTS forum_subscription_policies",
        "DROP INDEX IF EXISTS idx_forum_topic_subscriptions_user_level",
        "DROP INDEX IF EXISTS idx_forum_category_subscriptions_user_level",
    ] {
        connection.execute_unprepared(statement).await?;
    }
    Ok(())
}
