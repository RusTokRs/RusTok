//! Visibility rule for content held by pre-moderation.
//!
//! A topic or reply in `Pending` is visible only to its author and to viewers that hold the
//! moderation scope `All` for the resource. Every owner read path applies this rule before a row
//! leaves the owner: list queries through [`reply_pending_condition`], and single-item and
//! audience reads through [`can_see_pending`]. Storefront reads already return only `Open` topics
//! and `Approved` replies, so they never reach this rule with pending content.
//!
//! See `DECISIONS/2026-10-09-forum-topic-pre-moderation.md`.

use rustok_api::{Action, Resource};
use rustok_core::{PermissionScope, SecurityContext};
use sea_orm::{ColumnTrait, Condition};
use uuid::Uuid;

use crate::entities::{forum_reply, forum_topic};
use crate::state_machine::{ReplyStatus, TopicStatus};

/// Returns `true` when `security` may read content authored by `author_id` while it is pending.
pub(crate) fn can_see_pending(
    security: &SecurityContext,
    resource: Resource,
    author_id: Option<Uuid>,
) -> bool {
    if security.user_id.is_some() && security.user_id == author_id {
        return true;
    }
    security.get_scope(resource, Action::Moderate) == PermissionScope::All
}

/// Row filter for topic lists: excludes other authors' pending topics unless the caller is a
/// moderator. Applied to every list, including lists that request an explicit status.
pub(crate) fn topic_pending_condition(security: &SecurityContext) -> Condition {
    if security.get_scope(Resource::ForumTopics, Action::Moderate) == PermissionScope::All {
        return Condition::all();
    }
    let not_pending = Condition::any().add(forum_topic::Column::Status.ne(TopicStatus::Pending));
    match security.user_id {
        Some(user_id) => not_pending.add(forum_topic::Column::AuthorId.eq(user_id)),
        None => not_pending,
    }
}

/// Row filter for reply lists: excludes other authors' pending replies unless the caller is a
/// moderator. Applied to every list, including lists that request explicit statuses.
pub(crate) fn reply_pending_condition(security: &SecurityContext) -> Condition {
    if security.get_scope(Resource::ForumReplies, Action::Moderate) == PermissionScope::All {
        return Condition::all();
    }
    let not_pending = Condition::any().add(forum_reply::Column::Status.ne(ReplyStatus::Pending));
    match security.user_id {
        Some(user_id) => not_pending.add(forum_reply::Column::AuthorId.eq(user_id)),
        None => not_pending,
    }
}
