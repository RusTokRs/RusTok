/// Type-Safe State Machine for Blog Posts
///
/// Replaces the simple status handling with a compile-time safe state machine.
///
/// Benefits:
/// - **Compile-time safety**: Invalid transitions are impossible
/// - **State-specific data**: Published state includes published_at, Archived includes reason
/// - **Clear transition graph**: Only valid transitions are available as methods
/// - **Self-documenting**: State diagram visible in type system
///
/// State Diagram:
/// ```text
///   ┌───────┐
///   │ Draft │──────────────────┐
///   └───┬───┘                  │
///       │ publish()            │ archive()
///       ↓                      │
///   ┌───────────┐              │
///   │ Published │──────────────┤
///   └─────┬─────┘              │
///         │ unpublish()        │ archive()
///         │                    ↓
///         └─────────→   ┌──────────┐
///                       │ Archived │
///                       └──────────┘
///                          │ restore()
///                          ↓
///                       ┌───────┐
///                       │ Draft │
///                       └───────┘
/// ```
///
/// Usage:
/// ```ignore
/// // Create new post in draft state
/// let post = BlogPost::new_draft(id, tenant_id, author_id);
///
/// // Publish (compile-time safe)
/// let post = post.publish(Utc::now());
///
/// // Archive with reason
/// let post = post.archive("Content outdated".to_string());
///
/// // Invalid: Draft -> Archived (compile error!)
/// // let post = draft_post.archive(); // ❌ method not available on Draft
/// ```
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

// ============================================================================
// State Definitions
// ============================================================================

/// Draft state - post is being created/edited
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Draft {
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Published state - post is live and visible
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Published {
    pub published_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Archived state - post is no longer active
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Archived {
    pub archived_at: DateTime<Utc>,
    pub reason: String,
}

// ============================================================================
// State Machine
// ============================================================================

/// Type-safe blog post state machine
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlogPost<S> {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub author_id: Uuid,

    // Content metadata
    pub title: String,
    pub slug: String,
    pub locale: String,

    // Optional relations
    pub category_id: Option<Uuid>,
    pub tags: Vec<String>,

    // State-specific data
    pub state: S,
}

// ============================================================================
// Constructors
// ============================================================================

impl BlogPost<Draft> {
    /// Create a new blog post in draft state
    pub fn new_draft(
        id: Uuid,
        tenant_id: Uuid,
        author_id: Uuid,
        title: String,
        slug: String,
        locale: String,
    ) -> Self {
        let now = Utc::now();

        Self {
            id,
            tenant_id,
            author_id,
            title,
            slug,
            locale,
            category_id: None,
            tags: Vec::new(),
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

impl BlogPost<Draft> {
    /// Publish post (Draft → Published)
    ///
    /// This is the primary valid transition from Draft state.
    pub fn publish(self) -> BlogPost<Published> {
        let published_at = Utc::now();

        tracing::info!(
            post_id = %self.id,
            tenant_id = %self.tenant_id,
            title = %self.title,
            "Blog post: Draft → Published"
        );

        BlogPost {
            id: self.id,
            tenant_id: self.tenant_id,
            author_id: self.author_id,
            title: self.title,
            slug: self.slug,
            locale: self.locale,
            category_id: self.category_id,
            tags: self.tags,
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

    /// Set title
    pub fn set_title(mut self, title: String) -> Self {
        self.title = title;
        self.state.updated_at = Utc::now();
        self
    }

    /// Set category
    pub fn set_category(mut self, category_id: Uuid) -> Self {
        self.category_id = Some(category_id);
        self.state.updated_at = Utc::now();
        self
    }

    /// Set tags
    pub fn set_tags(mut self, tags: Vec<String>) -> Self {
        self.tags = tags;
        self.state.updated_at = Utc::now();
        self
    }
}

// ============================================================================
// Transitions: Published
// ============================================================================

impl BlogPost<Published> {
    /// Archive published post (Published → Archived)
    pub fn archive(self, reason: String) -> BlogPost<Archived> {
        let archived_at = Utc::now();

        tracing::info!(
            post_id = %self.id,
            tenant_id = %self.tenant_id,
            reason = %reason,
            "Blog post: Published → Archived"
        );

        BlogPost {
            id: self.id,
            tenant_id: self.tenant_id,
            author_id: self.author_id,
            title: self.title,
            slug: self.slug,
            locale: self.locale,
            category_id: self.category_id,
            tags: self.tags,
            state: Archived {
                archived_at,
                reason,
            },
        }
    }

    /// Unpublish post (Published → Draft)
    pub fn unpublish(self) -> BlogPost<Draft> {
        let now = Utc::now();

        tracing::info!(
            post_id = %self.id,
            tenant_id = %self.tenant_id,
            "Blog post: Published → Draft (unpublished)"
        );

        BlogPost {
            id: self.id,
            tenant_id: self.tenant_id,
            author_id: self.author_id,
            title: self.title,
            slug: self.slug,
            locale: self.locale,
            category_id: self.category_id,
            tags: self.tags,
            state: Draft {
                created_at: self.state.published_at,
                updated_at: now,
            },
        }
    }

    /// Update published post
    pub fn update(mut self) -> Self {
        self.state.updated_at = Utc::now();
        self
    }
}

// ============================================================================
// Transitions: Archived
// ============================================================================

impl BlogPost<Archived> {
    /// Restore archived post to draft (Archived → Draft)
    ///
    /// Allows restoring archived posts for editing.
    pub fn restore_to_draft(self) -> BlogPost<Draft> {
        let now = Utc::now();

        tracing::info!(
            post_id = %self.id,
            tenant_id = %self.tenant_id,
            "Blog post: Archived → Draft (restored)"
        );

        BlogPost {
            id: self.id,
            tenant_id: self.tenant_id,
            author_id: self.author_id,
            title: self.title,
            slug: self.slug,
            locale: self.locale,
            category_id: self.category_id,
            tags: self.tags,
            state: Draft {
                created_at: self.state.archived_at,
                updated_at: now,
            },
        }
    }
}

// ============================================================================
// Common Methods (all states)
// ============================================================================

impl<S> BlogPost<S> {
    /// Get post ID
    pub fn id(&self) -> Uuid {
        self.id
    }

    /// Get tenant ID
    pub fn tenant_id(&self) -> Uuid {
        self.tenant_id
    }

    /// Get author ID
    pub fn author_id(&self) -> Uuid {
        self.author_id
    }

    /// Get title
    pub fn title(&self) -> &str {
        &self.title
    }

    /// Get slug
    pub fn slug(&self) -> &str {
        &self.slug
    }

    /// Get locale
    pub fn locale(&self) -> &str {
        &self.locale
    }
}

// ============================================================================
// Blog Post Status Enum (for database compatibility)
// ============================================================================

/// Blog post status enum for database storage
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sea_orm::EnumIter, ToSchema, Default,
)]
#[serde(rename_all = "lowercase")]
pub enum BlogPostStatus {
    #[default]
    Draft,
    Published,
    Archived,
}

/// Convert type-safe state to database enum
pub trait ToBlogPostStatus {
    fn to_status(&self) -> BlogPostStatus;
}

impl ToBlogPostStatus for BlogPost<Draft> {
    fn to_status(&self) -> BlogPostStatus {
        BlogPostStatus::Draft
    }
}

impl ToBlogPostStatus for BlogPost<Published> {
    fn to_status(&self) -> BlogPostStatus {
        BlogPostStatus::Published
    }
}

impl ToBlogPostStatus for BlogPost<Archived> {
    fn to_status(&self) -> BlogPostStatus {
        BlogPostStatus::Archived
    }
}

// ============================================================================
// Blog Comment State Machine
// ============================================================================

/// Comment status enum for database storage
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sea_orm::EnumIter, Default)]
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
    fn test_new_draft() {
        let id = Uuid::new_v4();
        let tenant_id = Uuid::new_v4();
        let author_id = Uuid::new_v4();

        let post = BlogPost::new_draft(
            id,
            tenant_id,
            author_id,
            "Test Post".to_string(),
            "test-post".to_string(),
            "en".to_string(),
        );

        assert_eq!(post.id, id);
        assert_eq!(post.tenant_id, tenant_id);
        assert_eq!(post.author_id, author_id);
        assert_eq!(post.title, "Test Post");
        assert_eq!(post.slug, "test-post");
        assert_eq!(post.locale, "en");
    }

    #[test]
    fn test_draft_to_published() {
        let post = BlogPost::new_draft(
            Uuid::new_v4(),
            Uuid::new_v4(),
            Uuid::new_v4(),
            "Test Post".to_string(),
            "test-post".to_string(),
            "en".to_string(),
        );

        let post = post.publish();

        assert!(post.state.published_at <= Utc::now());
        assert_eq!(post.to_status(), BlogPostStatus::Published);
    }

    #[test]
    fn test_published_to_archived() {
        let post = BlogPost::new_draft(
            Uuid::new_v4(),
            Uuid::new_v4(),
            Uuid::new_v4(),
            "Test Post".to_string(),
            "test-post".to_string(),
            "en".to_string(),
        )
        .publish();

        let reason = "Content outdated".to_string();
        let post = post.archive(reason.clone());

        assert_eq!(post.state.reason, reason);
        assert_eq!(post.to_status(), BlogPostStatus::Archived);
    }

    #[test]
    fn test_published_to_draft_via_unpublish() {
        let post = BlogPost::new_draft(
            Uuid::new_v4(),
            Uuid::new_v4(),
            Uuid::new_v4(),
            "Test Post".to_string(),
            "test-post".to_string(),
            "en".to_string(),
        )
        .publish()
        .unpublish();

        assert_eq!(post.to_status(), BlogPostStatus::Draft);
    }

    #[test]
    fn test_archived_to_draft() {
        let post = BlogPost::new_draft(
            Uuid::new_v4(),
            Uuid::new_v4(),
            Uuid::new_v4(),
            "Test Post".to_string(),
            "test-post".to_string(),
            "en".to_string(),
        )
        .publish()
        .archive("Test".to_string())
        .restore_to_draft();

        assert_eq!(post.to_status(), BlogPostStatus::Draft);
    }

    #[test]
    fn test_update_timestamps() {
        let post = BlogPost::new_draft(
            Uuid::new_v4(),
            Uuid::new_v4(),
            Uuid::new_v4(),
            "Test Post".to_string(),
            "test-post".to_string(),
            "en".to_string(),
        );

        let created_at = post.state.created_at;
        std::thread::sleep(std::time::Duration::from_millis(10));

        let post = post.update();

        assert!(post.state.updated_at > created_at);
    }

    #[test]
    fn test_set_category_and_tags() {
        let category_id = Uuid::new_v4();
        let post = BlogPost::new_draft(
            Uuid::new_v4(),
            Uuid::new_v4(),
            Uuid::new_v4(),
            "Test Post".to_string(),
            "test-post".to_string(),
            "en".to_string(),
        )
        .set_category(category_id)
        .set_tags(vec!["rust".to_string(), "blog".to_string()]);

        assert_eq!(post.category_id, Some(category_id));
        assert_eq!(post.tags, vec!["rust", "blog"]);
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

    // Compile-time safety tests (these should NOT compile if uncommented)

    // #[test]
    // fn test_invalid_draft_to_archived() {
    //     let post = BlogPost::new_draft(/* ... */);
    //     // ❌ Compile error: no method `archive` on `BlogPost<Draft>`
    //     let post = post.archive("test".to_string());
    // }

    // #[test]
    // fn test_invalid_archived_to_published() {
    //     let post = /* ... archived post ... */;
    //     // ❌ Compile error: no method `publish` on `BlogPost<Archived>`
    //     let post = post.publish();
    // }
}
