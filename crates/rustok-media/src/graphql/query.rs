use async_graphql::{Context, FieldError, Object, Result};
use rustok_api::graphql::{GraphQLError, PaginationInput, require_module_enabled};
use rustok_api::{
    Action, AuthContext, Permission, Resource, TenantContext, has_effective_permission,
};
use rustok_storage::StorageRuntime;
use sea_orm::DatabaseConnection;
use uuid::Uuid;

use crate::{MediaService, load_media_usage_snapshot};

use super::{GqlMediaItem, GqlMediaList, GqlMediaTranslation, MODULE_SLUG, MediaUsageStats};

#[derive(Default)]
pub struct MediaQuery;

#[Object]
impl MediaQuery {
    /// Media usage statistics for the current tenant.
    async fn media_usage(&self, ctx: &Context<'_>, tenant_id: Uuid) -> Result<MediaUsageStats> {
        require_module_enabled(ctx, MODULE_SLUG).await?;
        require_media_permission(ctx, tenant_id, Action::List)?;
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

    /// List media assets for the current tenant.
    async fn media(
        &self,
        ctx: &Context<'_>,
        tenant_id: Uuid,
        #[graphql(default)] pagination: PaginationInput,
    ) -> Result<GqlMediaList> {
        require_module_enabled(ctx, MODULE_SLUG).await?;
        require_media_permission(ctx, tenant_id, Action::List)?;
        let db = ctx.data::<DatabaseConnection>()?;
        let storage = ctx.data::<StorageRuntime>()?;

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
        require_media_permission(ctx, tenant_id, Action::Read)?;
        let db = ctx.data::<DatabaseConnection>()?;
        let storage = ctx.data::<StorageRuntime>()?;

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
        require_media_permission(ctx, tenant_id, Action::Read)?;
        let db = ctx.data::<DatabaseConnection>()?;
        let storage = ctx.data::<StorageRuntime>()?;

        let service = MediaService::new(db.clone(), storage.clone());
        let translations = service
            .get_translations(tenant_id, media_id)
            .await
            .map_err(|error| async_graphql::Error::new(error.to_string()))?;

        Ok(translations.into_iter().map(Into::into).collect())
    }
}

fn require_media_permission(
    ctx: &Context<'_>,
    requested_tenant: Uuid,
    action: Action,
) -> Result<()> {
    let auth = ctx
        .data::<AuthContext>()
        .map_err(|_| <FieldError as GraphQLError>::unauthenticated())?;
    let tenant = ctx.data::<TenantContext>()?;
    if requested_tenant != tenant.id || auth.tenant_id != tenant.id {
        return Err(<FieldError as GraphQLError>::permission_denied(
            "Media queries must use the current authenticated tenant",
        ));
    }

    let permission = Permission::new(Resource::Media, action);
    if !has_effective_permission(&auth.permissions, &permission) {
        return Err(<FieldError as GraphQLError>::permission_denied(&format!(
            "Permission required: {permission}"
        )));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use rustok_api::{Action, Permission, Resource};

    #[test]
    fn media_query_permissions_distinguish_list_and_read() {
        assert_ne!(
            Permission::new(Resource::Media, Action::List),
            Permission::new(Resource::Media, Action::Read)
        );
    }
}
