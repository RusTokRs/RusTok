use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::ConnectionTrait;

pub(super) async fn apply(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .get_connection()
        .execute_unprepared(
            r#"
ALTER TABLE forum_category_subscriptions
    ADD COLUMN IF NOT EXISTS level VARCHAR(32) NOT NULL DEFAULT 'watching',
    ADD COLUMN IF NOT EXISTS notify_mentions BOOLEAN NOT NULL DEFAULT TRUE,
    ADD COLUMN IF NOT EXISTS notify_replies BOOLEAN NOT NULL DEFAULT TRUE,
    ADD COLUMN IF NOT EXISTS notify_new_topics BOOLEAN NOT NULL DEFAULT TRUE,
    ADD COLUMN IF NOT EXISTS digest_mode VARCHAR(32) NOT NULL DEFAULT 'immediate',
    ADD COLUMN IF NOT EXISTS last_notified_at TIMESTAMPTZ NULL,
    ADD COLUMN IF NOT EXISTS revision BIGINT NOT NULL DEFAULT 1,
    ADD COLUMN IF NOT EXISTS updated_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP;

ALTER TABLE forum_topic_subscriptions
    ADD COLUMN IF NOT EXISTS level VARCHAR(32) NOT NULL DEFAULT 'watching',
    ADD COLUMN IF NOT EXISTS notify_mentions BOOLEAN NOT NULL DEFAULT TRUE,
    ADD COLUMN IF NOT EXISTS notify_replies BOOLEAN NOT NULL DEFAULT TRUE,
    ADD COLUMN IF NOT EXISTS notify_new_topics BOOLEAN NOT NULL DEFAULT TRUE,
    ADD COLUMN IF NOT EXISTS digest_mode VARCHAR(32) NOT NULL DEFAULT 'immediate',
    ADD COLUMN IF NOT EXISTS last_notified_at TIMESTAMPTZ NULL,
    ADD COLUMN IF NOT EXISTS revision BIGINT NOT NULL DEFAULT 1,
    ADD COLUMN IF NOT EXISTS updated_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP;

DO $$ BEGIN
    ALTER TABLE forum_category_subscriptions ADD CONSTRAINT chk_forum_category_subscription_level
        CHECK (level IN ('watching', 'tracking', 'normal', 'muted'));
EXCEPTION WHEN duplicate_object THEN NULL; END $$;
DO $$ BEGIN
    ALTER TABLE forum_topic_subscriptions ADD CONSTRAINT chk_forum_topic_subscription_level
        CHECK (level IN ('watching', 'tracking', 'normal', 'muted'));
EXCEPTION WHEN duplicate_object THEN NULL; END $$;
DO $$ BEGIN
    ALTER TABLE forum_category_subscriptions ADD CONSTRAINT chk_forum_category_subscription_digest
        CHECK (digest_mode IN ('immediate', 'daily', 'weekly', 'disabled'));
EXCEPTION WHEN duplicate_object THEN NULL; END $$;
DO $$ BEGIN
    ALTER TABLE forum_topic_subscriptions ADD CONSTRAINT chk_forum_topic_subscription_digest
        CHECK (digest_mode IN ('immediate', 'daily', 'weekly', 'disabled'));
EXCEPTION WHEN duplicate_object THEN NULL; END $$;
DO $$ BEGIN
    ALTER TABLE forum_category_subscriptions ADD CONSTRAINT chk_forum_category_subscription_muted
        CHECK (level <> 'muted' OR (NOT notify_mentions AND NOT notify_replies AND NOT notify_new_topics AND digest_mode = 'disabled'));
EXCEPTION WHEN duplicate_object THEN NULL; END $$;
DO $$ BEGIN
    ALTER TABLE forum_topic_subscriptions ADD CONSTRAINT chk_forum_topic_subscription_muted
        CHECK (level <> 'muted' OR (NOT notify_mentions AND NOT notify_replies AND NOT notify_new_topics AND digest_mode = 'disabled'));
EXCEPTION WHEN duplicate_object THEN NULL; END $$;
DO $$ BEGIN
    ALTER TABLE forum_category_subscriptions ADD CONSTRAINT chk_forum_category_subscription_revision
        CHECK (revision > 0);
EXCEPTION WHEN duplicate_object THEN NULL; END $$;
DO $$ BEGIN
    ALTER TABLE forum_topic_subscriptions ADD CONSTRAINT chk_forum_topic_subscription_revision
        CHECK (revision > 0);
EXCEPTION WHEN duplicate_object THEN NULL; END $$;

CREATE TABLE IF NOT EXISTS forum_subscription_policies (
    tenant_id UUID PRIMARY KEY,
    auto_subscribe_topic_authors BOOLEAN NOT NULL DEFAULT TRUE,
    topic_author_level VARCHAR(32) NOT NULL DEFAULT 'watching',
    auto_subscribe_reply_participants BOOLEAN NOT NULL DEFAULT TRUE,
    reply_participant_level VARCHAR(32) NOT NULL DEFAULT 'tracking',
    revision BIGINT NOT NULL DEFAULT 1,
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    CONSTRAINT chk_forum_subscription_policy_topic_level
        CHECK (topic_author_level IN ('watching', 'tracking', 'normal')),
    CONSTRAINT chk_forum_subscription_policy_reply_level
        CHECK (reply_participant_level IN ('watching', 'tracking', 'normal')),
    CONSTRAINT chk_forum_subscription_policy_revision CHECK (revision > 0)
);

CREATE INDEX IF NOT EXISTS idx_forum_category_subscriptions_user_level
    ON forum_category_subscriptions (tenant_id, user_id, level, category_id);
CREATE INDEX IF NOT EXISTS idx_forum_topic_subscriptions_user_level
    ON forum_topic_subscriptions (tenant_id, user_id, level, topic_id);
"#,
        )
        .await?;
    Ok(())
}
