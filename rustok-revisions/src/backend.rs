//! Backend trait for revision storage.

use async_trait::async_trait;
use uuid::Uuid;

use crate::{Revision, RevisionError};

/// Trait for revision storage backends.
///
/// Implementations of this trait provide the actual storage mechanism
/// for revisions (e.g., database, file system, etc.).
#[async_trait]
pub trait RevisionBackend: Send + Sync {
    /// Create a new revision.
    async fn create_revision(&self, revision: &Revision) -> Result<Uuid, RevisionError>;

    /// Get a revision by ID.
    async fn get_revision(&self, id: Uuid) -> Result<Option<Revision>, RevisionError>;

    /// Get the latest revision for content.
    async fn get_latest_revision(
        &self,
        tenant_id: Uuid,
        content_id: Uuid,
        locale: &str,
    ) -> Result<Option<Revision>, RevisionError>;

    /// Get a revision by number.
    async fn get_revision_by_number(
        &self,
        tenant_id: Uuid,
        content_id: Uuid,
        locale: &str,
        revision_number: i64,
    ) -> Result<Option<Revision>, RevisionError>;

    /// List revisions for content.
    async fn list_revisions(
        &self,
        tenant_id: Uuid,
        content_id: Uuid,
        locale: &str,
        limit: Option<usize>,
        offset: Option<usize>,
    ) -> Result<Vec<Revision>, RevisionError>;

    /// List revisions with filters.
    async fn list_revisions_filtered(
        &self,
        tenant_id: Uuid,
        content_id: Option<Uuid>,
        content_type: Option<&str>,
        locale: Option<&str>,
        limit: Option<usize>,
        offset: Option<usize>,
    ) -> Result<Vec<Revision>, RevisionError>;

    /// Count revisions for content.
    async fn count_revisions(
        &self,
        tenant_id: Uuid,
        content_id: Uuid,
        locale: &str,
    ) -> Result<i64, RevisionError>;

    /// Delete a revision.
    async fn delete_revision(&self, id: Uuid) -> Result<(), RevisionError>;

    /// Delete old revisions based on retention policy.
    async fn delete_old_revisions(
        &self,
        tenant_id: Uuid,
        content_id: Uuid,
        locale: &str,
        keep_count: usize,
    ) -> Result<usize, RevisionError>;

    /// Delete revisions older than a certain date.
    async fn delete_revisions_older_than(
        &self,
        tenant_id: Uuid,
        content_id: Uuid,
        locale: &str,
        days: u32,
    ) -> Result<usize, RevisionError>;

    /// Update revision version name.
    async fn update_version_name(
        &self,
        id: Uuid,
        version_name: Option<String>,
    ) -> Result<(), RevisionError>;

    /// Get all named versions for content.
    async fn get_named_versions(
        &self,
        tenant_id: Uuid,
        content_id: Uuid,
        locale: &str,
    ) -> Result<Vec<Revision>, RevisionError>;
}
