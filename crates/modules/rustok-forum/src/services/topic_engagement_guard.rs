//! Shared topic engagement gate for author-facing writes.
//!
//! Ideal lock/closed matrix:
//! - `archived` ⇒ no engagement writes
//! - `pending` ⇒ no votes, new replies, or reply restores (unpublished content)
//! - `locked` ⇒ no author topic/reply/vote writes; Manage/Moderate-All may still write
//! - `closed` ⇒ no new replies (including restore of a public reply); content edits remain
//!   subject to the author edit window (or a moderator)
//!
//! Callers lock the topic row first, then apply this policy.

use rustok_api::{Action, Resource};
use rustok_core::{PermissionScope, SecurityContext};

use crate::entities::forum_topic;
use crate::error::{ForumError, ForumResult};
use crate::state_machine::TopicStatus;

/// Kind of author-facing write that must consult the parent topic's lifecycle.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum TopicEngagementKind {
    Vote,
    CreateReply,
    UpdateReply,
    RestoreReply,
    UpdateTopic,
}

pub(crate) fn enforce_topic_engagement(
    topic: &forum_topic::Model,
    kind: TopicEngagementKind,
    security: &SecurityContext,
    resource: Resource,
) -> ForumResult<()> {
    match topic.status {
        TopicStatus::Archived => return Err(ForumError::TopicArchived),
        TopicStatus::Pending => match kind {
            TopicEngagementKind::Vote
            | TopicEngagementKind::CreateReply
            | TopicEngagementKind::RestoreReply => {
                return Err(ForumError::TopicAwaitingModeration);
            }
            TopicEngagementKind::UpdateReply | TopicEngagementKind::UpdateTopic => {}
        },
        TopicStatus::Closed => match kind {
            TopicEngagementKind::CreateReply | TopicEngagementKind::RestoreReply => {
                return Err(ForumError::TopicClosed);
            }
            TopicEngagementKind::Vote
            | TopicEngagementKind::UpdateReply
            | TopicEngagementKind::UpdateTopic => {}
        },
        TopicStatus::Open => {}
    }

    if topic.is_locked && !may_write_locked_topic(security, resource) {
        return Err(ForumError::TopicLocked);
    }
    Ok(())
}

fn may_write_locked_topic(security: &SecurityContext, resource: Resource) -> bool {
    security.get_scope(resource, Action::Moderate) == PermissionScope::All
        || security.get_scope(resource, Action::Manage) == PermissionScope::All
}

#[cfg(test)]
mod tests {
    use super::*;
    use rustok_api::Permission;
    use rustok_core::UserRole;
    use uuid::Uuid;

    fn sample_topic(status: TopicStatus, is_locked: bool) -> forum_topic::Model {
        let now = chrono::Utc::now().fixed_offset();
        forum_topic::Model {
            id: Uuid::nil(),
            tenant_id: Uuid::nil(),
            category_id: Uuid::nil(),
            author_id: None,
            status,
            metadata: serde_json::json!({}),
            is_pinned: false,
            is_locked,
            reply_count: 0,
            created_at: now,
            updated_at: now,
            last_reply_at: None,
        }
    }

    fn author() -> SecurityContext {
        SecurityContext::from_permissions(
            UserRole::Customer,
            Some(Uuid::new_v4()),
            [
                Permission::FORUM_TOPICS_UPDATE,
                Permission::FORUM_REPLIES_CREATE,
                Permission::FORUM_REPLIES_UPDATE,
                Permission::FORUM_TOPICS_READ,
                Permission::FORUM_REPLIES_READ,
            ],
        )
    }

    fn moderator() -> SecurityContext {
        SecurityContext::from_permissions(
            UserRole::Manager,
            Some(Uuid::new_v4()),
            [
                Permission::FORUM_TOPICS_MODERATE,
                Permission::FORUM_TOPICS_MANAGE,
                Permission::FORUM_REPLIES_MODERATE,
                Permission::FORUM_REPLIES_MANAGE,
            ],
        )
    }

    fn assert_err(
        kind: TopicEngagementKind,
        topic: &forum_topic::Model,
        security: &SecurityContext,
    ) {
        let resource = match kind {
            TopicEngagementKind::Vote | TopicEngagementKind::UpdateTopic => Resource::ForumTopics,
            TopicEngagementKind::CreateReply
            | TopicEngagementKind::UpdateReply
            | TopicEngagementKind::RestoreReply => Resource::ForumReplies,
        };
        enforce_topic_engagement(topic, kind, security, resource).expect_err("expected rejection");
    }

    #[test]
    fn archived_rejects_every_engagement_kind() {
        let topic = sample_topic(TopicStatus::Archived, true);
        let author = author();
        let moderator = moderator();
        for kind in [
            TopicEngagementKind::Vote,
            TopicEngagementKind::CreateReply,
            TopicEngagementKind::UpdateReply,
            TopicEngagementKind::RestoreReply,
            TopicEngagementKind::UpdateTopic,
        ] {
            assert_err(kind, &topic, &author);
            assert_err(kind, &topic, &moderator);
        }
    }

    #[test]
    fn pending_rejects_votes_new_replies_and_restore() {
        let topic = sample_topic(TopicStatus::Pending, false);
        let author = author();
        assert_err(TopicEngagementKind::Vote, &topic, &author);
        assert_err(TopicEngagementKind::CreateReply, &topic, &author);
        assert_err(TopicEngagementKind::RestoreReply, &topic, &author);
        enforce_topic_engagement(
            &topic,
            TopicEngagementKind::UpdateTopic,
            &author,
            Resource::ForumTopics,
        )
        .expect("author may still edit a pending topic");
        enforce_topic_engagement(
            &topic,
            TopicEngagementKind::UpdateReply,
            &author,
            Resource::ForumReplies,
        )
        .expect("author may still edit a pending reply");
    }

    #[test]
    fn closed_rejects_new_replies_and_restore_but_allows_votes_and_edits() {
        let topic = sample_topic(TopicStatus::Closed, false);
        let author = author();
        assert_err(TopicEngagementKind::CreateReply, &topic, &author);
        assert_err(TopicEngagementKind::RestoreReply, &topic, &author);
        enforce_topic_engagement(
            &topic,
            TopicEngagementKind::Vote,
            &author,
            Resource::ForumTopics,
        )
        .expect("closed topics still accept votes");
        enforce_topic_engagement(
            &topic,
            TopicEngagementKind::UpdateReply,
            &author,
            Resource::ForumReplies,
        )
        .expect("closed topics still accept edits inside the edit window");
        enforce_topic_engagement(
            &topic,
            TopicEngagementKind::UpdateTopic,
            &author,
            Resource::ForumTopics,
        )
        .expect("closed topics still accept topic edits inside the edit window");
    }

    #[test]
    fn locked_blocks_authors_and_allows_moderators() {
        let topic = sample_topic(TopicStatus::Open, true);
        let author = author();
        let moderator = moderator();
        for kind in [
            TopicEngagementKind::Vote,
            TopicEngagementKind::CreateReply,
            TopicEngagementKind::UpdateReply,
            TopicEngagementKind::UpdateTopic,
            TopicEngagementKind::RestoreReply,
        ] {
            assert_err(kind, &topic, &author);
            let resource = match kind {
                TopicEngagementKind::CreateReply
                | TopicEngagementKind::UpdateReply
                | TopicEngagementKind::RestoreReply => Resource::ForumReplies,
                _ => Resource::ForumTopics,
            };
            enforce_topic_engagement(&topic, kind, &moderator, resource)
                .expect("moderators may write on a locked topic");
        }
    }

    #[test]
    fn open_unlocked_allows_author_engagement() {
        let topic = sample_topic(TopicStatus::Open, false);
        let author = author();
        for (kind, resource) in [
            (TopicEngagementKind::Vote, Resource::ForumTopics),
            (TopicEngagementKind::CreateReply, Resource::ForumReplies),
            (TopicEngagementKind::UpdateReply, Resource::ForumReplies),
            (TopicEngagementKind::RestoreReply, Resource::ForumReplies),
            (TopicEngagementKind::UpdateTopic, Resource::ForumTopics),
        ] {
            enforce_topic_engagement(&topic, kind, &author, resource)
                .expect("open unlocked topics accept author writes");
        }
    }
}
