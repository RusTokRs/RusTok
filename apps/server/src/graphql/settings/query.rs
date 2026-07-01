use async_graphql::{Context, FieldError, Object, Result};
use loco_rs::app::AppContext;

use crate::context::{AuthContext, TenantContext};
use crate::services::rbac_service::RbacService;
use crate::services::settings_service::SettingsService;
use rustok_api::graphql::GraphQLError;

use super::types::PlatformSettingsPayload;

#[derive(Default)]
pub struct SettingsQuery;

#[Object]
impl SettingsQuery {
    /// Retrieve settings for a single category.
    /// Requires `settings:read` permission.
    async fn platform_settings(
        &self,
        ctx: &Context<'_>,
        category: String,
    ) -> Result<PlatformSettingsPayload> {
        let app_ctx = ctx.data::<AppContext>()?;
        let auth = ctx
            .data::<AuthContext>()
            .map_err(|_| <FieldError as GraphQLError>::unauthenticated())?;
        let tenant = ctx.data::<TenantContext>()?;

        let can_read = RbacService::has_permission(
            &app_ctx.db,
            &tenant.id,
            &auth.user_id,
            &rustok_api::Permission::SETTINGS_READ,
        )
        .await
        .map_err(|e| <FieldError as GraphQLError>::internal_error(&e.to_string()))?;

        if !can_read {
            return Err(<FieldError as GraphQLError>::permission_denied(
                "settings:read required",
            ));
        }

        let value = SettingsService::get(app_ctx, tenant.id, &category)
            .await
            .map_err(|e| <FieldError as GraphQLError>::internal_error(&e.to_string()))?;

        let settings = serde_json::to_string(&value)
            .map_err(|e| <FieldError as GraphQLError>::internal_error(&e.to_string()))?;

        Ok(PlatformSettingsPayload { category, settings })
    }

    /// Retrieve all platform setting categories for the current tenant.
    /// Requires `settings:read` permission.
    async fn all_platform_settings(
        &self,
        ctx: &Context<'_>,
    ) -> Result<Vec<PlatformSettingsPayload>> {
        let app_ctx = ctx.data::<AppContext>()?;
        let auth = ctx
            .data::<AuthContext>()
            .map_err(|_| <FieldError as GraphQLError>::unauthenticated())?;
        let tenant = ctx.data::<TenantContext>()?;

        let can_read = RbacService::has_permission(
            &app_ctx.db,
            &tenant.id,
            &auth.user_id,
            &rustok_api::Permission::SETTINGS_READ,
        )
        .await
        .map_err(|e| <FieldError as GraphQLError>::internal_error(&e.to_string()))?;

        if !can_read {
            return Err(<FieldError as GraphQLError>::permission_denied(
                "settings:read required",
            ));
        }

        let categories = SettingsService::get_all(app_ctx, tenant.id)
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
