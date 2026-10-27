//! Visibility rule for content held by pre-moderation.
//!
//! A topic or reply in `Pending` is visible only to its author and to viewers that hold the
//! moderation scope `All` for the resource. Every owner read path applies this rule before a row
//! leaves the owner: list queries through [`reply_owner_read_condition`] and [`topic_owner_read_condition`],
//! and single-item and audience reads through [`can_see_pending`]. Storefront reads already return
//! only `Open` topics and `Approved` replies, so they never reach this rule with pending content.
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

/// Returns `true` when `security` may read content whose status is not public
/// (non-`Open`/non-`Closed` topics and non-`Approved` replies, e.g. `Pending`,
/// `Archived`, `Hidden`, `Rejected`, `Flagged`, `Deleted`).
///
/// Only the author and viewers holding the moderation scope `All` may read
/// non-public content. Owner list and single-item read paths apply this rule so
/// moderated-away and soft-deleted rows never leave the owner for arbitrary
/// permission holders.
pub(crate) fn can_read_non_public(
    security: &SecurityContext,
    resource: Resource,
    author_id: Option<Uuid>,
) -> bool {
    if security.user_id.is_some() && security.user_id == author_id {
        return true;
    }
    security.get_scope(resource, Action::Moderate) == PermissionScope::All
}

/// Row filter for owner topic lists: hides other authors' `Pending` and
/// `Archived` topics from every caller that is not a moderator. Applied to
/// every owner list, including lists that request an explicit status, so
/// archived tombstones (deleted or merged) cannot be enumerated by ordinary
/// members.
pub(crate) fn topic_owner_read_condition(security: &SecurityContext) -> Condition {
    if security.get_scope(Resource::ForumTopics, Action::Moderate) == PermissionScope::All {
        return Condition::all();
    }
    let public = Condition::any().add(
        forum_topic::Column::Status.is_not_in(vec![TopicStatus::Pending, TopicStatus::Archived]),
    );
    match security.user_id {
        Some(user_id) => public.add(forum_topic::Column::AuthorId.eq(user_id)),
        None => public,
    }
}

/// Row filter for owner reply lists: hides other authors' non-`Approved`
/// replies (`Pending`, `Rejected`, `Hidden`, `Flagged`, `Deleted`) from every
/// caller that is not a moderator. Applied to every owner list, including lists
/// that request explicit statuses, so moderated and deleted bodies cannot be
/// enumerated by ordinary members.
pub(crate) fn reply_owner_read_condition(security: &SecurityContext) -> Condition {
    if security.get_scope(Resource::ForumReplies, Action::Moderate) == PermissionScope::All {
        return Condition::all();
    }
    let public = Condition::any().add(forum_reply::Column::Status.eq(ReplyStatus::Approved));
    match security.user_id {
        Some(user_id) => public.add(forum_reply::Column::AuthorId.eq(user_id)),
        None => public,
    }
}
