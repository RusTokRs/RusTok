//! Property-based tests for the canonical Blog status model.
//!
//! The retired generic `BlogPost<Draft|Published|Archived>` type-state model is
//! gone: it was dead code that contradicted the persisted semantics (it
//! overwrote `created_at` when a post left the published state) and no write
//! path used it. The properties below guard the model the write path actually
//! uses — the persisted status enum, its canonical transition table
//! ([`BlogPostStatus::can_transition_to`], consulted by
//! `crate::services::post::commands`) and the comment moderation model.
//!
//! Every property is a property of the *relation* between two statuses, so a
//! change of the transition table is caught by the property suite and not only
//! by the explicit table test in `state_machine.rs`.

use super::state_machine::*;
use proptest::prelude::*;

/// Generates every persisted Blog post status.
fn status_strategy() -> impl Strategy<Value = BlogPostStatus> {
    any::<u8>().prop_map(|value| match value % 3 {
        0 => BlogPostStatus::Draft,
        1 => BlogPostStatus::Published,
        _ => BlogPostStatus::Archived,
    })
}

/// Generates every Blog comment status.
fn comment_status_strategy() -> impl Strategy<Value = CommentStatus> {
    any::<u8>().prop_map(|value| match value % 4 {
        0 => CommentStatus::Pending,
        1 => CommentStatus::Approved,
        2 => CommentStatus::Spam,
        _ => CommentStatus::Trash,
    })
}

proptest! {
    /// A post is published out of the draft state only.
    #[test]
    fn publishing_is_allowed_only_from_draft(from in status_strategy()) {
        prop_assert_eq!(
            from.can_transition_to(BlogPostStatus::Published),
            from == BlogPostStatus::Draft,
        );
    }

    /// A post is archived out of the published state only.
    #[test]
    fn archiving_is_allowed_only_from_published(from in status_strategy()) {
        prop_assert_eq!(
            from.can_transition_to(BlogPostStatus::Archived),
            from == BlogPostStatus::Published,
        );
    }

    /// A post returns to the draft state out of the published and the archived
    /// state, and never out of the draft state itself.
    #[test]
    fn restoring_to_draft_is_allowed_from_published_and_archived(from in status_strategy()) {
        prop_assert_eq!(
            from.can_transition_to(BlogPostStatus::Draft),
            matches!(from, BlogPostStatus::Published | BlogPostStatus::Archived),
        );
    }

    /// The transition table has no self-transition.
    #[test]
    fn a_status_never_transitions_to_itself(from in status_strategy()) {
        prop_assert!(!from.can_transition_to(from));
    }

    /// The only symmetric pair of the transition table is the publish and the
    /// unpublish pair.
    #[test]
    fn the_only_symmetric_transition_pair_is_publish_and_unpublish(
        from in status_strategy(),
        to in status_strategy(),
    ) {
        let symmetric = from.can_transition_to(to) && to.can_transition_to(from);
        prop_assert_eq!(
            symmetric,
            matches!(
                (from, to),
                (BlogPostStatus::Draft, BlogPostStatus::Published)
                    | (BlogPostStatus::Published, BlogPostStatus::Draft)
            ),
        );
    }

    /// Every persisted status is reachable from the draft state in at most two
    /// transitions.
    #[test]
    fn every_status_is_reachable_from_draft(from in status_strategy()) {
        let reachable = from == BlogPostStatus::Draft
            || BlogPostStatus::Draft.can_transition_to(from)
            || (BlogPostStatus::Draft.can_transition_to(BlogPostStatus::Published)
                && BlogPostStatus::Published.can_transition_to(from));
        prop_assert!(reachable);
    }

    /// A post that can be archived can always be restored to the draft state.
    #[test]
    fn an_archivable_post_can_always_be_restored(from in status_strategy()) {
        prop_assert!(
            !from.can_transition_to(BlogPostStatus::Archived)
                || from.can_transition_to(BlogPostStatus::Draft),
        );
    }

    /// A draft post is never archived directly.
    #[test]
    fn a_draft_post_is_never_archived_directly(from in status_strategy()) {
        prop_assert!(
            from != BlogPostStatus::Draft
                || !from.can_transition_to(BlogPostStatus::Archived),
        );
    }

    /// An archived post is never published directly.
    #[test]
    fn an_archived_post_is_never_published_directly(from in status_strategy()) {
        prop_assert!(
            from != BlogPostStatus::Archived
                || !from.can_transition_to(BlogPostStatus::Published),
        );
    }

    /// Only an approved comment is publicly visible.
    #[test]
    fn comment_visibility_matches_the_approved_status(from in comment_status_strategy()) {
        prop_assert_eq!(from.is_visible(), from == CommentStatus::Approved);
    }

    /// A trashed comment never becomes visible again.
    #[test]
    fn a_trashed_comment_never_becomes_visible_again(from in comment_status_strategy()) {
        prop_assert!(!from.trash().is_visible());
        prop_assert!(
            from != CommentStatus::Trash
                || (!from.approve().is_visible() && !from.mark_spam().is_visible()),
        );
    }
}

/// The comment visibility is decided by the approved status only.
#[test]
fn comment_visibility_is_correct() {
    let statuses = [
        CommentStatus::Pending,
        CommentStatus::Approved,
        CommentStatus::Spam,
        CommentStatus::Trash,
    ];

    for status in statuses {
        let is_visible = status.is_visible();
        assert_eq!(is_visible, status == CommentStatus::Approved);
    }
}

/// An approved comment stays approved.
#[test]
fn approved_stays_approved_on_approve() {
    let status = CommentStatus::Approved;
    assert_eq!(status.approve(), CommentStatus::Approved);
}

/// The trashed state is terminal.
#[test]
fn trash_is_terminal() {
    let status = CommentStatus::Trash;
    assert_eq!(status.approve(), CommentStatus::Trash);
    assert_eq!(status.mark_spam(), CommentStatus::Trash);
}
