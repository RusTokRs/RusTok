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

/// Comment status enum for database storage
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum CommentStatus {
    #[default]
    Pending,
    Approved,
    Spam,
    Trash,
}

impl CommentStatus {
    /// Check if comment is visible publicly
    pub fn is_visible(&self) -> bool {
        matches!(self, Self::Approved)
    }

    /// Transition to approved
    pub fn approve(self) -> Self {
        match self {
            Self::Pending | Self::Spam => Self::Approved,
            Self::Approved | Self::Trash => self,
        }
    }

    /// Transition to spam
    pub fn mark_spam(self) -> Self {
        match self {
            Self::Pending | Self::Approved => Self::Spam,
            Self::Spam | Self::Trash => self,
        }
    }

    /// Transition to trash
    pub fn trash(self) -> Self {
        Self::Trash
    }
}

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
    fn the_comment_model_carries_no_payload() {
        assert_eq!(std::mem::size_of::<CommentStatus>(), 1);
    }

    #[test]
    fn the_default_statuses_are_the_entry_states() {
        assert_eq!(BlogPostStatus::default(), BlogPostStatus::Draft);
        assert_eq!(CommentStatus::default(), CommentStatus::Pending);
    }

    #[test]
    fn test_comment_status_transitions() {
        // Pending -> Approved
        assert_eq!(CommentStatus::Pending.approve(), CommentStatus::Approved);

        // Approved -> Spam
        assert_eq!(CommentStatus::Approved.mark_spam(), CommentStatus::Spam);

        // Spam -> Approved
        assert_eq!(CommentStatus::Spam.approve(), CommentStatus::Approved);

        // Any -> Trash
        assert_eq!(CommentStatus::Pending.trash(), CommentStatus::Trash);
        assert_eq!(CommentStatus::Approved.trash(), CommentStatus::Trash);
    }

    #[test]
    fn test_comment_visibility() {
        assert!(!CommentStatus::Pending.is_visible());
        assert!(CommentStatus::Approved.is_visible());
        assert!(!CommentStatus::Spam.is_visible());
        assert!(!CommentStatus::Trash.is_visible());
    }

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

    #[test]
    fn approved_stays_approved_on_approve() {
        let status = CommentStatus::Approved;
        assert_eq!(status.approve(), CommentStatus::Approved);
    }

    #[test]
    fn trash_is_terminal() {
        let status = CommentStatus::Trash;
        assert_eq!(status.approve(), CommentStatus::Trash);
        assert_eq!(status.mark_spam(), CommentStatus::Trash);
    }
}
