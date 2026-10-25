//! Blog status model and the canonical transition table of the write path.
//!
//! The persisted Blog post status is a single column. Every status change goes
//! through the service layer, which consults [`BlogPostStatus::can_transition_to`]
//! before it writes. The table below is the only transition contract of the
//! module: the retired generic `BlogPost<Draft|Published|Archived>` type-state
//! model was a parallel implementation that no write path used, and it
//! contradicted the persisted semantics (it overwrote `created_at` when a post
//! left the published state). It was removed instead of being maintained.
//!
//! State Diagram:
//! ```text
//!   ┌───────┐
//!   │ Draft │
//!   └───┬───┘
//!       │ publish
//!       ↓
//!   ┌───────────┐
//!   │ Published │──────────────┐
//!   └─────┬─────┘              │ archive
//!         │ unpublish           ↓
//!         └─────────→   ┌──────────┐
//!              Draft    │ Archived │
//!                       └────┬─────┘
//!                            │ restore
//!                            ↓
//!                         Draft
//! ```
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

// ============================================================================
// Blog Post Status
// ============================================================================

/// Blog post status enum for database storage
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema, Default)]
#[serde(rename_all = "lowercase")]
pub enum BlogPostStatus {
    #[default]
    Draft,
    Published,
    Archived,
}

impl BlogPostStatus {
    /// Canonical transition table of the Blog post write path.
    ///
    /// Every persistence-backed command consults this table before it mutates
    /// the stored status, so an invalid transition can never reach the
    /// database.
    pub const fn can_transition_to(self, next: Self) -> bool {
        matches!(
            (self, next),
            (Self::Draft, Self::Published)
                | (Self::Published, Self::Draft)
                | (Self::Published, Self::Archived)
                | (Self::Archived, Self::Draft)
        )
    }
}

// ============================================================================
// Blog Comment State Machine
// ============================================================================
//
// Blog does not own comment state. Comments (including their moderation
// status) are owned by the `rustok-comments` module and reached through the
// `CommentsThreadPort`; a second `CommentStatus` model here was a dead
// parallel implementation that no write path or transport consulted. The
// canonical model lives in `rustok-comments`, so it is intentionally absent
// from this module.

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn runtime_transition_table_matches_the_state_graph() {
        assert!(BlogPostStatus::Draft.can_transition_to(BlogPostStatus::Published));
        assert!(BlogPostStatus::Published.can_transition_to(BlogPostStatus::Draft));
        assert!(BlogPostStatus::Published.can_transition_to(BlogPostStatus::Archived));
        assert!(BlogPostStatus::Archived.can_transition_to(BlogPostStatus::Draft));

        assert!(!BlogPostStatus::Draft.can_transition_to(BlogPostStatus::Archived));
        assert!(!BlogPostStatus::Archived.can_transition_to(BlogPostStatus::Published));
        assert!(!BlogPostStatus::Draft.can_transition_to(BlogPostStatus::Draft));
    }

    #[test]
    fn the_transition_table_has_exactly_four_edges() {
        let statuses = [
            BlogPostStatus::Draft,
            BlogPostStatus::Published,
            BlogPostStatus::Archived,
        ];
        let mut edges = 0usize;
        for from in statuses {
            for to in statuses {
                if from.can_transition_to(to) {
                    edges += 1;
                }
            }
        }
        assert_eq!(edges, 4);
    }

    #[test]
    fn a_draft_post_cannot_be_archived_directly() {
        assert!(!BlogPostStatus::Draft.can_transition_to(BlogPostStatus::Archived));
    }

    #[test]
    fn an_archived_post_cannot_be_published_directly() {
        assert!(!BlogPostStatus::Archived.can_transition_to(BlogPostStatus::Published));
    }

    #[test]
    fn the_publish_and_unpublish_transitions_are_inverse() {
        assert_eq!(
            BlogPostStatus::Draft.can_transition_to(BlogPostStatus::Published),
            BlogPostStatus::Published.can_transition_to(BlogPostStatus::Draft),
        );
    }

    #[test]
    fn the_transition_table_rejects_a_self_transition() {
        for status in [
            BlogPostStatus::Draft,
            BlogPostStatus::Published,
            BlogPostStatus::Archived,
        ] {
            assert!(!status.can_transition_to(status));
        }
    }

    #[test]
    fn the_status_model_carries_no_payload() {
        assert_eq!(std::mem::size_of::<BlogPostStatus>(), 1);
    }

    #[test]
    fn the_default_post_status_is_the_entry_state() {
        assert_eq!(BlogPostStatus::default(), BlogPostStatus::Draft);
    }
}
