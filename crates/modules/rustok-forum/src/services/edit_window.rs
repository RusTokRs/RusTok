//! Author edit window for topics and replies.
//!
//! `max_edit_window_minutes` closes the window for an item's author after the item's creation.
//! The caller is the author when its user id equals the item's `author_id`. Callers that hold
//! the moderation scope `All` for the resource (moderators and administrators) are not
//! limited.
//!
//! The rule checks ownership and the moderation scope, not the update scope. Staff roles hold the
//! update scope `All`, and the forum author role holds `Own`, so ownership is the test that holds
//! for both. See `DECISIONS/2026-10-09-forum-wire-author-edit-window-and-locked-list-settings.md`
//! and `DECISIONS/2026-10-09-forum-author-self-service-and-deletion.md`.

use chrono::{DateTime, Duration, FixedOffset, Utc};
use rustok_api::{Action, Resource};
use rustok_core::{PermissionScope, SecurityContext};
use uuid::Uuid;

use crate::error::{ForumError, ForumResult};

pub(crate) fn enforce_author_edit_window(
    security: &SecurityContext,
    resource: Resource,
    owner_id: Option<Uuid>,
    created_at: DateTime<FixedOffset>,
    max_edit_window_minutes: u32,
    now: DateTime<Utc>,
) -> ForumResult<()> {
    if max_edit_window_minutes == 0 {
        return Ok(());
    }
    let Some(author_id) = owner_id else {
        return Ok(());
    };
    if security.user_id != Some(author_id) {
        return Ok(());
    }
    if security.get_scope(resource, Action::Moderate) == PermissionScope::All {
        return Ok(());
    }
    let window = Duration::minutes(i64::from(max_edit_window_minutes));
    if now.signed_duration_since(created_at) > window {
        return Err(ForumError::Validation(
            "Forum edit window has closed for this author".to_string(),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use rustok_api::Permission;
    use rustok_core::UserRole;

    fn author_security(author_id: Uuid) -> SecurityContext {
        SecurityContext::from_permissions(
            UserRole::Customer,
            Some(author_id),
            [Permission::FORUM_TOPICS_UPDATE],
        )
    }

    fn created_minutes_ago(minutes: i64, now: DateTime<Utc>) -> DateTime<FixedOffset> {
        (now - Duration::minutes(minutes)).fixed_offset()
    }

    fn assert_closed(result: ForumResult<()>) {
        assert!(
            matches!(result, Err(ForumError::Validation(_))),
            "the window should be closed, got {result:?}"
        );
    }

    #[test]
    fn author_inside_the_window_is_allowed() {
        let now = Utc::now();
        let author = Uuid::new_v4();
        let result = enforce_author_edit_window(
            &author_security(author),
            Resource::ForumTopics,
            Some(author),
            created_minutes_ago(9, now),
            10,
            now,
        );
        assert!(result.is_ok());
    }

    #[test]
    fn author_after_the_window_is_rejected() {
        let now = Utc::now();
        let author = Uuid::new_v4();
        assert_closed(enforce_author_edit_window(
            &author_security(author),
            Resource::ForumTopics,
            Some(author),
            created_minutes_ago(11, now),
            10,
            now,
        ));
    }

    #[test]
    fn exactly_at_the_limit_is_still_open() {
        let now = Utc::now();
        let author = Uuid::new_v4();
        let result = enforce_author_edit_window(
            &author_security(author),
            Resource::ForumReplies,
            Some(author),
            created_minutes_ago(10, now),
            10,
            now,
        );
        assert!(result.is_ok());
    }

    #[test]
    fn zero_window_is_unlimited() {
        let now = Utc::now();
        let author = Uuid::new_v4();
        let result = enforce_author_edit_window(
            &author_security(author),
            Resource::ForumTopics,
            Some(author),
            created_minutes_ago(100_000, now),
            0,
            now,
        );
        assert!(result.is_ok());
    }

    #[test]
    fn moderator_who_is_also_the_author_is_not_limited() {
        let now = Utc::now();
        let author = Uuid::new_v4();
        let manager = SecurityContext::new(UserRole::Manager, Some(author));
        let result = enforce_author_edit_window(
            &manager,
            Resource::ForumTopics,
            Some(author),
            created_minutes_ago(60, now),
            10,
            now,
        );
        assert!(result.is_ok());
    }

    #[test]
    fn non_author_is_not_limited_by_the_author_window() {
        let now = Utc::now();
        let author = Uuid::new_v4();
        let other = Uuid::new_v4();
        let result = enforce_author_edit_window(
            &author_security(other),
            Resource::ForumTopics,
            Some(author),
            created_minutes_ago(60, now),
            10,
            now,
        );
        assert!(result.is_ok());
    }

    #[test]
    fn future_creation_time_does_not_close_the_window() {
        let now = Utc::now();
        let author = Uuid::new_v4();
        let result = enforce_author_edit_window(
            &author_security(author),
            Resource::ForumTopics,
            Some(author),
            (now + Duration::minutes(5)).fixed_offset(),
            10,
            now,
        );
        assert!(result.is_ok());
    }
}
