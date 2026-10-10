use crate::error::ContentRevisionError;
use async_trait::async_trait;
use rustok_revisions::{Revision, RevisionDiff, Revisionable};
use uuid::Uuid;

/// Public API for content revision tracking.
///
/// This trait defines the interface that business modules use to interact
/// with the revision system.
#[async_trait]
pub trait ContentRevisionApi: Send + Sync {
    /// Track an update to content.
    async fn track_update<T: Revisionable + Send + Sync>(
        &self,
        tenant_id: Uuid,
        content_id: Uuid,
        locale: &str,
        old_content: &T,
        new_content: &T,
        user_id: Uuid,
    ) -> Result<Option<Revision>, ContentRevisionError>;

    /// Track content creation.
    async fn track_create<T: Revisionable + Send + Sync>(
        &self,
        tenant_id: Uuid,
        content_id: Uuid,
        locale: &str,
        content: &T,
        user_id: Uuid,
    ) -> Result<Option<Revision>, ContentRevisionError>;

    /// List all revisions for a content item.
    async fn list_revisions(
        &self,
        tenant_id: Uuid,
        content_type: &str,
        content_id: Uuid,
        locale: &str,
    ) -> Result<Vec<Revision>, ContentRevisionError>;

    /// Get content at a specific revision.
    async fn get_content_at_revision<T: Revisionable + Send + Sync>(
        &self,
        tenant_id: Uuid,
        content_id: Uuid,
        locale: &str,
        target_revision: i32,
        current_content: &T,
    ) -> Result<T, ContentRevisionError>;

    /// Restore content to a previous revision.
    async fn restore_revision<T: Revisionable + Send + Sync>(
        &self,
        tenant_id: Uuid,
        content_id: Uuid,
        locale: &str,
        target_revision: i32,
        current_content: &T,
        restored_by: Uuid,
    ) -> Result<T, ContentRevisionError>;

    /// Create a named version (snapshot).
    async fn create_named_version<T: Revisionable + Send + Sync>(
        &self,
        tenant_id: Uuid,
        content_id: Uuid,
        locale: &str,
        current_content: &T,
        version_name: &str,
        created_by: Uuid,
    ) -> Result<Revision, ContentRevisionError>;

    /// List all named versions for a content item.
    async fn list_named_versions(
        &self,
        tenant_id: Uuid,
        content_type: &str,
        content_id: Uuid,
        locale: &str,
    ) -> Result<Vec<Revision>, ContentRevisionError>;

    /// Get diff between two revisions.
    async fn diff_revisions(
        &self,
        tenant_id: Uuid,
        content_type: &str,
        content_id: Uuid,
        locale: &str,
        from_revision: i32,
        to_revision: i32,
    ) -> Result<RevisionDiff, ContentRevisionError>;

    /// Check if tracking is enabled for a content type.
    fn is_enabled(&self, content_type: &str) -> bool;
}
