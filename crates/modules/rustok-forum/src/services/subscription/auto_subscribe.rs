//! Rust-owned auto-subscribe side effects for topic authors and reply
//! participants.
//!
//! This module replaces the removed `forum_70_*` database triggers
//! (migration `m20261102_000040`). Subscription creation is a domain rule,
//! so it runs in the caller's write transaction next to the topic/reply
//! write it belongs to, using the tenant subscription policy row.
//!
//! The insert is strictly `ON CONFLICT DO NOTHING`: an existing explicit
//! row is never overwritten. In particular a `Normal` row written by an
//! explicit unsubscribe ("clear") survives later replies and moderation
//! approvals, and a `Muted` row stays muted. Only a user who has no row at
//! all receives the policy default. Operator paths (imports, forks,
//! splits, tombstone restores) deliberately do not call these helpers.

use chrono::Utc;
use sea_orm::{
    ActiveValue::Set, DatabaseTransaction, EntityTrait, sea_query::OnConflict,
};
use uuid::Uuid;

use crate::entities::{forum_subscription_policy, forum_topic_subscription};
use crate::error::ForumResult;
use crate::subscription::ForumSubscriptionLevel;

use super::helpers::INITIAL_REVISION;

/// Subscribes a topic author according to the tenant subscription policy.
/// Called from the topic-create owner transaction after the topic insert.
pub(crate) async fn auto_subscribe_topic_author_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    topic_id: Uuid,
    author_id: Option<Uuid>,
) -> ForumResult<()> {
    let Some(author_id) = author_id.filter(|id| !id.is_nil()) else {
        return Ok(());
    };
    let policy = forum_subscription_policy::Entity::find_by_id(tenant_id)
        .one(txn)
        .await?;
    let (enabled, level) = match &policy {
        Some(policy) => (
            policy.auto_subscribe_topic_authors,
            policy.topic_author_level,
        ),
        None => (true, ForumSubscriptionLevel::Watching),
    };
    if !enabled {
        return Ok(());
    }
    insert_implicit_topic_subscription_in_tx(txn, tenant_id, topic_id, author_id, level, true)
        .await
}

/// Subscribes an author whose reply is (or just became) approved, according
/// to the tenant subscription policy. Called from the reply-create owner
/// transaction and from the moderation approval transition.
pub(crate) async fn auto_subscribe_reply_participant_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    topic_id: Uuid,
    author_id: Option<Uuid>,
) -> ForumResult<()> {
    let Some(author_id) = author_id.filter(|id| !id.is_nil()) else {
        return Ok(());
    };
    let policy = forum_subscription_policy::Entity::find_by_id(tenant_id)
        .one(txn)
        .await?;
    let (enabled, level) = match &policy {
        Some(policy) => (
            policy.auto_subscribe_reply_participants,
            policy.reply_participant_level,
        ),
        None => (true, ForumSubscriptionLevel::Tracking),
    };
    if !enabled {
        return Ok(());
    }
    insert_implicit_topic_subscription_in_tx(txn, tenant_id, topic_id, author_id, level, false)
        .await
}

async fn insert_implicit_topic_subscription_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    topic_id: Uuid,
    user_id: Uuid,
    level: ForumSubscriptionLevel,
    is_topic_author: bool,
) -> ForumResult<()> {
    let preferences = level.default_preferences();
    let now = Utc::now();
    forum_topic_subscription::Entity::insert(forum_topic_subscription::ActiveModel {
        topic_id: Set(topic_id),
        user_id: Set(user_id),
        tenant_id: Set(tenant_id),
        level: Set(level),
        notify_mentions: Set(preferences.notify_mentions),
        notify_replies: Set(preferences.notify_replies),
        // Topic authors follow new topics in their own thread; participants
        // only receive the level defaults for an existing thread.
        notify_new_topics: Set(if is_topic_author {
            preferences.notify_new_topics
        } else {
            false
        }),
        digest_mode: Set(preferences.digest_mode),
        last_notified_at: Set(None),
        revision: Set(INITIAL_REVISION),
        created_at: Set(now.into()),
        updated_at: Set(now.into()),
    })
    .on_conflict(
        OnConflict::columns([
            forum_topic_subscription::Column::TenantId,
            forum_topic_subscription::Column::TopicId,
            forum_topic_subscription::Column::UserId,
        ])
        .do_nothing()
        .to_owned(),
    )
    .exec_without_returning(txn)
    .await?;
    Ok(())
}
