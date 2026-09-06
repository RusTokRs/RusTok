use rustok_api::{Action, Resource};
use rustok_core::{PermissionScope, SecurityContext};
use uuid::Uuid;

use crate::dto::{
    ForumSubscriptionPolicyResponse, ForumSubscriptionResponse, ForumSubscriptionTargetType,
    UpdateForumSubscriptionInput, UpdateForumSubscriptionPolicyInput,
};
use crate::entities::{
    forum_category_subscription, forum_subscription_policy, forum_topic_subscription,
};
use crate::error::{ForumError, ForumResult};
use crate::subscription::{ForumSubscriptionLevel, ForumSubscriptionPreferences};

pub(super) const INITIAL_REVISION: i64 = 1;

pub(super) fn resolve_preferences(
    input: &UpdateForumSubscriptionInput,
) -> ForumSubscriptionPreferences {
    ForumSubscriptionPreferences::resolve(
        input.level,
        input.notify_mentions,
        input.notify_replies,
        input.notify_new_topics,
        input.digest_mode,
    )
}

pub(super) fn ensure_revision_update(rows_affected: u64) -> ForumResult<()> {
    if rows_affected != 1 {
        return Err(ForumError::Validation(
            "Forum subscription revision conflict".to_string(),
        ));
    }
    Ok(())
}

pub(super) fn validate_expected_revision(expected: Option<i64>, actual: i64) -> ForumResult<()> {
    if expected.is_some_and(|expected| expected != actual) {
        return Err(ForumError::Validation(format!(
            "Forum subscription revision conflict: expected {}, actual {actual}",
            expected.unwrap_or_default()
        )));
    }
    Ok(())
}

pub(super) fn validate_new_revision(expected: Option<i64>) -> ForumResult<()> {
    if expected.is_some_and(|revision| revision != 0) {
        return Err(ForumError::Validation(
            "A new forum subscription or policy requires expected_revision 0".to_string(),
        ));
    }
    Ok(())
}

pub(super) fn validate_policy(input: &UpdateForumSubscriptionPolicyInput) -> ForumResult<()> {
    if input.auto_subscribe_topic_authors
        && input.topic_author_level == ForumSubscriptionLevel::Muted
    {
        return Err(ForumError::Validation(
            "Topic author auto-subscribe level cannot be muted".to_string(),
        ));
    }
    if input.auto_subscribe_reply_participants
        && input.reply_participant_level == ForumSubscriptionLevel::Muted
    {
        return Err(ForumError::Validation(
            "Reply participant auto-subscribe level cannot be muted".to_string(),
        ));
    }
    Ok(())
}

pub(super) fn enforce_policy_scope(security: &SecurityContext) -> ForumResult<()> {
    let topic_scope = security.get_scope(Resource::ForumTopics, Action::Manage);
    let category_scope = security.get_scope(Resource::ForumCategories, Action::Manage);
    if matches!(topic_scope, PermissionScope::None)
        && matches!(category_scope, PermissionScope::None)
    {
        return Err(ForumError::forbidden(
            "Forum subscription policy management permission is required",
        ));
    }
    Ok(())
}

pub(super) fn require_authenticated_user(security: &SecurityContext) -> ForumResult<Uuid> {
    security.user_id.ok_or_else(|| {
        ForumError::forbidden("Authenticated user context is required for subscriptions")
    })
}

pub(super) fn implicit_response(
    tenant_id: Uuid,
    target_type: ForumSubscriptionTargetType,
    target_id: Uuid,
    user_id: Uuid,
) -> ForumSubscriptionResponse {
    let level = ForumSubscriptionLevel::Normal;
    let preferences = level.default_preferences();
    ForumSubscriptionResponse {
        tenant_id,
        target_type,
        target_id,
        user_id,
        level,
        notify_mentions: preferences.notify_mentions,
        notify_replies: preferences.notify_replies,
        notify_new_topics: preferences.notify_new_topics,
        digest_mode: preferences.digest_mode,
        last_notified_at: None,
        revision: 0,
        explicit: false,
        created_at: None,
        updated_at: None,
    }
}

pub(super) fn category_response(
    model: forum_category_subscription::Model,
) -> ForumSubscriptionResponse {
    ForumSubscriptionResponse {
        tenant_id: model.tenant_id,
        target_type: ForumSubscriptionTargetType::Category,
        target_id: model.category_id,
        user_id: model.user_id,
        level: model.level,
        notify_mentions: model.notify_mentions,
        notify_replies: model.notify_replies,
        notify_new_topics: model.notify_new_topics,
        digest_mode: model.digest_mode,
        last_notified_at: model.last_notified_at.map(|value| value.to_rfc3339()),
        revision: model.revision,
        explicit: true,
        created_at: Some(model.created_at.to_rfc3339()),
        updated_at: Some(model.updated_at.to_rfc3339()),
    }
}

pub(super) fn topic_response(model: forum_topic_subscription::Model) -> ForumSubscriptionResponse {
    ForumSubscriptionResponse {
        tenant_id: model.tenant_id,
        target_type: ForumSubscriptionTargetType::Topic,
        target_id: model.topic_id,
        user_id: model.user_id,
        level: model.level,
        notify_mentions: model.notify_mentions,
        notify_replies: model.notify_replies,
        notify_new_topics: model.notify_new_topics,
        digest_mode: model.digest_mode,
        last_notified_at: model.last_notified_at.map(|value| value.to_rfc3339()),
        revision: model.revision,
        explicit: true,
        created_at: Some(model.created_at.to_rfc3339()),
        updated_at: Some(model.updated_at.to_rfc3339()),
    }
}

pub(super) fn default_policy(tenant_id: Uuid) -> ForumSubscriptionPolicyResponse {
    ForumSubscriptionPolicyResponse {
        tenant_id,
        auto_subscribe_topic_authors: true,
        topic_author_level: ForumSubscriptionLevel::Watching,
        auto_subscribe_reply_participants: true,
        reply_participant_level: ForumSubscriptionLevel::Tracking,
        revision: 0,
        explicit: false,
        created_at: None,
        updated_at: None,
    }
}

pub(super) fn policy_response(
    model: forum_subscription_policy::Model,
) -> ForumSubscriptionPolicyResponse {
    ForumSubscriptionPolicyResponse {
        tenant_id: model.tenant_id,
        auto_subscribe_topic_authors: model.auto_subscribe_topic_authors,
        topic_author_level: model.topic_author_level,
        auto_subscribe_reply_participants: model.auto_subscribe_reply_participants,
        reply_participant_level: model.reply_participant_level,
        revision: model.revision,
        explicit: true,
        created_at: Some(model.created_at.to_rfc3339()),
        updated_at: Some(model.updated_at.to_rfc3339()),
    }
}
