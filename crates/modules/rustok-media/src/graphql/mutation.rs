use async_graphql::{Context, FieldError, Object, Result};
use rustok_api::graphql::{GraphQLError, require_module_enabled};
use rustok_api::{
    Action, AuthContext, Permission, Resource, TenantContext, has_effective_permission,
};
use rustok_storage::StorageRuntime;
use sea_orm::DatabaseConnection;
use uuid::Uuid;

use crate::{MediaService, dto::UpsertTranslationInput};

use super::{GqlMediaTranslation, MODULE_SLUG, UpsertMediaTranslationInput};

#[derive(Default)]
pub struct MediaMutation;

#[Object]
impl MediaMutation {
    /// Delete a media asset and remove it from storage.
    async fn delete_media(&self, ctx: &Context<'_>, tenant_id: Uuid, id: Uuid) -> Result<bool> {
        require_module_enabled(ctx, MODULE_SLUG).await?;
        require_media_permission(ctx, tenant_id, Action::Delete)?;
        let db = ctx.data::<DatabaseConnection>()?;
        let storage = ctx.data::<StorageRuntime>()?;

        let service = MediaService::new(db.clone(), storage.clone());
        service
            .delete(tenant_id, id)
            .await
            .map_err(|error| async_graphql::Error::new(error.to_string()))?;
        Ok(true)
    }

    /// Upsert alt-text / title / caption for a given locale.
    async fn upsert_media_translation(
        &self,
        ctx: &Context<'_>,
        tenant_id: Uuid,
        media_id: Uuid,
        input: UpsertMediaTranslationInput,
    ) -> Result<GqlMediaTranslation> {
        require_module_enabled(ctx, MODULE_SLUG).await?;
        require_media_permission(ctx, tenant_id, Action::Update)?;
        let db = ctx.data::<DatabaseConnection>()?;
        let storage = ctx.data::<StorageRuntime>()?;

        let service = MediaService::new(db.clone(), storage.clone());
        let translation = service
            .upsert_translation(
                tenant_id,
                media_id,
                UpsertTranslationInput {
                    locale: input.locale,
                    title: input.title,
                    alt_text: input.alt_text,
                    caption: input.caption,
                },
            )
            .await
            .map_err(|error| async_graphql::Error::new(error.to_string()))?;

        Ok(translation.into())
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
            "Media mutations must use the current authenticated tenant",
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
    fn media_mutation_permissions_are_action_specific() {
        assert_ne!(
            Permission::new(Resource::Media, Action::Delete),
            Permission::new(Resource::Media, Action::Update)
        );
    }
}
