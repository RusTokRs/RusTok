use async_graphql::{Context, FieldError, Object, Result};

use crate::context::{AuthContext, TenantContext};
use crate::services::server_runtime_context::ServerRuntimeContext;
use crate::services::settings_service::SettingsService;
use rustok_api::{Permission, graphql::GraphQLError, has_effective_permission};

use super::types::PlatformSettingsPayload;

#[derive(Default)]
pub struct SettingsQuery;

#[Object]
impl SettingsQuery {
    /// Retrieve settings for a single category.
    /// Requires `settings:read` permission from the authenticated snapshot.
    async fn platform_settings(
        &self,
        ctx: &Context<'_>,
        category: String,
    ) -> Result<PlatformSettingsPayload> {
        let runtime_ctx = ctx.data::<ServerRuntimeContext>()?;
        let auth = ctx
            .data::<AuthContext>()
            .map_err(|_| <FieldError as GraphQLError>::unauthenticated())?;
        let tenant = ctx.data::<TenantContext>()?;

        if !has_effective_permission(&auth.permissions, &Permission::SETTINGS_READ) {
            return Err(<FieldError as GraphQLError>::permission_denied(
                "settings:read required",
            ));
        }

        let value = SettingsService::get(runtime_ctx, tenant.id, &category)
            .await
            .map_err(|e| <FieldError as GraphQLError>::internal_error(&e.to_string()))?;

        let settings = serde_json::to_string(&value)
            .map_err(|e| <FieldError as GraphQLError>::internal_error(&e.to_string()))?;

        Ok(PlatformSettingsPayload { category, settings })
    }

    /// Retrieve all platform setting categories for the current tenant.
    /// Requires `settings:read` permission from the authenticated snapshot.
    async fn all_platform_settings(
        &self,
        ctx: &Context<'_>,
    ) -> Result<Vec<PlatformSettingsPayload>> {
        let runtime_ctx = ctx.data::<ServerRuntimeContext>()?;
        let auth = ctx
            .data::<AuthContext>()
            .map_err(|_| <FieldError as GraphQLError>::unauthenticated())?;
        let tenant = ctx.data::<TenantContext>()?;

        if !has_effective_permission(&auth.permissions, &Permission::SETTINGS_READ) {
            return Err(<FieldError as GraphQLError>::permission_denied(
                "settings:read required",
            ));
        }

        let categories = SettingsService::get_all(runtime_ctx, tenant.id)
            .await
            .map_err(|e| <FieldError as GraphQLError>::internal_error(&e.to_string()))?;

        categories
            .into_iter()
            .map(|(category, value)| {
                let settings = serde_json::to_string(&value)
                    .map_err(|e| <FieldError as GraphQLError>::internal_error(&e.to_string()))?;
                Ok(PlatformSettingsPayload { category, settings })
            })
            .collect()
    }
}
