use async_graphql::{Context, FieldError, Object, Result};
use std::fmt::Display;

use crate::context::{AuthContext, TenantContext};
use crate::services::server_runtime_context::ServerRuntimeContext;
use crate::services::settings_service::{SettingsError, SettingsService};
use rustok_api::{HostAuthority, Permission, graphql::GraphQLError, has_effective_permission};

use super::types::{
    EventDeliveryConfigurationPayload, IggyConnectorConfigurationPayload, PlatformSettingsPayload,
};
use super::{require_host_authority, require_tenant_settings_scope};

fn graphql_settings_internal_error(message: &'static str, error: impl Display) -> FieldError {
    tracing::error!(%error, message, "GraphQL settings query failed");
    <FieldError as GraphQLError>::internal_error(message)
}

fn map_platform_settings_error(error: SettingsError) -> FieldError {
    match error {
        SettingsError::InvalidCategory(_) => {
            <FieldError as GraphQLError>::bad_user_input("Invalid settings category")
        }
        error => graphql_settings_internal_error("Platform settings are unavailable", error),
    }
}

#[derive(Default)]
pub struct SettingsQuery;

#[Object]
impl SettingsQuery {
    async fn iggy_connector_configuration(
        &self,
        ctx: &Context<'_>,
    ) -> Result<IggyConnectorConfigurationPayload> {
        require_host_authority(ctx, HostAuthority::Read)?;
        let runtime_ctx = ctx.data::<ServerRuntimeContext>()?;
        let snapshot = crate::services::iggy_connector_settings_service::IggyConnectorSettingsService::configuration(runtime_ctx)
            .await
            .map_err(|error| graphql_settings_internal_error("Unable to read Iggy connector configuration", error))?;
        Ok(snapshot.into())
    }

    /// Read the global event delivery profile. This setting applies to the whole
    /// process after a controlled restart and is never tenant-scoped.
    async fn event_delivery_configuration(
        &self,
        ctx: &Context<'_>,
    ) -> Result<EventDeliveryConfigurationPayload> {
        require_host_authority(ctx, HostAuthority::Read)?;
        let runtime_ctx = ctx.data::<ServerRuntimeContext>()?;

        let configuration = crate::services::event_delivery_settings_service::EventDeliverySettingsService::configuration(runtime_ctx)
            .await
            .map_err(|error| graphql_settings_internal_error("Unable to read event delivery configuration", error))?;
        let active_profile = runtime_ctx
            .shared_get::<std::sync::Arc<crate::services::event_transport_factory::EventRuntime>>()
            .map(|runtime| runtime.delivery_profile)
            .unwrap_or(configuration.active_profile);
        let iggy = crate::services::iggy_connector_settings_service::IggyConnectorSettingsService::configuration(runtime_ctx)
            .await
            .map_err(|error| graphql_settings_internal_error("Unable to read Iggy connector configuration", error))?;

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
        require_tenant_settings_scope(auth, tenant.id)?;

        if !has_effective_permission(&auth.permissions, &Permission::SETTINGS_READ) {
            return Err(<FieldError as GraphQLError>::permission_denied(
                "settings:read required",
            ));
        }

        let value = SettingsService::get(runtime_ctx, tenant.id, &category)
            .await
            .map_err(map_platform_settings_error)?;

        let settings = serde_json::to_string(&value).map_err(|error| {
            graphql_settings_internal_error("Platform settings are unavailable", error)
        })?;

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
        require_tenant_settings_scope(auth, tenant.id)?;

        if !has_effective_permission(&auth.permissions, &Permission::SETTINGS_READ) {
            return Err(<FieldError as GraphQLError>::permission_denied(
                "settings:read required",
            ));
        }

        let categories = SettingsService::get_all(runtime_ctx, tenant.id)
            .await
            .map_err(|error| {
                graphql_settings_internal_error("Platform settings are unavailable", error)
            })?;

        categories
            .into_iter()
            .map(|(category, value)| {
                let settings = serde_json::to_string(&value).map_err(|error| {
                    graphql_settings_internal_error("Platform settings are unavailable", error)
                })?;
                Ok(PlatformSettingsPayload { category, settings })
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::{graphql_settings_internal_error, map_platform_settings_error};
    use crate::services::settings_service::SettingsError;

    #[test]
    fn invalid_settings_category_is_bad_user_input_without_echoing_input() {
        let attacker_input = "email\nDROP TABLE platform_settings";
        let error =
            map_platform_settings_error(SettingsError::InvalidCategory(attacker_input.to_string()));

        assert_eq!(error.message, "Invalid settings category");
        assert!(!error.message.contains(attacker_input));
    }

    #[test]
    fn settings_internal_error_redacts_backend_diagnostics() {
        let error = graphql_settings_internal_error(
            "Platform settings are unavailable",
            "database password=secret table=platform_settings",
        );

        assert_eq!(error.message, "Platform settings are unavailable");
        assert!(!error.message.contains("database password=secret"));
    }
}
