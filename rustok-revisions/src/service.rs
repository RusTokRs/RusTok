//! Main service for revision management.

use chrono::Utc;
use serde_json::Value;
use uuid::Uuid;

use crate::{
    compute_diff, ChangeSource, Revision, RevisionDiff, RevisionError, RevisionEvent,
    RevisionMetadata, RevisionTracker, Revisionable, RetentionPolicy,
};
use crate::backend::RevisionBackend;

/// Main service for managing content revisions.
pub struct RevisionService {
    backend: Box<dyn RevisionBackend>,
}

impl RevisionService {
    /// Create a new revision service with the given backend.
    pub fn new(backend: Box<dyn RevisionBackend>) -> Self {
        Self { backend }
    }

    /// Create a revision for content update.
    pub async fn create_revision_with_tracker<T: Revisionable>(
        &self,
        tenant_id: Uuid,
        content_id: Uuid,
        locale: &str,
        _old_content: &T,
        new_content: &T,
        user_id: Uuid,
        tracker: &RevisionTracker,
        event: RevisionEvent,
    ) -> Result<Option<Revision>, RevisionError> {
        // Check if tracking is enabled
        if !tracker.enabled {
            return Ok(None);
        }

        // Check if this event type should be tracked
        if !tracker.track_on.contains(&event) {
            return Ok(None);
        }

        // Get the latest revision number
        let latest = self
            .backend
            .get_latest_revision(tenant_id, content_id, locale)
            .await?;

        let revision_number = latest.as_ref().map(|r| r.revision_number + 1).unwrap_or(1);
        let parent_revision_id = latest.as_ref().map(|r| r.id);

        // Create the revision
        let revision = Revision {
            id: Uuid::new_v4(),
            tenant_id,
            content_id,
            content_type: T::content_type().to_string(),
            locale: locale.to_string(),
            revision_number,
            parent_revision_id,
            event,
            content: new_content.to_revision_json(),
            metadata: RevisionMetadata {
                user_id,
                source: tracker.source.clone(),
                summary: None,
                ip_address: None,
                user_agent: None,
                custom: Value::Null,
            },
            created_at: Utc::now(),
            version_name: None,
        };

        // Store the revision
        self.backend.create_revision(&revision).await?;

        // Create snapshot if needed
        if tracker.auto_snapshot {
            if let Some(interval) = tracker.snapshot_interval {
                if revision_number % interval as i64 == 0 {
                    self.create_named_version(
                        tenant_id,
                        content_id,
                        locale,
                        revision.id,
                        &format!("snapshot-{}", revision_number),
                    )
                    .await?;
                }
            }
        }

        Ok(Some(revision))
    }

    /// Create a revision for content creation.
    pub async fn create_revision_for_create<T: Revisionable>(
        &self,
        tenant_id: Uuid,
        content_id: Uuid,
        locale: &str,
        content: &T,
        user_id: Uuid,
        source: ChangeSource,
    ) -> Result<Revision, RevisionError> {
        let revision = Revision {
            id: Uuid::new_v4(),
            tenant_id,
            content_id,
            content_type: T::content_type().to_string(),
            locale: locale.to_string(),
            revision_number: 1,
            parent_revision_id: None,
            event: RevisionEvent::Create,
            content: content.to_revision_json(),
            metadata: RevisionMetadata {
                user_id,
                source,
                summary: None,
                ip_address: None,
                user_agent: None,
                custom: Value::Null,
            },
            created_at: Utc::now(),
            version_name: None,
        };

        self.backend.create_revision(&revision).await?;
        Ok(revision)
    }

    /// Get a revision by ID.
    pub async fn get_revision(&self, id: Uuid) -> Result<Option<Revision>, RevisionError> {
        self.backend.get_revision(id).await
    }

    /// Get the latest revision for content.
    pub async fn get_latest_revision(
        &self,
        tenant_id: Uuid,
        content_id: Uuid,
        locale: &str,
    ) -> Result<Option<Revision>, RevisionError> {
        self.backend
            .get_latest_revision(tenant_id, content_id, locale)
            .await
    }

    /// Get a revision by number.
    pub async fn get_revision_by_number(
        &self,
        tenant_id: Uuid,
        content_id: Uuid,
        locale: &str,
        revision_number: i64,
    ) -> Result<Option<Revision>, RevisionError> {
        self.backend
            .get_revision_by_number(tenant_id, content_id, locale, revision_number)
            .await
    }

    /// List revisions for content.
    pub async fn list_revisions(
        &self,
        tenant_id: Uuid,
        content_id: Uuid,
        locale: &str,
        limit: Option<usize>,
        offset: Option<usize>,
    ) -> Result<Vec<Revision>, RevisionError> {
        self.backend
            .list_revisions(tenant_id, content_id, locale, limit, offset)
            .await
    }

    /// Compute diff between two revisions.
    pub async fn compare_revisions(
        &self,
        from_id: Uuid,
        to_id: Uuid,
    ) -> Result<RevisionDiff, RevisionError> {
        let from = self
            .backend
            .get_revision(from_id)
            .await?
            .ok_or_else(|| RevisionError::NotFound(format!("Revision {} not found", from_id)))?;

        let to = self
            .backend
            .get_revision(to_id)
            .await?
            .ok_or_else(|| RevisionError::NotFound(format!("Revision {} not found", to_id)))?;

        compute_diff(&from, &to)
    }

    /// Restore content to a previous revision.
    pub async fn restore_revision<T: Revisionable + serde::de::DeserializeOwned>(
        &self,
        tenant_id: Uuid,
        content_id: Uuid,
        locale: &str,
        target_revision_id: Uuid,
        user_id: Uuid,
        source: ChangeSource,
    ) -> Result<(T, Revision), RevisionError> {
        // Get the target revision
        let target = self
            .backend
            .get_revision(target_revision_id)
            .await?
            .ok_or_else(|| {
                RevisionError::NotFound(format!("Revision {} not found", target_revision_id))
            })?;

        // Deserialize the content
        let restored: T = serde_json::from_value(target.content.clone())?;

        // Create a new revision for the restore
        let revision = Revision {
            id: Uuid::new_v4(),
            tenant_id,
            content_id,
            content_type: T::content_type().to_string(),
            locale: locale.to_string(),
            revision_number: self
                .backend
                .get_latest_revision(tenant_id, content_id, locale)
                .await?
                .map(|r| r.revision_number + 1)
                .unwrap_or(1),
            parent_revision_id: Some(target_revision_id),
            event: RevisionEvent::Restore,
            content: target.content.clone(),
            metadata: RevisionMetadata {
                user_id,
                source,
                summary: Some(format!("Restored to revision {}", target.revision_number)),
                ip_address: None,
                user_agent: None,
                custom: Value::Null,
            },
            created_at: Utc::now(),
            version_name: None,
        };

        self.backend.create_revision(&revision).await?;

        Ok((restored, revision))
    }

    /// Create a named version (snapshot).
    pub async fn create_named_version(
        &self,
        tenant_id: Uuid,
        content_id: Uuid,
        locale: &str,
        revision_id: Uuid,
        version_name: &str,
    ) -> Result<(), RevisionError> {
        // Verify the revision exists
        let revision = self
            .backend
            .get_revision(revision_id)
            .await?
            .ok_or_else(|| RevisionError::NotFound(format!("Revision {} not found", revision_id)))?;

        // Verify it belongs to the specified content
        if revision.tenant_id != tenant_id
            || revision.content_id != content_id
            || revision.locale != locale
        {
            return Err(RevisionError::InvalidOperation(
                "Revision does not belong to specified content".to_string(),
            ));
        }

        // Update the version name
        self.backend
            .update_version_name(revision_id, Some(version_name.to_string()))
            .await
    }

    /// Get all named versions for content.
    pub async fn get_named_versions(
        &self,
        tenant_id: Uuid,
        content_id: Uuid,
        locale: &str,
    ) -> Result<Vec<Revision>, RevisionError> {
        self.backend
            .get_named_versions(tenant_id, content_id, locale)
            .await
    }

    /// Apply retention policy for a content type.
    pub async fn apply_retention_policy_for_type<T: Revisionable>(
        &self,
        tenant_id: Uuid,
        content_id: Uuid,
        locale: &str,
        policy: &RetentionPolicy,
    ) -> Result<usize, RevisionError> {
        match policy {
            RetentionPolicy::KeepLast(count) => {
                self.backend
                    .delete_old_revisions(tenant_id, content_id, locale, *count)
                    .await
            }
            RetentionPolicy::KeepDays(days) => {
                self.backend
                    .delete_revisions_older_than(tenant_id, content_id, locale, *days)
                    .await
            }
            RetentionPolicy::KeepAll => Ok(0),
            RetentionPolicy::Custom(_) => {
                // Custom policies are not implemented in the base service
                Ok(0)
            }
        }
    }

    /// Delete a revision.
    pub async fn delete_revision(&self, id: Uuid) -> Result<(), RevisionError> {
        self.backend.delete_revision(id).await
    }

    /// Count revisions for content.
    pub async fn count_revisions(
        &self,
        tenant_id: Uuid,
        content_id: Uuid,
        locale: &str,
    ) -> Result<i64, RevisionError> {
        self.backend
            .count_revisions(tenant_id, content_id, locale)
            .await
    }
}
