use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::ConnectionTrait;

pub(super) async fn apply(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .get_connection()
        .execute_unprepared(
            r#"
CREATE OR REPLACE FUNCTION forum_validate_subscription_revision()
RETURNS trigger AS $$
BEGIN
    IF NEW.revision <> OLD.revision + 1 THEN
        RAISE EXCEPTION 'forum subscription revision must increment by one';
    END IF;
    NEW.updated_at := CURRENT_TIMESTAMP;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS forum_60_category_subscription_revision ON forum_category_subscriptions;
CREATE TRIGGER forum_60_category_subscription_revision
BEFORE UPDATE ON forum_category_subscriptions
FOR EACH ROW EXECUTE FUNCTION forum_validate_subscription_revision();

DROP TRIGGER IF EXISTS forum_60_topic_subscription_revision ON forum_topic_subscriptions;
CREATE TRIGGER forum_60_topic_subscription_revision
BEFORE UPDATE ON forum_topic_subscriptions
FOR EACH ROW EXECUTE FUNCTION forum_validate_subscription_revision();

CREATE OR REPLACE FUNCTION forum_validate_subscription_policy_revision()
RETURNS trigger AS $$
BEGIN
    IF NEW.revision <> OLD.revision + 1 THEN
        RAISE EXCEPTION 'forum subscription policy revision must increment by one';
    END IF;
    NEW.updated_at := CURRENT_TIMESTAMP;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS forum_60_subscription_policy_revision ON forum_subscription_policies;
CREATE TRIGGER forum_60_subscription_policy_revision
BEFORE UPDATE ON forum_subscription_policies
FOR EACH ROW EXECUTE FUNCTION forum_validate_subscription_policy_revision();

CREATE OR REPLACE FUNCTION forum_auto_subscribe_topic_author()
RETURNS trigger AS $$
DECLARE selected_level TEXT;
BEGIN
    IF NEW.author_id IS NULL OR NOT COALESCE(
        (SELECT auto_subscribe_topic_authors FROM forum_subscription_policies WHERE tenant_id = NEW.tenant_id),
        TRUE
    ) THEN RETURN NEW; END IF;
    selected_level := COALESCE(
        (SELECT topic_author_level FROM forum_subscription_policies WHERE tenant_id = NEW.tenant_id),
        'watching'
    );
    INSERT INTO forum_topic_subscriptions (
        topic_id, user_id, tenant_id, level,
        notify_mentions, notify_replies, notify_new_topics,
        digest_mode, revision, created_at, updated_at
    ) VALUES (
        NEW.id, NEW.author_id, NEW.tenant_id, selected_level,
        TRUE, selected_level = 'watching', selected_level = 'watching',
        CASE WHEN selected_level = 'watching' THEN 'immediate' ELSE 'disabled' END,
        1, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP
    ) ON CONFLICT (tenant_id, topic_id, user_id) DO UPDATE SET
        level = EXCLUDED.level,
        notify_mentions = EXCLUDED.notify_mentions,
        notify_replies = EXCLUDED.notify_replies,
        notify_new_topics = EXCLUDED.notify_new_topics,
        digest_mode = EXCLUDED.digest_mode,
        revision = forum_topic_subscriptions.revision + 1,
        updated_at = CURRENT_TIMESTAMP
    WHERE forum_topic_subscriptions.level = 'normal';
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE OR REPLACE FUNCTION forum_auto_subscribe_reply_participant()
RETURNS trigger AS $$
DECLARE selected_level TEXT;
BEGIN
    IF NEW.author_id IS NULL OR NEW.status <> 'approved' THEN RETURN NEW; END IF;
    IF TG_OP = 'UPDATE' THEN
        IF OLD.status = 'approved' THEN RETURN NEW; END IF;
    END IF;
    IF NOT COALESCE(
        (SELECT auto_subscribe_reply_participants FROM forum_subscription_policies WHERE tenant_id = NEW.tenant_id),
        TRUE
    ) THEN RETURN NEW; END IF;
    selected_level := COALESCE(
        (SELECT reply_participant_level FROM forum_subscription_policies WHERE tenant_id = NEW.tenant_id),
        'tracking'
    );
    INSERT INTO forum_topic_subscriptions (
        topic_id, user_id, tenant_id, level,
        notify_mentions, notify_replies, notify_new_topics,
        digest_mode, revision, created_at, updated_at
    ) VALUES (
        NEW.topic_id, NEW.author_id, NEW.tenant_id, selected_level,
        TRUE, selected_level = 'watching', FALSE,
        CASE WHEN selected_level = 'watching' THEN 'immediate' ELSE 'disabled' END,
        1, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP
    ) ON CONFLICT (tenant_id, topic_id, user_id) DO UPDATE SET
        level = EXCLUDED.level,
        notify_mentions = EXCLUDED.notify_mentions,
        notify_replies = EXCLUDED.notify_replies,
        notify_new_topics = EXCLUDED.notify_new_topics,
        digest_mode = EXCLUDED.digest_mode,
        revision = forum_topic_subscriptions.revision + 1,
        updated_at = CURRENT_TIMESTAMP
    WHERE forum_topic_subscriptions.level = 'normal';
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS forum_70_auto_subscribe_topic_author ON forum_topics;
CREATE TRIGGER forum_70_auto_subscribe_topic_author
AFTER INSERT ON forum_topics
FOR EACH ROW EXECUTE FUNCTION forum_auto_subscribe_topic_author();

DROP TRIGGER IF EXISTS forum_70_auto_subscribe_reply_participant ON forum_replies;
CREATE TRIGGER forum_70_auto_subscribe_reply_participant
AFTER INSERT OR UPDATE OF status ON forum_replies
FOR EACH ROW EXECUTE FUNCTION forum_auto_subscribe_reply_participant();
"#,
        )
        .await?;
    Ok(())
}
