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

-- Auto-subscribe is a Rust owner rule, not a database trigger: the topic,
-- reply and moderation services create the subscription row inside their
-- write transaction (see `services::subscription::auto_subscribe`).
-- Trigger-based auto-subscribe was removed because it upgraded an
-- explicit `normal` (cleared) row back to a subscribed level without the
-- user's consent and was invisible to the Rust owner contract.
-- Migration `m20261102_000040` drops the triggers and functions on
-- databases that already applied the original version of this migration.
"#,
        )
        .await?;
    Ok(())
}
