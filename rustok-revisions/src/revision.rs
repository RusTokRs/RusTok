//! Revision data structures.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

/// A revision of content.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Revision {
    /// Unique identifier for this revision.
    pub id: Uuid,

    /// Tenant ID (for multi-tenant systems).
    pub tenant_id: Uuid,

    /// Content ID being revised.
    pub content_id: Uuid,

    /// Type of content (e.g., "blog_post").
    pub content_type: String,

    /// Locale of the content.
    pub locale: String,

    /// Sequential revision number.
    pub revision_number: i64,

    /// ID of the parent revision (None for first revision).
    pub parent_revision_id: Option<Uuid>,

    /// Type of event that created this revision.
    pub event: RevisionEvent,

    /// JSON representation of the content at this revision.
    pub content: Value,

    /// Metadata about the revision.
    pub metadata: RevisionMetadata,

    /// When this revision was created.
    pub created_at: DateTime<Utc>,

    /// Optional name for this revision (e.g., "v1.0").
    pub version_name: Option<String>,
}

/// Event types that can create revisions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RevisionEvent {
    /// Content was created.
    Create,

    /// Content was updated.
    Update,

    /// Content was deleted.
    Delete,

    /// Content was restored to a previous revision.
    Restore,

    /// Manual snapshot.
    Snapshot,
}

/// Metadata about a revision.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RevisionMetadata {
    /// User ID who created this revision.
    pub user_id: Uuid,

    /// Source of the change (e.g., "web", "api", "admin").
    pub source: ChangeSource,

    /// Optional summary of changes.
    pub summary: Option<String>,

    /// IP address of the user (if available).
    pub ip_address: Option<String>,

    /// User agent string (if available).
    pub user_agent: Option<String>,

    /// Additional custom metadata.
    pub custom: Value,
}

/// Source of a revision change.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ChangeSource {
    /// Change from web interface.
    Web,

    /// Change from API.
    Api,

    /// Change from admin panel.
    Admin,

    /// Change from background job.
    BackgroundJob,

    /// Change from import/migration.
    Import,

    /// Custom source.
    Custom(String),
}

/// Difference between two revisions.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RevisionDiff {
    /// ID of the "from" revision.
    pub from_revision_id: Uuid,

    /// ID of the "to" revision.
    pub to_revision_id: Uuid,

    /// Fields that were added.
    pub added: Value,

    /// Fields that were removed.
    pub removed: Value,

    /// Fields that were changed.
    pub changed: Value,
}
