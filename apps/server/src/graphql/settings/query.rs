use async_graphql::{Context, FieldError, Object, Result};

use crate::context::{AuthContext, TenantContext};
use crate::services::server_runtime_context::ServerRuntimeContext;
use crate::services::settings_service::SettingsService;
use rustok_api::{Permission, graphql::GraphQLError, has_effective_permission};

use super::types::{
    EventDeliveryConfigurationPayload, IggyConnectorConfigurationPayload, PlatformSettingsPayload,
};

#[derive(Default)]
pub struct SettingsQuery;

#[Object]
impl SettingsQuery {
    async fn iggy_connector_configuration(
        &self,
        ctx: &Context<'_>,
    ) -> Result<IggyConnectorConfigurationPayload> {
        let runtime_ctx = ctx.data::<ServerRuntimeContext>()?;
        let auth = ctx
            .data::<AuthContext>()
            .map_err(|_| <FieldError as GraphQLError>::unauthenticated())?;
        if !has_effective_permission(&auth.permissions, &Permission::SETTINGS_READ) {
            return Err(<FieldError as GraphQLError>::permission_denied(
                "settings:read required",
            ));
        }
        let snapshot = crate::services::iggy_connector_settings_service::IggyConnectorSettingsService::configuration(runtime_ctx)
            .await
            .map_err(|error| <FieldError as GraphQLError>::internal_error(&error.to_string()))?;
        Ok(snapshot.into())
    }

    /// Read the global event delivery profile. This setting applies to the whole
    /// process after a controlled restart and is never tenant-scoped.
    async fn event_delivery_configuration(
        &self,
        ctx: &Context<'_>,
    ) -> Result<EventDeliveryConfigurationPayload> {
        let runtime_ctx = ctx.data::<ServerRuntimeContext>()?;
        let auth = ctx
            .data::<AuthContext>()
            .map_err(|_| <FieldError as GraphQLError>::unauthenticated())?;

        if !has_effective_permission(&auth.permissions, &Permission::SETTINGS_READ) {
            return Err(<FieldError as GraphQLError>::permission_denied(
                "settings:read required",
            ));
        }

        let configuration = crate::services::event_delivery_settings_service::EventDeliverySettingsService::configuration(runtime_ctx)
            .await
            .map_err(|error| <FieldError as GraphQLError>::internal_error(&error.to_string()))?;
        let active_profile = runtime_ctx
            .shared_get::<std::sync::Arc<crate::services::event_transport_factory::EventRuntime>>()
            .map(|runtime| runtime.delivery_profile)
            .unwrap_or(configuration.active_profile);
        let iggy = crate::services::iggy_connector_settings_service::IggyConnectorSettingsService::configuration(runtime_ctx)
            .await
            .map_err(|error| <FieldError as GraphQLError>::internal_error(&error.to_string()))?;

        Ok(EventDeliveryConfigurationPayload {
            active_profile: active_profile.as_str().to_string(),
            desired_profile: configuration.desired_profile.as_str().to_string(),
            iggy_mode: iggy.desired_mode,
            iggy_configured: configuration.iggy_configured,
            restart_required: active_profile != configuration.desired_profile,
        })
    }

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
