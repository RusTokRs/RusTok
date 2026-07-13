use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::ConnectionTrait;

pub(super) async fn apply(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .get_connection()
        .execute_unprepared(
            r#"
CREATE OR REPLACE FUNCTION forum_emit_category_subscription_event()
RETURNS trigger AS $$
DECLARE
    event_tenant UUID; event_target UUID; event_user UUID;
    old_level TEXT; new_level TEXT; event_digest_mode TEXT;
    event_notify_mentions BOOLEAN; event_notify_replies BOOLEAN;
    event_notify_new_topics BOOLEAN; event_revision BIGINT;
BEGIN
    IF TG_OP = 'INSERT' THEN
        event_tenant := NEW.tenant_id; event_target := NEW.category_id;
        event_user := NEW.user_id; old_level := 'normal'; new_level := NEW.level;
        event_notify_mentions := NEW.notify_mentions;
        event_notify_replies := NEW.notify_replies;
        event_notify_new_topics := NEW.notify_new_topics;
        event_digest_mode := NEW.digest_mode; event_revision := NEW.revision;
    ELSIF TG_OP = 'UPDATE' THEN
        IF ROW(OLD.level, OLD.notify_mentions, OLD.notify_replies, OLD.notify_new_topics, OLD.digest_mode, OLD.revision)
           IS NOT DISTINCT FROM
           ROW(NEW.level, NEW.notify_mentions, NEW.notify_replies, NEW.notify_new_topics, NEW.digest_mode, NEW.revision)
        THEN RETURN NEW; END IF;
        event_tenant := NEW.tenant_id; event_target := NEW.category_id;
        event_user := NEW.user_id; old_level := OLD.level; new_level := NEW.level;
        event_notify_mentions := NEW.notify_mentions;
        event_notify_replies := NEW.notify_replies;
        event_notify_new_topics := NEW.notify_new_topics;
        event_digest_mode := NEW.digest_mode; event_revision := NEW.revision;
    ELSE
        event_tenant := OLD.tenant_id; event_target := OLD.category_id;
        event_user := OLD.user_id; old_level := OLD.level; new_level := 'normal';
        event_notify_mentions := TRUE; event_notify_replies := FALSE;
        event_notify_new_topics := FALSE; event_digest_mode := 'disabled';
        event_revision := OLD.revision + 1;
    END IF;
    PERFORM forum_append_domain_event(
        event_tenant, 'category', event_target, 'forum.subscription.changed.v1', event_user,
        jsonb_build_object(
            'target_type', 'category', 'target_id', event_target, 'user_id', event_user,
            'previous_level', old_level, 'level', new_level,
            'notify_mentions', event_notify_mentions, 'notify_replies', event_notify_replies,
            'notify_new_topics', event_notify_new_topics, 'digest_mode', event_digest_mode,
            'revision', event_revision
        )
    );
    IF TG_OP = 'DELETE' THEN RETURN OLD; END IF;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE OR REPLACE FUNCTION forum_emit_topic_subscription_event()
RETURNS trigger AS $$
DECLARE
    event_tenant UUID; event_target UUID; event_user UUID;
    old_level TEXT; new_level TEXT; event_digest_mode TEXT;
    event_notify_mentions BOOLEAN; event_notify_replies BOOLEAN;
    event_notify_new_topics BOOLEAN; event_revision BIGINT;
BEGIN
    IF TG_OP = 'INSERT' THEN
        event_tenant := NEW.tenant_id; event_target := NEW.topic_id;
        event_user := NEW.user_id; old_level := 'normal'; new_level := NEW.level;
        event_notify_mentions := NEW.notify_mentions;
        event_notify_replies := NEW.notify_replies;
        event_notify_new_topics := NEW.notify_new_topics;
        event_digest_mode := NEW.digest_mode; event_revision := NEW.revision;
    ELSIF TG_OP = 'UPDATE' THEN
        IF ROW(OLD.level, OLD.notify_mentions, OLD.notify_replies, OLD.notify_new_topics, OLD.digest_mode, OLD.revision)
           IS NOT DISTINCT FROM
           ROW(NEW.level, NEW.notify_mentions, NEW.notify_replies, NEW.notify_new_topics, NEW.digest_mode, NEW.revision)
        THEN RETURN NEW; END IF;
        event_tenant := NEW.tenant_id; event_target := NEW.topic_id;
        event_user := NEW.user_id; old_level := OLD.level; new_level := NEW.level;
        event_notify_mentions := NEW.notify_mentions;
        event_notify_replies := NEW.notify_replies;
        event_notify_new_topics := NEW.notify_new_topics;
        event_digest_mode := NEW.digest_mode; event_revision := NEW.revision;
    ELSE
        event_tenant := OLD.tenant_id; event_target := OLD.topic_id;
        event_user := OLD.user_id; old_level := OLD.level; new_level := 'normal';
        event_notify_mentions := TRUE; event_notify_replies := FALSE;
        event_notify_new_topics := FALSE; event_digest_mode := 'disabled';
        event_revision := OLD.revision + 1;
    END IF;
    PERFORM forum_append_domain_event(
        event_tenant, 'topic', event_target, 'forum.subscription.changed.v1', event_user,
        jsonb_build_object(
            'target_type', 'topic', 'target_id', event_target, 'user_id', event_user,
            'previous_level', old_level, 'level', new_level,
            'notify_mentions', event_notify_mentions, 'notify_replies', event_notify_replies,
            'notify_new_topics', event_notify_new_topics, 'digest_mode', event_digest_mode,
            'revision', event_revision
        )
    );
    IF TG_OP = 'DELETE' THEN RETURN OLD; END IF;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS forum_80_category_subscription_events ON forum_category_subscriptions;
CREATE TRIGGER forum_80_category_subscription_events
AFTER INSERT OR UPDATE OR DELETE ON forum_category_subscriptions
FOR EACH ROW EXECUTE FUNCTION forum_emit_category_subscription_event();

DROP TRIGGER IF EXISTS forum_80_topic_subscription_events ON forum_topic_subscriptions;
CREATE TRIGGER forum_80_topic_subscription_events
AFTER INSERT OR UPDATE OR DELETE ON forum_topic_subscriptions
FOR EACH ROW EXECUTE FUNCTION forum_emit_topic_subscription_event();
"#,
        )
        .await?;
    Ok(())
}
