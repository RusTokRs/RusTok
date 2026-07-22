/// Type-Safe State Machine for Forum Topics
///
/// Models valid status transitions for forum topics:
///
/// ```text
///  ┌──────┐
///  │ Open │◄──────────────────┐
///  └──┬───┘                   │
///     │ close()               │ reopen()
///     ↓                       │
///  ┌────────┐                 │
///  │ Closed │─────────────────┘
///  └──┬─────┘
///     │ archive()
///     ↓
///  ┌──────────┐
///  │ Archived │─── reopen() ──→ Open
///  └──────────┘
///
///  Open ──── archive() ──→ Archived
/// ```
///
/// Allowed transitions:
/// - Open     → Closed   (close)
/// - Open     → Archived (archive)
/// - Closed   → Open     (reopen)
/// - Closed   → Archived (archive)
/// - Archived → Open     (reopen)
use std::fmt;

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::constants::{reply_status, topic_status};

// ============================================================================
// Topic Status Enum
// ============================================================================

/// Enumerated topic status with validated transitions.
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Hash,
    EnumIter,
    DeriveActiveEnum,
    Serialize,
    Deserialize,
    ToSchema,
)]
#[sea_orm(rs_type = "String", db_type = "String(StringLen::N(32))")]
#[serde(rename_all = "snake_case")]
pub enum TopicStatus {
    #[sea_orm(string_value = "open")]
    Open,
    #[sea_orm(string_value = "closed")]
    Closed,
    #[sea_orm(string_value = "archived")]
    Archived,
}

impl TopicStatus {
    /// Parse a topic status from a string value (from metadata).
    pub fn from_str_value(s: &str) -> Option<Self> {
        match s {
            topic_status::OPEN => Some(Self::Open),
            topic_status::CLOSED => Some(Self::Closed),
            topic_status::ARCHIVED => Some(Self::Archived),
            _ => None,
        }
    }

    /// Convert to string value for storage in metadata.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Open => topic_status::OPEN,
            Self::Closed => topic_status::CLOSED,
            Self::Archived => topic_status::ARCHIVED,
        }
    }

    /// Check if a transition to the target status is allowed.
    pub fn can_transition_to(&self, target: &TopicStatus) -> bool {
        matches!(
            (self, target),
            (Self::Open, Self::Closed)
                | (Self::Open, Self::Archived)
                | (Self::Closed, Self::Open)
                | (Self::Closed, Self::Archived)
                | (Self::Archived, Self::Open)
        )
    }

    /// Validate a transition and return an error if it's not allowed.
    pub fn validate_transition(&self, target: &TopicStatus) -> Result<(), InvalidTopicTransition> {
        if self.can_transition_to(target) {
            Ok(())
        } else {
            Err(InvalidTopicTransition {
                from: *self,
                to: *target,
            })
        }
    }
}

impl fmt::Display for TopicStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

// ============================================================================
// Reply Status Enum
// ============================================================================

/// Enumerated reply status with validated transitions.
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Hash,
    EnumIter,
    DeriveActiveEnum,
    Serialize,
    Deserialize,
    ToSchema,
)]
#[sea_orm(rs_type = "String", db_type = "String(StringLen::N(32))")]
#[serde(rename_all = "snake_case")]
pub enum ReplyStatus {
    #[sea_orm(string_value = "pending")]
    Pending,
    #[sea_orm(string_value = "approved")]
    Approved,
    #[sea_orm(string_value = "rejected")]
    Rejected,
    #[sea_orm(string_value = "hidden")]
    Hidden,
    #[sea_orm(string_value = "flagged")]
    Flagged,
    #[sea_orm(string_value = "deleted")]
    Deleted,
}

impl ReplyStatus {
    /// Parse a reply status from a string value.
    pub fn from_str_value(s: &str) -> Option<Self> {
        match s {
            reply_status::PENDING => Some(Self::Pending),
            reply_status::APPROVED => Some(Self::Approved),
            reply_status::REJECTED => Some(Self::Rejected),
            reply_status::HIDDEN => Some(Self::Hidden),
            reply_status::FLAGGED => Some(Self::Flagged),
            reply_status::DELETED => Some(Self::Deleted),
            _ => None,
        }
    }

    /// Convert to string for metadata storage.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Pending => reply_status::PENDING,
            Self::Approved => reply_status::APPROVED,
            Self::Rejected => reply_status::REJECTED,
            Self::Hidden => reply_status::HIDDEN,
            Self::Flagged => reply_status::FLAGGED,
            Self::Deleted => reply_status::DELETED,
        }
    }

    /// Check if a transition to the target status is allowed.
    ///
    /// Allowed transitions:
    /// - Approved → Hidden  (moderation hide)
    /// - Approved → Flagged (user flag)
    /// - Approved → Deleted (soft-delete)
    /// - Flagged  → Approved (resolve flag)
    /// - Flagged  → Hidden  (moderator hides flagged)
    /// - Flagged  → Deleted (moderator deletes flagged)
    /// - Hidden   → Approved (moderator restores)
    /// - Hidden   → Deleted (moderator deletes hidden)
    pub fn can_transition_to(&self, target: &ReplyStatus) -> bool {
        matches!(
            (self, target),
            (Self::Pending, Self::Approved)
                | (Self::Pending, Self::Rejected)
                | (Self::Pending, Self::Hidden)
                | (Self::Pending, Self::Deleted)
                | (Self::Approved, Self::Rejected)
                | (Self::Approved, Self::Hidden)
                | (Self::Approved, Self::Flagged)
                | (Self::Approved, Self::Deleted)
                | (Self::Rejected, Self::Approved)
                | (Self::Rejected, Self::Hidden)
                | (Self::Rejected, Self::Deleted)
                | (Self::Flagged, Self::Approved)
                | (Self::Flagged, Self::Rejected)
                | (Self::Flagged, Self::Hidden)
                | (Self::Flagged, Self::Deleted)
                | (Self::Hidden, Self::Approved)
                | (Self::Hidden, Self::Rejected)
                | (Self::Hidden, Self::Deleted)
        )
    }

    /// Validate a transition and return an error if it's not allowed.
    pub fn validate_transition(&self, target: &ReplyStatus) -> Result<(), InvalidReplyTransition> {
        if self.can_transition_to(target) {
            Ok(())
        } else {
            Err(InvalidReplyTransition {
                from: *self,
                to: *target,
            })
        }
    }
}

impl fmt::Display for ReplyStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

// ============================================================================
// Transition Errors
// ============================================================================

/// Error returned when a topic status transition is not allowed.
#[derive(Debug, Clone)]
pub struct InvalidTopicTransition {
    pub from: TopicStatus,
    pub to: TopicStatus,
}

impl fmt::Display for InvalidTopicTransition {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Invalid topic status transition: {} → {}",
            self.from, self.to
        )
    }
}

impl std::error::Error for InvalidTopicTransition {}

/// Error returned when a reply status transition is not allowed.
#[derive(Debug, Clone)]
pub struct InvalidReplyTransition {
    pub from: ReplyStatus,
    pub to: ReplyStatus,
}

impl fmt::Display for InvalidReplyTransition {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Invalid reply status transition: {} → {}",
            self.from, self.to
        )
    }
}

impl std::error::Error for InvalidReplyTransition {}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // --- TopicStatus parsing ---

    #[test]
    fn parse_valid_topic_statuses() {
        assert_eq!(TopicStatus::from_str_value("open"), Some(TopicStatus::Open));
        assert_eq!(
            TopicStatus::from_str_value("closed"),
            Some(TopicStatus::Closed)
        );
        assert_eq!(
            TopicStatus::from_str_value("archived"),
            Some(TopicStatus::Archived)
        );
    }

    #[test]
    fn parse_invalid_topic_status_returns_none() {
        assert_eq!(TopicStatus::from_str_value("unknown"), None);
        assert_eq!(TopicStatus::from_str_value(""), None);
    }

    #[test]
    fn topic_status_roundtrip() {
        for status in [
            TopicStatus::Open,
            TopicStatus::Closed,
            TopicStatus::Archived,
        ] {
            let s = status.as_str();
            assert_eq!(TopicStatus::from_str_value(s), Some(status));
        }
    }

    // --- TopicStatus transitions ---

    #[test]
    fn valid_topic_transitions() {
        // Open → Closed
        assert!(TopicStatus::Open.can_transition_to(&TopicStatus::Closed));
        // Open → Archived
        assert!(TopicStatus::Open.can_transition_to(&TopicStatus::Archived));
        // Closed → Open (reopen)
        assert!(TopicStatus::Closed.can_transition_to(&TopicStatus::Open));
        // Closed → Archived
        assert!(TopicStatus::Closed.can_transition_to(&TopicStatus::Archived));
        // Archived → Open (reopen)
        assert!(TopicStatus::Archived.can_transition_to(&TopicStatus::Open));
    }

    #[test]
    fn invalid_topic_transitions() {
        // Self-transitions are invalid
        assert!(!TopicStatus::Open.can_transition_to(&TopicStatus::Open));
        assert!(!TopicStatus::Closed.can_transition_to(&TopicStatus::Closed));
        assert!(!TopicStatus::Archived.can_transition_to(&TopicStatus::Archived));
        // Archived → Closed is invalid (must reopen first)
        assert!(!TopicStatus::Archived.can_transition_to(&TopicStatus::Closed));
    }

    #[test]
    fn validate_topic_transition_returns_error() {
        let err = TopicStatus::Archived
            .validate_transition(&TopicStatus::Closed)
            .unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("archived"));
        assert!(msg.contains("closed"));
    }

    #[test]
    fn validate_topic_transition_ok() {
        assert!(
            TopicStatus::Open
                .validate_transition(&TopicStatus::Closed)
                .is_ok()
        );
    }

    // --- ReplyStatus parsing ---

    #[test]
    fn parse_valid_reply_statuses() {
        assert_eq!(
            ReplyStatus::from_str_value("pending"),
            Some(ReplyStatus::Pending)
        );
        assert_eq!(
            ReplyStatus::from_str_value("approved"),
            Some(ReplyStatus::Approved)
        );
        assert_eq!(
            ReplyStatus::from_str_value("rejected"),
            Some(ReplyStatus::Rejected)
        );
        assert_eq!(
            ReplyStatus::from_str_value("hidden"),
            Some(ReplyStatus::Hidden)
        );
        assert_eq!(
            ReplyStatus::from_str_value("flagged"),
            Some(ReplyStatus::Flagged)
        );
        assert_eq!(
            ReplyStatus::from_str_value("deleted"),
            Some(ReplyStatus::Deleted)
        );
    }

    #[test]
    fn parse_invalid_reply_status_returns_none() {
        assert_eq!(ReplyStatus::from_str_value("unknown"), None);
    }

    #[test]
    fn reply_status_roundtrip() {
        for status in [
            ReplyStatus::Pending,
            ReplyStatus::Approved,
            ReplyStatus::Rejected,
            ReplyStatus::Hidden,
            ReplyStatus::Flagged,
            ReplyStatus::Deleted,
        ] {
            let s = status.as_str();
            assert_eq!(ReplyStatus::from_str_value(s), Some(status));
        }
    }

    // --- ReplyStatus transitions ---

    #[test]
    fn valid_reply_transitions() {
        assert!(ReplyStatus::Pending.can_transition_to(&ReplyStatus::Approved));
        assert!(ReplyStatus::Pending.can_transition_to(&ReplyStatus::Rejected));
        assert!(ReplyStatus::Pending.can_transition_to(&ReplyStatus::Hidden));
        assert!(ReplyStatus::Pending.can_transition_to(&ReplyStatus::Deleted));
        assert!(ReplyStatus::Approved.can_transition_to(&ReplyStatus::Rejected));
        assert!(ReplyStatus::Approved.can_transition_to(&ReplyStatus::Hidden));
        assert!(ReplyStatus::Approved.can_transition_to(&ReplyStatus::Flagged));
        assert!(ReplyStatus::Approved.can_transition_to(&ReplyStatus::Deleted));
        assert!(ReplyStatus::Rejected.can_transition_to(&ReplyStatus::Approved));
        assert!(ReplyStatus::Rejected.can_transition_to(&ReplyStatus::Hidden));
        assert!(ReplyStatus::Rejected.can_transition_to(&ReplyStatus::Deleted));
        assert!(ReplyStatus::Flagged.can_transition_to(&ReplyStatus::Approved));
        assert!(ReplyStatus::Flagged.can_transition_to(&ReplyStatus::Rejected));
        assert!(ReplyStatus::Flagged.can_transition_to(&ReplyStatus::Hidden));
        assert!(ReplyStatus::Flagged.can_transition_to(&ReplyStatus::Deleted));
        assert!(ReplyStatus::Hidden.can_transition_to(&ReplyStatus::Approved));
        assert!(ReplyStatus::Hidden.can_transition_to(&ReplyStatus::Rejected));
        assert!(ReplyStatus::Hidden.can_transition_to(&ReplyStatus::Deleted));
    }

    #[test]
    fn invalid_reply_transitions() {
        assert!(!ReplyStatus::Pending.can_transition_to(&ReplyStatus::Pending));
        assert!(!ReplyStatus::Approved.can_transition_to(&ReplyStatus::Approved));
        assert!(!ReplyStatus::Rejected.can_transition_to(&ReplyStatus::Rejected));
        assert!(!ReplyStatus::Deleted.can_transition_to(&ReplyStatus::Deleted));
        assert!(!ReplyStatus::Deleted.can_transition_to(&ReplyStatus::Pending));
        assert!(!ReplyStatus::Deleted.can_transition_to(&ReplyStatus::Approved));
        assert!(!ReplyStatus::Deleted.can_transition_to(&ReplyStatus::Rejected));
        assert!(!ReplyStatus::Deleted.can_transition_to(&ReplyStatus::Hidden));
        assert!(!ReplyStatus::Deleted.can_transition_to(&ReplyStatus::Flagged));
        assert!(!ReplyStatus::Pending.can_transition_to(&ReplyStatus::Flagged));
        assert!(!ReplyStatus::Hidden.can_transition_to(&ReplyStatus::Flagged));
    }

    #[test]
    fn validate_reply_transition_returns_error() {
        let err = ReplyStatus::Deleted
            .validate_transition(&ReplyStatus::Approved)
            .unwrap_err();
        assert!(err.to_string().contains("deleted"));
        assert!(err.to_string().contains("approved"));
    }
}
