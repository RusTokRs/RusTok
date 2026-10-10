//! Traits for revisionable content.

use serde_json::Value;

/// Trait for types that can be tracked for revisions.
///
/// This trait must be implemented for any content type that should be
/// tracked by the revision system.
///
/// # Example
///
/// ```rust,ignore
/// use rustok_revisions::Revisionable;
/// use serde::{Serialize, Deserialize};
///
/// #[derive(Clone, Serialize, Deserialize)]
/// struct Post {
///     title: String,
///     content: String,
///     views: i32,
/// }
///
/// impl Revisionable for Post {
///     fn content_type() -> &'static str {
///         "blog_post"
///     }
///
///     fn to_revision_json(&self) -> Value {
///         serde_json::to_value(self).unwrap()
///     }
/// }
/// ```
pub trait Revisionable: Clone + Send + Sync {
    /// Returns the content type identifier.
    ///
    /// This is used to categorize revisions in the database.
    /// Examples: "blog_post", "forum_topic", "product"
    fn content_type() -> &'static str;

    /// Converts the content to JSON for storage.
    ///
    /// This should return a complete representation of the content
    /// that can be used to restore it later.
    fn to_revision_json(&self) -> Value;
}

/// Trait for configuring revision behavior per content type.
pub trait RevisionConfig: Revisionable {
    /// Returns the default retention policy for this content type.
    fn default_retention_policy() -> Option<RetentionPolicy> {
        None
    }

    /// Returns fields that should be ignored when computing diffs.
    fn ignored_fields() -> Vec<&'static str> {
        vec![]
    }
}

/// Retention policy for revisions.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum RetentionPolicy {
    /// Keep only the last N revisions.
    KeepLast(usize),

    /// Keep revisions for the last N days.
    KeepDays(u32),

    /// Keep all revisions (no automatic cleanup).
    KeepAll,

    /// Custom retention policy.
    Custom(String),
}
