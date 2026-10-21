use crate::api::ContentRevisionApi;
use crate::config::ContentRevisionConfig;
use crate::error::ContentRevisionError;
use async_trait::async_trait;
use rustok_revisions::{
    ChangeSource, Revision, RevisionDiff, RevisionEvent, RevisionService,
    RevisionTracker, Revisionable, SeaOrmBackend,
};
use sea_orm::DatabaseConnection;
use serde::de::DeserializeOwned;
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

    /// Direct access to database connection
    pub fn db(&self) -> &DatabaseConnection {
        &self.db
    }

    /// Get configuration for a content type.
    pub fn get_config(&self, content_type: &str) -> Option<&crate::config::ContentTypeConfig> {
        self.config.get_content_type(content_type)
    }

    /// Check if tracking is enabled for a content type.
    pub fn is_enabled(&self, content_type: &str) -> bool {
        self.config.is_enabled(content_type)
    }

    /// Track an update to content.
    pub async fn track_update<T: Revisionable>(
        &self,
        tenant_id: Uuid,
        content_id: Uuid,
        locale: &str,
        old_content: &T,
        new_content: &T,
        user_id: Uuid,
    ) -> Result<Option<Revision>, ContentRevisionError> {
        if tenant_id.is_nil() {
            return Err(ContentRevisionError::TenantMismatch);
        }
        if content_id.is_nil() {
            return Err(ContentRevisionError::Configuration(
                "content_id must not be nil".to_string(),
            ));
        }
        let content_type = T::content_type();

        if !self.config.is_enabled(content_type) {
            return Ok(None);
        }

        let type_config = self.config.get_content_type(content_type).ok_or_else(|| {
            ContentRevisionError::ContentTypeNotConfigured(content_type.to_string())
        })?;

        if let (Some(field), Some(value)) =
            (&type_config.condition_field, &type_config.condition_value)
        {
            let new_json = new_content.to_revision_json();
            if let Some(field_value) = new_json.get(field) {
                if field_value.as_str() != Some(value.as_str()) {
                    return Ok(None);
                }
            }
        }

        let tracker = RevisionTracker::builder()
            .enabled(true)
            .track_on(type_config.track_on.clone())
            .retention(type_config.retention.clone())
            .build();

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
        if tenant_id.is_nil() {
            return Err(ContentRevisionError::TenantMismatch);
        }
        if content_id.is_nil() {
            return Err(ContentRevisionError::Configuration(
                "content_id must not be nil".to_string(),
            ));
        }
        let content_type = T::content_type();

        if !self.config.is_enabled(content_type) {
            return Ok(None);
        }

        let type_config = self.config.get_content_type(content_type).ok_or_else(|| {
            ContentRevisionError::ContentTypeNotConfigured(content_type.to_string())
        })?;

        if !type_config.track_on.contains(&RevisionEvent::Create) {
            return Ok(None);
        }

        let tracker = RevisionTracker::builder()
            .enabled(true)
            .track_on(type_config.track_on.clone())
            .retention(type_config.retention.clone())
            .build();

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
        _content_type: &str,
        content_id: Uuid,
        locale: &str,
    ) -> Result<Vec<Revision>, ContentRevisionError> {
        if tenant_id.is_nil() {
            return Err(ContentRevisionError::TenantMismatch);
        }
        if content_id.is_nil() {
            return Err(ContentRevisionError::Configuration(
                "content_id must not be nil".to_string(),
            ));
        }
        let revisions = self
            .revision_service
            .list_revisions(tenant_id, content_id, locale, None, None)
            .await?;

        Ok(revisions)
    }

    /// Get content at a specific revision.
    pub async fn get_content_at_revision<T: Revisionable + DeserializeOwned>(
        &self,
        tenant_id: Uuid,
        content_id: Uuid,
        locale: &str,
        target_revision: i64,
        _current_content: &T,
    ) -> Result<T, ContentRevisionError> {
        if tenant_id.is_nil() {
            return Err(ContentRevisionError::TenantMismatch);
        }
        if content_id.is_nil() {
            return Err(ContentRevisionError::Configuration(
                "content_id must not be nil".to_string(),
            ));
        }
        let revision = self
            .revision_service
            .get_revision_by_number(tenant_id, content_id, locale, target_revision)
            .await?
            .ok_or_else(|| {
                rustok_revisions::RevisionError::NotFound(format!(
                    "Revision {} not found",
                    target_revision
                ))
            })?;

        let content: T = serde_json::from_value(revision.content)
            .map_err(|e| rustok_revisions::RevisionError::Serialization(e.to_string()))?;

        Ok(content)
    }

    /// Restore content to a previous revision.
    pub async fn restore_revision<T: Revisionable + DeserializeOwned>(
        &self,
        tenant_id: Uuid,
        content_id: Uuid,
        locale: &str,
        target_revision: i64,
        _current_content: &T,
        restored_by: Uuid,
    ) -> Result<T, ContentRevisionError> {
        if tenant_id.is_nil() {
            return Err(ContentRevisionError::TenantMismatch);
        }
        if content_id.is_nil() {
            return Err(ContentRevisionError::Configuration(
                "content_id must not be nil".to_string(),
            ));
        }
        let target = self
            .revision_service
            .get_revision_by_number(tenant_id, content_id, locale, target_revision)
            .await?
            .ok_or_else(|| {
                rustok_revisions::RevisionError::NotFound(format!(
                    "Revision {} not found",
                    target_revision
                ))
            })?;

        let (restored, _revision) = self
            .revision_service
            .restore_revision::<T>(
                tenant_id,
                content_id,
                locale,
                target.id,
                restored_by,
                ChangeSource::Admin,
            )
            .await?;

        Ok(restored)
    }

    /// Create a named version (snapshot).
    pub async fn create_named_version<T: Revisionable>(
        &self,
        tenant_id: Uuid,
        content_id: Uuid,
        locale: &str,
        _current_content: &T,
        version_name: &str,
        _created_by: Uuid,
    ) -> Result<Revision, ContentRevisionError> {
        if tenant_id.is_nil() {
            return Err(ContentRevisionError::TenantMismatch);
        }
        if content_id.is_nil() {
            return Err(ContentRevisionError::Configuration(
                "content_id must not be nil".to_string(),
            ));
        }
        let latest = self
            .revision_service
            .get_latest_revision(tenant_id, content_id, locale)
            .await?
            .ok_or_else(|| {
                rustok_revisions::RevisionError::NotFound("No revision found to name".to_string())
            })?;

        self.revision_service
            .create_named_version(
                tenant_id,
                content_id,
                locale,
                latest.id,
                version_name,
            )
            .await?;

        let mut named = latest;
        named.version_name = Some(version_name.to_string());
        Ok(named)
    }

    /// List all named versions for a content item.
    pub async fn list_named_versions(
        &self,
        tenant_id: Uuid,
        _content_type: &str,
        content_id: Uuid,
        locale: &str,
    ) -> Result<Vec<Revision>, ContentRevisionError> {
        if tenant_id.is_nil() {
            return Err(ContentRevisionError::TenantMismatch);
        }
        if content_id.is_nil() {
            return Err(ContentRevisionError::Configuration(
                "content_id must not be nil".to_string(),
            ));
        }
        let versions = self
            .revision_service
            .get_named_versions(tenant_id, content_id, locale)
            .await?;

        Ok(versions)
    }

    /// Get diff between two revisions.
    pub async fn diff_revisions(
        &self,
        tenant_id: Uuid,
        _content_type: &str,
        content_id: Uuid,
        locale: &str,
        from_revision: i64,
        to_revision: i64,
    ) -> Result<RevisionDiff, ContentRevisionError> {
        if tenant_id.is_nil() {
            return Err(ContentRevisionError::TenantMismatch);
        }
        if content_id.is_nil() {
            return Err(ContentRevisionError::Configuration(
                "content_id must not be nil".to_string(),
            ));
        }
        let from = self
            .revision_service
            .get_revision_by_number(tenant_id, content_id, locale, from_revision)
            .await?
            .ok_or_else(|| {
                rustok_revisions::RevisionError::NotFound(format!(
                    "Revision {} not found",
                    from_revision
                ))
            })?;
        let to = self
            .revision_service
            .get_revision_by_number(tenant_id, content_id, locale, to_revision)
            .await?
            .ok_or_else(|| {
                rustok_revisions::RevisionError::NotFound(format!(
                    "Revision {} not found",
                    to_revision
                ))
            })?;

        let diff = self
            .revision_service
            .compare_revisions(from.id, to.id)
            .await?;

        Ok(diff)
    }

    /// Get all diffs between consecutive revisions.
    pub async fn all_diffs(
        &self,
        tenant_id: Uuid,
        _content_type: &str,
        content_id: Uuid,
        locale: &str,
    ) -> Result<Vec<RevisionDiff>, ContentRevisionError> {
        if tenant_id.is_nil() {
            return Err(ContentRevisionError::TenantMismatch);
        }
        if content_id.is_nil() {
            return Err(ContentRevisionError::Configuration(
                "content_id must not be nil".to_string(),
            ));
        }
        let revisions = self
            .revision_service
            .list_revisions(tenant_id, content_id, locale, None, None)
            .await?;

        let mut diffs = Vec::new();
        for window in revisions.windows(2) {
            let diff = self
                .revision_service
                .compare_revisions(window[1].id, window[0].id)
                .await?;
            diffs.push(diff);
        }

        Ok(diffs)
    }
}

#[async_trait]
impl ContentRevisionApi for ContentRevisionService {
    async fn track_update<T: Revisionable + Send + Sync>(
        &self,
        tenant_id: Uuid,
        content_id: Uuid,
        locale: &str,
        old_content: &T,
        new_content: &T,
        user_id: Uuid,
    ) -> Result<Option<Revision>, ContentRevisionError> {
        self.track_update(
            tenant_id,
            content_id,
            locale,
            old_content,
            new_content,
            user_id,
        )
        .await
    }

    async fn track_create<T: Revisionable + Send + Sync>(
        &self,
        tenant_id: Uuid,
        content_id: Uuid,
        locale: &str,
        content: &T,
        user_id: Uuid,
    ) -> Result<Option<Revision>, ContentRevisionError> {
        self.track_create(tenant_id, content_id, locale, content, user_id)
            .await
    }

    async fn list_revisions(
        &self,
        tenant_id: Uuid,
        content_type: &str,
        content_id: Uuid,
        locale: &str,
    ) -> Result<Vec<Revision>, ContentRevisionError> {
        self.list_revisions(tenant_id, content_type, content_id, locale)
            .await
    }

    async fn get_content_at_revision<T: Revisionable + DeserializeOwned + Send + Sync>(
        &self,
        tenant_id: Uuid,
        content_id: Uuid,
        locale: &str,
        target_revision: i64,
        current_content: &T,
    ) -> Result<T, ContentRevisionError> {
        self.get_content_at_revision(
            tenant_id,
            content_id,
            locale,
            target_revision,
            current_content,
        )
        .await
    }

    async fn restore_revision<T: Revisionable + DeserializeOwned + Send + Sync>(
        &self,
        tenant_id: Uuid,
        content_id: Uuid,
        locale: &str,
        target_revision: i64,
        current_content: &T,
        restored_by: Uuid,
    ) -> Result<T, ContentRevisionError> {
        self.restore_revision(
            tenant_id,
            content_id,
            locale,
            target_revision,
            current_content,
            restored_by,
        )
        .await
    }

    async fn create_named_version<T: Revisionable + Send + Sync>(
        &self,
        tenant_id: Uuid,
        content_id: Uuid,
        locale: &str,
        current_content: &T,
        version_name: &str,
        created_by: Uuid,
    ) -> Result<Revision, ContentRevisionError> {
        self.create_named_version(
            tenant_id,
            content_id,
            locale,
            current_content,
            version_name,
            created_by,
        )
        .await
    }

    async fn list_named_versions(
        &self,
        tenant_id: Uuid,
        content_type: &str,
        content_id: Uuid,
        locale: &str,
    ) -> Result<Vec<Revision>, ContentRevisionError> {
        self.list_named_versions(tenant_id, content_type, content_id, locale)
            .await
    }

    async fn diff_revisions(
        &self,
        tenant_id: Uuid,
        content_type: &str,
        content_id: Uuid,
        locale: &str,
        from_revision: i64,
        to_revision: i64,
    ) -> Result<RevisionDiff, ContentRevisionError> {
        self.diff_revisions(
            tenant_id,
            content_type,
            content_id,
            locale,
            from_revision,
            to_revision,
        )
        .await
    }

    async fn all_diffs(
        &self,
        tenant_id: Uuid,
        content_type: &str,
        content_id: Uuid,
        locale: &str,
    ) -> Result<Vec<RevisionDiff>, ContentRevisionError> {
        self.all_diffs(tenant_id, content_type, content_id, locale)
            .await
    }

    fn is_enabled(&self, content_type: &str) -> bool {
        self.is_enabled(content_type)
    }
}
