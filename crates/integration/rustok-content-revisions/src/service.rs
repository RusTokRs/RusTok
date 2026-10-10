use crate::config::ContentRevisionConfig;
use crate::error::ContentRevisionError;
use async_trait::async_trait;
use rustok_revisions::{
    ChangeSource, Revision, RevisionDiff, RevisionEvent, RevisionMetadata,
    RevisionService, RevisionTracker, Revisionable, RetentionPolicy, SeaOrmBackend,
};
use sea_orm::DatabaseConnection;
use uuid::Uuid;

/// Service for managing content revisions across RusTok modules.
///
/// This is the main entry point for tracking revisions in business modules.
pub struct ContentRevisionService {
    revision_service: RevisionService,
    config: ContentRevisionConfig,
    db: DatabaseConnection,
}

impl ContentRevisionService {
    /// Create a new content revision service.
    pub fn new(db: DatabaseConnection, config: ContentRevisionConfig) -> Self {
        let backend = SeaOrmBackend::new(db.clone());
        let revision_service = RevisionService::new(Box::new(backend));

        Self {
            revision_service,
            config,
            db,
        }
    }

    /// Track an update to content.
    ///
    /// This is the main method called by business modules when content is updated.
    pub async fn track_update<T: Revisionable>(
        &self,
        tenant_id: Uuid,
        content_id: Uuid,
        locale: &str,
        old_content: &T,
        new_content: &T,
        user_id: Uuid,
    ) -> Result<Option<Revision>, ContentRevisionError> {
        let content_type = T::content_type();

        // Check if tracking is enabled
        if !self.config.is_enabled(content_type) {
            return Ok(None);
        }

        // Get configuration for this content type
        let type_config = self
            .config
            .get_content_type(content_type)
            .ok_or_else(|| ContentRevisionError::ContentTypeNotConfigured(content_type.to_string()))?;

        // Check condition if specified
        if let (Some(field), Some(value)) = (&type_config.condition_field, &type_config.condition_value) {
            let new_json = new_content.to_revision_json();
            if let Some(field_value) = new_json.get(field) {
                if field_value.as_str() != Some(value.as_str()) {
                    // Condition not met, skip tracking
                    return Ok(None);
                }
            }
        }

        // Create tracker with configuration
        let tracker = RevisionTracker::builder()
            .enabled(true)
            .track_on(type_config.track_on.clone())
            .retention(type_config.retention.clone())
            .build();

        // Create revision
        let revision = self
            .revision_service
            .create_revision_with_tracker(
                tenant_id,
                content_id,
                locale,
                old_content,
                new_content,
                user_id,
                &tracker,
                RevisionEvent::Update,
            )
            .await?;

        // Apply retention policy
        if revision.is_some() {
            self.revision_service
                .apply_retention_policy_for_type::<T>(
                    tenant_id,
                    content_id,
                    locale,
                    &type_config.retention,
                )
                .await?;
        }

        Ok(revision)
    }

    /// Track content creation.
    pub async fn track_create<T: Revisionable>(
        &self,
        tenant_id: Uuid,
        content_id: Uuid,
        locale: &str,
        content: &T,
        user_id: Uuid,
    ) -> Result<Option<Revision>, ContentRevisionError> {
        let content_type = T::content_type();

        if !self.config.is_enabled(content_type) {
            return Ok(None);
        }

        let type_config = self
            .config
            .get_content_type(content_type)
            .ok_or_else(|| ContentRevisionError::ContentTypeNotConfigured(content_type.to_string()))?;

        // Only track create if configured to do so
        if !type_config.track_on.contains(&RevisionEvent::Create) {
            return Ok(None);
        }

        // Create a revision with empty old content
        let tracker = RevisionTracker::builder()
            .enabled(true)
            .track_on(type_config.track_on.clone())
            .retention(type_config.retention.clone())
            .build();

        // For create, we pass the same content as old and new
        // The delta will be empty, but we still create a revision record
        let revision = self
            .revision_service
            .create_revision_with_tracker(
                tenant_id,
                content_id,
                locale,
                content,
                content,
                user_id,
                &tracker,
                RevisionEvent::Create,
            )
            .await?;

        Ok(revision)
    }

    /// List all revisions for a content item.
    pub async fn list_revisions(
        &self,
        tenant_id: Uuid,
        content_type: &str,
        content_id: Uuid,
        locale: &str,
    ) -> Result<Vec<Revision>, ContentRevisionError> {
        let revisions = self
            .revision_service
            .list_revisions(tenant_id, content_type, content_id, locale)
            .await?;

        Ok(revisions)
    }

    /// Get content at a specific revision.
    pub async fn get_content_at_revision<T: Revisionable>(
        &self,
        tenant_id: Uuid,
        content_id: Uuid,
        locale: &str,
        target_revision: i32,
        current_content: &T,
    ) -> Result<T, ContentRevisionError> {
        let content = self
            .revision_service
            .get_content_at_revision(tenant_id, content_id, locale, target_revision, current_content)
            .await?;

        Ok(content)
    }

    /// Restore content to a previous revision.
    pub async fn restore_revision<T: Revisionable>(
        &self,
        tenant_id: Uuid,
        content_id: Uuid,
        locale: &str,
        target_revision: i32,
        current_content: &T,
        restored_by: Uuid,
    ) -> Result<T, ContentRevisionError> {
        let restored = self
            .revision_service
            .restore_revision(tenant_id, content_id, locale, target_revision, current_content, restored_by)
            .await?;

        Ok(restored)
    }

    /// Create a named version (snapshot).
    pub async fn create_named_version<T: Revisionable>(
        &self,
        tenant_id: Uuid,
        content_id: Uuid,
        locale: &str,
        current_content: &T,
        version_name: &str,
        created_by: Uuid,
    ) -> Result<Revision, ContentRevisionError> {
        let revision = self
            .revision_service
            .create_named_version(tenant_id, content_id, locale, current_content, version_name, created_by)
            .await?;

        Ok(revision)
    }

    /// List all named versions for a content item.
    pub async fn list_named_versions(
        &self,
        tenant_id: Uuid,
        content_type: &str,
        content_id: Uuid,
        locale: &str,
    ) -> Result<Vec<Revision>, ContentRevisionError> {
        let versions = self
            .revision_service
            .list_named_versions(tenant_id, content_type, content_id, locale)
            .await?;

        Ok(versions)
    }

    /// Get diff between two revisions.
    pub async fn diff_revisions(
        &self,
        tenant_id: Uuid,
        content_type: &str,
        content_id: Uuid,
        locale: &str,
        from_revision: i32,
        to_revision: i32,
    ) -> Result<RevisionDiff, ContentRevisionError> {
        let diff = self
            .revision_service
            .diff_revisions(tenant_id, content_type, content_id, locale, from_revision, to_revision)
            .await?;

        Ok(diff)
    }

    /// Get all diffs between consecutive revisions.
    pub async fn all_diffs(
        &self,
        tenant_id: Uuid,
        content_type: &str,
        content_id: Uuid,
        locale: &str,
    ) -> Result<Vec<RevisionDiff>, ContentRevisionError> {
        let diffs = self
            .revision_service
            .all_diffs(tenant_id, content_type, content_id, locale)
            .await?;

        Ok(diffs)
    }

    /// Get configuration for a content type.
    pub fn get_config(&self, content_type: &str) -> Option<&crate::config::ContentTypeConfig> {
        self.config.get_content_type(content_type)
    }

    /// Check if tracking is enabled for a content type.
    pub fn is_enabled(&self, content_type: &str) -> bool {
        self.config.is_enabled(content_type)
    }
}
