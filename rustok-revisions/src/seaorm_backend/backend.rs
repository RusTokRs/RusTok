//! SeaORM backend implementation.

use async_trait::async_trait;
use chrono::{Duration, Utc};
use sea_orm::*;
use uuid::Uuid;

use crate::{
    ChangeSource, Revision, RevisionError, RevisionEvent, RevisionMetadata,
};
use crate::backend::RevisionBackend;
use super::entities;

/// SeaORM-based revision backend.
pub struct SeaOrmBackend {
    db: DatabaseConnection,
}

impl SeaOrmBackend {
    /// Create a new SeaORM backend.
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }
}

#[async_trait]
impl RevisionBackend for SeaOrmBackend {
    async fn create_revision(&self, revision: &Revision) -> Result<Uuid, RevisionError> {
        let model = entities::ActiveModel {
            id: Set(revision.id),
            tenant_id: Set(revision.tenant_id),
            content_id: Set(revision.content_id),
            content_type: Set(revision.content_type.clone()),
            locale: Set(revision.locale.clone()),
            revision_number: Set(revision.revision_number),
            parent_revision_id: Set(revision.parent_revision_id),
            event: Set(event_to_string(revision.event)),
            content: Set(revision.content.clone()),
            user_id: Set(revision.metadata.user_id),
            source: Set(source_to_string(&revision.metadata.source)),
            summary: Set(revision.metadata.summary.clone()),
            ip_address: Set(revision.metadata.ip_address.clone()),
            user_agent: Set(revision.metadata.user_agent.clone()),
            custom_metadata: Set(revision.metadata.custom.clone()),
            created_at: Set(revision.created_at),
            version_name: Set(revision.version_name.clone()),
        };

        entities::Entity::insert(model)
            .exec(&self.db)
            .await
            .map(|_| revision.id)
            .map_err(|e| RevisionError::Database(e.to_string()))
    }

    async fn get_revision(&self, id: Uuid) -> Result<Option<Revision>, RevisionError> {
        let model = entities::Entity::find_by_id(id)
            .one(&self.db)
            .await
            .map_err(|e| RevisionError::Database(e.to_string()))?;

        Ok(model.map(model_to_revision))
    }

    async fn get_latest_revision(
        &self,
        tenant_id: Uuid,
        content_id: Uuid,
        locale: &str,
    ) -> Result<Option<Revision>, RevisionError> {
        let model = entities::Entity::find()
            .filter(entities::Column::TenantId.eq(tenant_id))
            .filter(entities::Column::ContentId.eq(content_id))
            .filter(entities::Column::Locale.eq(locale))
            .order_by_desc(entities::Column::RevisionNumber)
            .one(&self.db)
            .await
            .map_err(|e| RevisionError::Database(e.to_string()))?;

        Ok(model.map(model_to_revision))
    }

    async fn get_revision_by_number(
        &self,
        tenant_id: Uuid,
        content_id: Uuid,
        locale: &str,
        revision_number: i64,
    ) -> Result<Option<Revision>, RevisionError> {
        let model = entities::Entity::find()
            .filter(entities::Column::TenantId.eq(tenant_id))
            .filter(entities::Column::ContentId.eq(content_id))
            .filter(entities::Column::Locale.eq(locale))
            .filter(entities::Column::RevisionNumber.eq(revision_number))
            .one(&self.db)
            .await
            .map_err(|e| RevisionError::Database(e.to_string()))?;

        Ok(model.map(model_to_revision))
    }

    async fn list_revisions(
        &self,
        tenant_id: Uuid,
        content_id: Uuid,
        locale: &str,
        limit: Option<usize>,
        offset: Option<usize>,
    ) -> Result<Vec<Revision>, RevisionError> {
        let mut query = entities::Entity::find()
            .filter(entities::Column::TenantId.eq(tenant_id))
            .filter(entities::Column::ContentId.eq(content_id))
            .filter(entities::Column::Locale.eq(locale))
            .order_by_desc(entities::Column::RevisionNumber);

        if let Some(limit) = limit {
            query = query.limit(limit as u64);
        }

        if let Some(offset) = offset {
            query = query.offset(offset as u64);
        }

        let models = query
            .all(&self.db)
            .await
            .map_err(|e| RevisionError::Database(e.to_string()))?;

        Ok(models.into_iter().map(model_to_revision).collect())
    }

    async fn list_revisions_filtered(
        &self,
        tenant_id: Uuid,
        content_id: Option<Uuid>,
        content_type: Option<&str>,
        locale: Option<&str>,
        limit: Option<usize>,
        offset: Option<usize>,
    ) -> Result<Vec<Revision>, RevisionError> {
        let mut query = entities::Entity::find()
            .filter(entities::Column::TenantId.eq(tenant_id));

        if let Some(content_id) = content_id {
            query = query.filter(entities::Column::ContentId.eq(content_id));
        }

        if let Some(content_type) = content_type {
            query = query.filter(entities::Column::ContentType.eq(content_type));
        }

        if let Some(locale) = locale {
            query = query.filter(entities::Column::Locale.eq(locale));
        }

        query = query.order_by_desc(entities::Column::RevisionNumber);

        if let Some(limit) = limit {
            query = query.limit(limit as u64);
        }

        if let Some(offset) = offset {
            query = query.offset(offset as u64);
        }

        let models = query
            .all(&self.db)
            .await
            .map_err(|e| RevisionError::Database(e.to_string()))?;

        Ok(models.into_iter().map(model_to_revision).collect())
    }

    async fn count_revisions(
        &self,
        tenant_id: Uuid,
        content_id: Uuid,
        locale: &str,
    ) -> Result<i64, RevisionError> {
        let count = entities::Entity::find()
            .filter(entities::Column::TenantId.eq(tenant_id))
            .filter(entities::Column::ContentId.eq(content_id))
            .filter(entities::Column::Locale.eq(locale))
            .count(&self.db)
            .await
            .map_err(|e| RevisionError::Database(e.to_string()))?;

        Ok(count as i64)
    }

    async fn delete_revision(&self, id: Uuid) -> Result<(), RevisionError> {
        entities::Entity::delete_by_id(id)
            .exec(&self.db)
            .await
            .map_err(|e| RevisionError::Database(e.to_string()))?;

        Ok(())
    }

    async fn delete_old_revisions(
        &self,
        tenant_id: Uuid,
        content_id: Uuid,
        locale: &str,
        keep_count: usize,
    ) -> Result<usize, RevisionError> {
        // Get the revision number to keep from
        let revisions = self
            .list_revisions(tenant_id, content_id, locale, Some(keep_count), None)
            .await?;

        if revisions.is_empty() {
            return Ok(0);
        }

        let min_revision_number = revisions.last().unwrap().revision_number;

        // Delete older revisions (excluding named versions)
        let delete_result = entities::Entity::delete_many()
            .filter(entities::Column::TenantId.eq(tenant_id))
            .filter(entities::Column::ContentId.eq(content_id))
            .filter(entities::Column::Locale.eq(locale))
            .filter(entities::Column::RevisionNumber.lt(min_revision_number))
            .filter(entities::Column::VersionName.is_null())
            .exec(&self.db)
            .await
            .map_err(|e| RevisionError::Database(e.to_string()))?;

        Ok(delete_result.rows_affected as usize)
    }

    async fn delete_revisions_older_than(
        &self,
        tenant_id: Uuid,
        content_id: Uuid,
        locale: &str,
        days: u32,
    ) -> Result<usize, RevisionError> {
        let cutoff_date = Utc::now() - Duration::days(days as i64);

        let delete_result = entities::Entity::delete_many()
            .filter(entities::Column::TenantId.eq(tenant_id))
            .filter(entities::Column::ContentId.eq(content_id))
            .filter(entities::Column::Locale.eq(locale))
            .filter(entities::Column::CreatedAt.lt(cutoff_date))
            .filter(entities::Column::VersionName.is_null())
            .exec(&self.db)
            .await
            .map_err(|e| RevisionError::Database(e.to_string()))?;

        Ok(delete_result.rows_affected as usize)
    }

    async fn update_version_name(
        &self,
        id: Uuid,
        version_name: Option<String>,
    ) -> Result<(), RevisionError> {
        let mut model: entities::ActiveModel = entities::Entity::find_by_id(id)
            .one(&self.db)
            .await
            .map_err(|e| RevisionError::Database(e.to_string()))?
            .ok_or_else(|| RevisionError::NotFound(format!("Revision {} not found", id)))?
            .into();

        model.version_name = Set(version_name);

        model
            .update(&self.db)
            .await
            .map_err(|e| RevisionError::Database(e.to_string()))?;

        Ok(())
    }

    async fn get_named_versions(
        &self,
        tenant_id: Uuid,
        content_id: Uuid,
        locale: &str,
    ) -> Result<Vec<Revision>, RevisionError> {
        let models = entities::Entity::find()
            .filter(entities::Column::TenantId.eq(tenant_id))
            .filter(entities::Column::ContentId.eq(content_id))
            .filter(entities::Column::Locale.eq(locale))
            .filter(entities::Column::VersionName.is_not_null())
            .order_by_desc(entities::Column::RevisionNumber)
            .all(&self.db)
            .await
            .map_err(|e| RevisionError::Database(e.to_string()))?;

        Ok(models.into_iter().map(model_to_revision).collect())
    }
}

/// Convert a model to a Revision.
fn model_to_revision(model: entities::Model) -> Revision {
    Revision {
        id: model.id,
        tenant_id: model.tenant_id,
        content_id: model.content_id,
        content_type: model.content_type,
        locale: model.locale,
        revision_number: model.revision_number,
        parent_revision_id: model.parent_revision_id,
        event: string_to_event(&model.event),
        content: model.content,
        metadata: RevisionMetadata {
            user_id: model.user_id,
            source: string_to_source(&model.source),
            summary: model.summary,
            ip_address: model.ip_address,
            user_agent: model.user_agent,
            custom: model.custom_metadata,
        },
        created_at: model.created_at,
        version_name: model.version_name,
    }
}

/// Convert event to string.
fn event_to_string(event: RevisionEvent) -> String {
    match event {
        RevisionEvent::Create => "create".to_string(),
        RevisionEvent::Update => "update".to_string(),
        RevisionEvent::Delete => "delete".to_string(),
        RevisionEvent::Restore => "restore".to_string(),
        RevisionEvent::Snapshot => "snapshot".to_string(),
    }
}

/// Convert string to event.
fn string_to_event(s: &str) -> RevisionEvent {
    match s {
        "create" => RevisionEvent::Create,
        "update" => RevisionEvent::Update,
        "delete" => RevisionEvent::Delete,
        "restore" => RevisionEvent::Restore,
        "snapshot" => RevisionEvent::Snapshot,
        _ => RevisionEvent::Update,
    }
}

/// Convert source to string.
fn source_to_string(source: &ChangeSource) -> String {
    match source {
        ChangeSource::Web => "web".to_string(),
        ChangeSource::Api => "api".to_string(),
        ChangeSource::Admin => "admin".to_string(),
        ChangeSource::BackgroundJob => "background_job".to_string(),
        ChangeSource::Import => "import".to_string(),
        ChangeSource::Custom(s) => format!("custom:{}", s),
    }
}

/// Convert string to source.
fn string_to_source(s: &str) -> ChangeSource {
    match s {
        "web" => ChangeSource::Web,
        "api" => ChangeSource::Api,
        "admin" => ChangeSource::Admin,
        "background_job" => ChangeSource::BackgroundJob,
        "import" => ChangeSource::Import,
        s if s.starts_with("custom:") => ChangeSource::Custom(s[7..].to_string()),
        _ => ChangeSource::Web,
    }
}
