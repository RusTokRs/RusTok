use async_graphql::{Context, Object, Result};
use rustok_api::graphql::{require_module_enabled, PaginationInput};
use rustok_storage::StorageService;
use sea_orm::DatabaseConnection;
use uuid::Uuid;

use crate::{load_media_usage_snapshot, MediaService};

use super::{GqlMediaItem, GqlMediaList, GqlMediaTranslation, MediaUsageStats, MODULE_SLUG};

#[derive(Default)]
pub struct MediaQuery;

#[Object]
impl MediaQuery {
    /// Media usage statistics for a tenant.
    async fn media_usage(&self, ctx: &Context<'_>, tenant_id: Uuid) -> Result<MediaUsageStats> {
        let db = ctx.data::<DatabaseConnection>()?;
        let usage = load_media_usage_snapshot(db, tenant_id)
            .await
            .map_err(|error| async_graphql::Error::new(error.to_string()))?;

        Ok(MediaUsageStats {
            tenant_id: usage.tenant_id,
            file_count: usage.file_count,
            total_bytes: usage.total_bytes,
        })
    }

    /// List media assets for a tenant.
    async fn media(
        &self,
        ctx: &Context<'_>,
        tenant_id: Uuid,
        #[graphql(default)] pagination: PaginationInput,
    ) -> Result<GqlMediaList> {
        require_module_enabled(ctx, MODULE_SLUG).await?;
        let db = ctx.data::<DatabaseConnection>()?;
        let storage = ctx.data::<StorageService>()?;

        let service = MediaService::new(db.clone(), storage.clone());
        let (offset, limit) = pagination.normalize()?;
        let (items, total) = service
            .list(tenant_id, limit as u64, offset as u64)
            .await
            .map_err(|error| async_graphql::Error::new(error.to_string()))?;

        Ok(GqlMediaList {
            items: items.into_iter().map(Into::into).collect(),
            total: total as i64,
        })
    }

    /// Get a single media asset by ID.
    async fn media_item(
        &self,
        ctx: &Context<'_>,
        tenant_id: Uuid,
        id: Uuid,
    ) -> Result<Option<GqlMediaItem>> {
        require_module_enabled(ctx, MODULE_SLUG).await?;
        let db = ctx.data::<DatabaseConnection>()?;
        let storage = ctx.data::<StorageService>()?;

        let service = MediaService::new(db.clone(), storage.clone());
        match service.get(tenant_id, id).await {
            Ok(item) => Ok(Some(item.into())),
            Err(crate::MediaError::NotFound(_)) => Ok(None),
            Err(error) => Err(async_graphql::Error::new(error.to_string())),
        }
    }

    /// Get all translations for a media asset.
    async fn media_translations(
        &self,
        ctx: &Context<'_>,
        tenant_id: Uuid,
        media_id: Uuid,
    ) -> Result<Vec<GqlMediaTranslation>> {
        require_module_enabled(ctx, MODULE_SLUG).await?;
        let db = ctx.data::<DatabaseConnection>()?;
        let storage = ctx.data::<StorageService>()?;

        let service = MediaService::new(db.clone(), storage.clone());
        let translations = service
            .get_translations(tenant_id, media_id)
            .await
            .map_err(|error| async_graphql::Error::new(error.to_string()))?;

        Ok(translations.into_iter().map(Into::into).collect())
    }
}
