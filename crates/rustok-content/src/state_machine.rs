/// Type-Safe State Machine for Content Nodes
///
/// Replaces the simple ContentStatus enum with a compile-time safe state machine.
///
/// Benefits:
/// - **Compile-time safety**: Invalid transitions (e.g., Draft → Archived) are impossible
/// - **State-specific data**: Published state includes published_at, Archived includes reason
/// - **Clear transition graph**: Only valid transitions are available as methods
/// - **Self-documenting**: State diagram visible in type system
///
/// State Diagram:
/// ```text
///  ┌───────┐
///  │ Draft │──────────────────┐
///  └───┬───┘                  │
///      │ publish()            │
///      ↓                      │ archive()
///  ┌───────────┐              │
///  │ Published │──────────────┤
///  └─────┬─────┘              │
///        │ archive()          │
///        │                    ↓
///        │              ┌──────────┐
///        └─────────────→│ Archived │
///                       └──────────┘
/// ```
///
/// Usage:
/// ```rust,ignore
/// // Create new node in draft state
/// let node = ContentNode::new_draft(id, tenant_id, author_id);
///
/// // Publish (compile-time safe)
/// let node = node.publish(Utc::now())?;
///
/// // Archive with reason
/// let node = node.archive("Content outdated".to_string());
///
/// // Invalid: Draft -> Archived (compile error!)
/// // let node = node.archive(); // ❌ method not available on Draft
/// ```
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::entities::node::ContentStatus;

// ============================================================================
// State Definitions
// ============================================================================

/// Draft state - content is being created/edited
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Draft {
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Published state - content is live
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Published {
    pub published_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Archived state - content is no longer active
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Archived {
    pub archived_at: DateTime<Utc>,
    pub reason: String,
}

// ============================================================================
// State Machine
// ============================================================================

/// Type-safe content node state machine
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContentNode<S> {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub author_id: Option<Uuid>,
    pub parent_id: Option<Uuid>,
    pub kind: String,
    pub category_id: Option<Uuid>,

    // State-specific data
    pub state: S,
}

// ============================================================================
// Constructors
// ============================================================================

impl ContentNode<Draft> {
    /// Create a new content node in draft state
    pub fn new_draft(id: Uuid, tenant_id: Uuid, author_id: Option<Uuid>, kind: String) -> Self {
        let now = Utc::now();

        Self {
            id,
            tenant_id,
            author_id,
            parent_id: None,
            kind,
            category_id: None,
            state: Draft {
                created_at: now,
                updated_at: now,
            },
        }
    }
}

// ============================================================================
// Transitions: Draft
// ============================================================================

impl ContentNode<Draft> {
    /// Publish content (Draft → Published)
    ///
    /// This is the only valid transition from Draft state.
    pub fn publish(self) -> ContentNode<Published> {
        let published_at = Utc::now();

        tracing::info!(
            node_id = %self.id,
            tenant_id = %self.tenant_id,
            "Content node: Draft → Published"
        );

        ContentNode {
            id: self.id,
            tenant_id: self.tenant_id,
            author_id: self.author_id,
            parent_id: self.parent_id,
            kind: self.kind,
            category_id: self.category_id,
            state: Published {
                published_at,
                updated_at: published_at,
            },
        }
    }

    /// Update draft metadata
    pub fn update(mut self) -> Self {
        self.state.updated_at = Utc::now();
        self
    }
}

// ============================================================================
// Transitions: Published
// ============================================================================

impl ContentNode<Published> {
    /// Unpublish content back to draft (Published → Draft)
    pub fn unpublish(self) -> ContentNode<Draft> {
        let now = Utc::now();

        tracing::info!(
            node_id = %self.id,
            tenant_id = %self.tenant_id,
            "Content node: Published → Draft"
        );

        ContentNode {
            id: self.id,
            tenant_id: self.tenant_id,
            author_id: self.author_id,
            parent_id: self.parent_id,
            kind: self.kind,
            category_id: self.category_id,
            state: Draft {
                created_at: now,
                updated_at: now,
            },
        }
    }

    /// Archive published content (Published → Archived)
    pub fn archive(self, reason: String) -> ContentNode<Archived> {
        let archived_at = Utc::now();

        tracing::info!(
            node_id = %self.id,
            tenant_id = %self.tenant_id,
            reason = %reason,
            "Content node: Published → Archived"
        );

        ContentNode {
            id: self.id,
            tenant_id: self.tenant_id,
            author_id: self.author_id,
            parent_id: self.parent_id,
            kind: self.kind,
            category_id: self.category_id,
            state: Archived {
                archived_at,
                reason,
            },
        }
    }

    /// Update published content
    pub fn update(mut self) -> Self {
        self.state.updated_at = Utc::now();
        self
    }
}

// ============================================================================
// Transitions: Archived
// ============================================================================

impl ContentNode<Archived> {
    /// Restore archived content to draft (Archived → Draft)
    ///
    /// Allows restoring archived content for editing.
    pub fn restore_to_draft(self) -> ContentNode<Draft> {
        let now = Utc::now();

        tracing::info!(
            node_id = %self.id,
            tenant_id = %self.tenant_id,
            "Content node: Archived → Draft"
        );

        ContentNode {
            id: self.id,
            tenant_id: self.tenant_id,
            author_id: self.author_id,
            parent_id: self.parent_id,
            kind: self.kind,
            category_id: self.category_id,
            state: Draft {
                created_at: now,
                updated_at: now,
            },
        }
    }
}

// ============================================================================
// Common Methods (all states)
// ============================================================================

impl<S> ContentNode<S> {
    /// Get node ID
    pub fn id(&self) -> Uuid {
        self.id
    }

    /// Get tenant ID
    pub fn tenant_id(&self) -> Uuid {
        self.tenant_id
    }

    /// Set parent
    pub fn set_parent(mut self, parent_id: Uuid) -> Self {
        self.parent_id = Some(parent_id);
        self
    }

    /// Set category
    pub fn set_category(mut self, category_id: Uuid) -> Self {
        self.category_id = Some(category_id);
        self
    }
}

// ============================================================================
// Conversion to/from database ContentStatus
// ============================================================================

/// Convert type-safe state to database enum
pub trait ToContentStatus {
    fn to_status(&self) -> ContentStatus;
}

impl ToContentStatus for ContentNode<Draft> {
    fn to_status(&self) -> ContentStatus {
        ContentStatus::Draft
    }
}

impl ToContentStatus for ContentNode<Published> {
    fn to_status(&self) -> ContentStatus {
        ContentStatus::Published
    }
}

impl ToContentStatus for ContentNode<Archived> {
    fn to_status(&self) -> ContentStatus {
        ContentStatus::Archived
    }
}

// ============================================================================
// Transition Validation (runtime guard, single source of truth)
// ============================================================================

/// Validate that a status transition is allowed by the state machine rules.
///
/// Valid transitions:
/// - `Draft`     → `Published`  (publish)
/// - `Published` → `Draft`      (unpublish)
/// - `Published` → `Archived`   (archive)
/// - `Archived`  → `Draft`      (restore to draft)
///
/// Everything else is invalid.
pub fn validate_status_transition(
    current: &ContentStatus,
    target: &ContentStatus,
) -> Result<(), InvalidStatusTransition> {
    let valid = matches!(
        (current, target),
        (ContentStatus::Draft, ContentStatus::Published)
            | (ContentStatus::Published, ContentStatus::Draft)
            | (ContentStatus::Published, ContentStatus::Archived)
            | (ContentStatus::Archived, ContentStatus::Draft)
    );

    if valid {
        Ok(())
    } else {
        Err(InvalidStatusTransition {
            from: current.clone(),
            to: target.clone(),
        })
    }
}

/// Error returned when a status transition is not allowed.
#[derive(Debug, Clone)]
pub struct InvalidStatusTransition {
    pub from: ContentStatus,
    pub to: ContentStatus,
}

impl std::fmt::Display for InvalidStatusTransition {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Invalid status transition: {:?} → {:?}",
            self.from, self.to
        )
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_draft() {
        let id = Uuid::new_v4();
        let tenant_id = Uuid::new_v4();
        let author_id = Some(Uuid::new_v4());

        let node = ContentNode::new_draft(id, tenant_id, author_id, "article".to_string());

        assert_eq!(node.id, id);
        assert_eq!(node.tenant_id, tenant_id);
        assert_eq!(node.author_id, author_id);
        assert_eq!(node.kind, "article");
    }

    #[test]
    fn test_draft_to_published() {
        let node = ContentNode::new_draft(
            Uuid::new_v4(),
            Uuid::new_v4(),
            Some(Uuid::new_v4()),
            "article".to_string(),
        );

        let node = node.publish();

        assert!(node.state.published_at <= Utc::now());
        assert_eq!(node.to_status(), ContentStatus::Published);
    }

    #[test]
    fn test_published_to_archived() {
        let node = ContentNode::new_draft(
            Uuid::new_v4(),
            Uuid::new_v4(),
            Some(Uuid::new_v4()),
            "article".to_string(),
        )
        .publish();

        let reason = "Content outdated".to_string();
        let node = node.archive(reason.clone());

        assert_eq!(node.state.reason, reason);
        assert_eq!(node.to_status(), ContentStatus::Archived);
    }

    #[test]
    fn test_archived_to_draft() {
        let node = ContentNode::new_draft(
            Uuid::new_v4(),
            Uuid::new_v4(),
            Some(Uuid::new_v4()),
            "article".to_string(),
        )
        .publish()
        .archive("Test".to_string())
        .restore_to_draft();

        assert_eq!(node.to_status(), ContentStatus::Draft);
    }

    #[test]
    fn test_update_timestamps() {
        let node = ContentNode::new_draft(
            Uuid::new_v4(),
            Uuid::new_v4(),
            Some(Uuid::new_v4()),
            "article".to_string(),
        );

        let created_at = node.state.created_at;
        std::thread::sleep(std::time::Duration::from_millis(10));

        let node = node.update();

        assert!(node.state.updated_at > created_at);
    }

    #[test]
    fn test_published_to_draft_unpublish() {
        let node = ContentNode::new_draft(
            Uuid::new_v4(),
            Uuid::new_v4(),
            Some(Uuid::new_v4()),
            "article".to_string(),
        )
        .publish()
        .unpublish();

        assert_eq!(node.to_status(), ContentStatus::Draft);
    }

    // --- validate_status_transition ---

    #[test]
    fn test_valid_transitions() {
        assert!(validate_status_transition(&ContentStatus::Draft, &ContentStatus::Published).is_ok());
        assert!(validate_status_transition(&ContentStatus::Published, &ContentStatus::Draft).is_ok());
        assert!(validate_status_transition(&ContentStatus::Published, &ContentStatus::Archived).is_ok());
        assert!(validate_status_transition(&ContentStatus::Archived, &ContentStatus::Draft).is_ok());
    }

    #[test]
    fn test_invalid_draft_to_archived() {
        assert!(validate_status_transition(&ContentStatus::Draft, &ContentStatus::Archived).is_err());
    }

    #[test]
    fn test_invalid_archived_to_published() {
        assert!(validate_status_transition(&ContentStatus::Archived, &ContentStatus::Published).is_err());
    }

    #[test]
    fn test_invalid_same_status_transitions() {
        assert!(validate_status_transition(&ContentStatus::Draft, &ContentStatus::Draft).is_err());
        assert!(validate_status_transition(&ContentStatus::Published, &ContentStatus::Published).is_err());
        assert!(validate_status_transition(&ContentStatus::Archived, &ContentStatus::Archived).is_err());
    }

    #[test]
    fn test_transition_error_message() {
        let err = validate_status_transition(&ContentStatus::Draft, &ContentStatus::Archived)
            .unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("Draft"));
        assert!(msg.contains("Archived"));
    }

    // Compile-time safety tests (these should NOT compile if uncommented)

    // #[test]
    // fn test_invalid_draft_to_archived() {
    //     let node = ContentNode::new_draft(/* ... */);
    //     // ❌ Compile error: no method `archive` on `ContentNode<Draft>`
    //     let node = node.archive("test".to_string());
    // }

    // #[test]
    // fn test_invalid_archived_to_published() {
    //     let node = /* ... archived node ... */;
    //     // ❌ Compile error: no method `publish` on `ContentNode<Archived>`
    //     let node = node.publish();
    // }
}
