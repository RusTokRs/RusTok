use async_graphql::{Context, FieldError, Object, Result};
use std::fmt::Display;
use rustok_outbox::TransactionalEventBus;

use crate::context::{AuthContext, TenantContext};
use crate::services::server_runtime_context::ServerRuntimeContext;
use crate::services::settings_service::{SettingsError, SettingsService, ValidatorRegistry};
use rustok_api::{HostAuthority, Permission, graphql::GraphQLError, has_effective_permission};

use super::types::{
    UpdateEventDeliveryConfigurationInput, UpdateEventDeliveryConfigurationPayload,
    UpdateIggyConnectorConfigurationInput, UpdateIggyConnectorConfigurationPayload,
    UpdatePlatformSettingsInput, UpdatePlatformSettingsPayload,
};
use super::{require_host_actor, require_host_authority, require_tenant_settings_scope};

fn graphql_settings_internal_error(
    message: &'static str,
    error: impl Display,
) -> FieldError {
    tracing::error!(%error, message, "GraphQL settings mutation failed");
    <FieldError as GraphQLError>::internal_error(message)
}

fn map_iggy_settings_error(
    error: crate::services::iggy_connector_settings_service::IggyConnectorSettingsError,
) -> FieldError {
    use crate::services::iggy_connector_settings_service::IggyConnectorSettingsError;

    match error {
        IggyConnectorSettingsError::InvalidMode
        | IggyConnectorSettingsError::InvalidConfiguration(_) => {
            <FieldError as GraphQLError>::bad_user_input("Invalid Iggy connector configuration")
        }
        IggyConnectorSettingsError::Database(error) => {
            graphql_settings_internal_error("Unable to update Iggy connector configuration", error)
        }
    }
}

fn map_event_delivery_settings_error(
    error: crate::services::event_delivery_settings_service::EventDeliverySettingsError,
) -> FieldError {
    use crate::services::event_delivery_settings_service::EventDeliverySettingsError;

    match error {
        EventDeliverySettingsError::InvalidProfile(_) => {
            <FieldError as GraphQLError>::bad_user_input(
                "Event delivery profile must be one of: outbox, outbox_iggy",
            )
        }
        EventDeliverySettingsError::IggyNotConfigured(_) => {
            <FieldError as GraphQLError>::bad_user_input(
                "Iggy connector must be configured before selecting outbox_iggy",
            )
        }
        EventDeliverySettingsError::Database(error) => {
            graphql_settings_internal_error("Unable to update event delivery configuration", error)
        }
    }
}

fn map_platform_settings_update_error(error: SettingsError) -> FieldError {
    match error {
        SettingsError::InvalidCategory(_) => {
            <FieldError as GraphQLError>::bad_user_input("Invalid settings category")
        }
        SettingsError::ValidationFailed(errors) => {
            <FieldError as GraphQLError>::bad_user_input(&format!(
                "Settings validation failed: {}",
                errors.join("; ")
            ))
        }
        error => graphql_settings_internal_error("Unable to update platform settings", error),
    }
}

#[derive(Default)]
pub struct SettingsMutation;

#[Object]
impl SettingsMutation {
    async fn update_iggy_connector_configuration(
        &self,
        ctx: &Context<'_>,
        input: UpdateIggyConnectorConfigurationInput,
    ) -> Result<UpdateIggyConnectorConfigurationPayload> {
        let (authority, auth) = require_host_actor(ctx, HostAuthority::Manage)?;
        let runtime_ctx = ctx.data::<ServerRuntimeContext>()?;
        crate::services::iggy_connector_settings_service::IggyConnectorSettingsService::save(
            runtime_ctx,
            input.into(),
            authority.actor_id(),
            auth.tenant_id,
        )
        .await
        .map_err(map_iggy_settings_error)?;
        let snapshot = crate::services::iggy_connector_settings_service::IggyConnectorSettingsService::configuration(runtime_ctx)
            .await
            .map_err(|error| graphql_settings_internal_error("Unable to read Iggy connector configuration", error))?;
        Ok(UpdateIggyConnectorConfigurationPayload {
            desired_mode: snapshot.desired_mode,
            configured: snapshot.configured,
            restart_required: snapshot.restart_required,
        })
    }

    /// Persist the desired global event profile. The running transport is not
    /// hot-swapped: an operator-controlled restart activates the saved profile.
    async fn update_event_delivery_configuration(
        &self,
        ctx: &Context<'_>,
        input: UpdateEventDeliveryConfigurationInput,
    ) -> Result<UpdateEventDeliveryConfigurationPayload> {
        let authority = require_host_authority(ctx, HostAuthority::Manage)?;
        let runtime_ctx = ctx.data::<ServerRuntimeContext>()?;

        let profile = crate::common::settings::EventDeliveryProfile::parse(&input.profile)
            .ok_or_else(|| <FieldError as GraphQLError>::bad_user_input(
                "Event delivery profile must be one of: outbox, outbox_iggy",
            ))?;
        crate::services::event_delivery_settings_service::EventDeliverySettingsService::save_profile(
            runtime_ctx,
            profile,
            authority.actor_id(),
        )
        .await
        .map_err(map_event_delivery_settings_error)?;

        let active_profile = runtime_ctx
            .shared_get::<std::sync::Arc<crate::services::event_transport_factory::EventRuntime>>()
            .map(|runtime| runtime.delivery_profile)
            .unwrap_or(runtime_ctx.settings().events.delivery_profile);
        Ok(UpdateEventDeliveryConfigurationPayload {
            desired_profile: profile.as_str().to_string(),
            restart_required: active_profile != profile,
        })
    }

    /// Update platform settings for a single category.
    /// Requires `settings:manage` permission from the authenticated snapshot.
    async fn update_platform_settings(
        &self,
        ctx: &Context<'_>,
        input: UpdatePlatformSettingsInput,
    ) -> Result<UpdatePlatformSettingsPayload> {
        let runtime_ctx = ctx.data::<ServerRuntimeContext>()?;
        let auth = ctx
            .data::<AuthContext>()
            .map_err(|_| <FieldError as GraphQLError>::unauthenticated())?;
        let tenant = ctx.data::<TenantContext>()?;
        require_tenant_settings_scope(auth, tenant.id)?;

        if !has_effective_permission(&auth.permissions, &Permission::SETTINGS_MANAGE) {
            return Err(<FieldError as GraphQLError>::permission_denied(
                "settings:manage required",
            ));
        }

        let settings_json: serde_json::Value = serde_json::from_str(&input.settings)
            .map_err(|_| <FieldError as GraphQLError>::bad_user_input("Invalid JSON in settings"))?;

        let validators = ValidatorRegistry::default();
        let event_bus = ctx.data::<TransactionalEventBus>()?;

        let stored = SettingsService::update(
            runtime_ctx,
            event_bus,
            tenant.id,
            &input.category,
            settings_json,
            Some(auth.user_id),
            &validators,
        )
        .await
        .map_err(map_platform_settings_update_error)?;

        let settings_str = serde_json::to_string(&stored)
            .map_err(|error| graphql_settings_internal_error("Unable to serialize platform settings", error))?;

        Ok(UpdatePlatformSettingsPayload {
            success: true,
            category: input.category,
            settings: settings_str,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::{
        FieldError, graphql_settings_internal_error, map_event_delivery_settings_error,
        map_iggy_settings_error, map_platform_settings_update_error,
    };
    use crate::services::{
        event_delivery_settings_service::EventDeliverySettingsError,
        iggy_connector_settings_service::IggyConnectorSettingsError,
        settings_service::SettingsError,
    };

    #[test]
    fn invalid_settings_json_is_not_exposed_as_backend_diagnostics() {
        let error = <FieldError as rustok_api::graphql::GraphQLError>::bad_user_input(
            "Invalid JSON in settings",
        );
        assert_eq!(error.message, "Invalid JSON in settings");
    }

    #[test]
    fn invalid_platform_category_is_bad_user_input_without_echo() {
        let error = map_platform_settings_update_error(SettingsError::InvalidCategory(
            "smtp-password-secret".to_string(),
        ));
        assert_eq!(error.message, "Invalid settings category");
    }

    #[test]
    fn invalid_iggy_configuration_is_bad_user_input_without_detail_leak() {
        let error = map_iggy_settings_error(IggyConnectorSettingsError::InvalidConfiguration(
            "external Iggy password secret cannot be resolved: secret/path".to_string(),
        ));
        assert_eq!(
            error.message,
            "Invalid Iggy connector configuration"
        );
    }

    #[test]
    fn invalid_event_delivery_configuration_is_bad_user_input() {
        let error = map_event_delivery_settings_error(
            EventDeliverySettingsError::IggyNotConfigured(
                "secret resolver exposed path=/run/secrets/iggy".to_string(),
            ),
        );
        assert_eq!(
            error.message,
            "Iggy connector must be configured before selecting outbox_iggy"
        );
    }

    #[test]
    fn settings_internal_error_redacts_backend_diagnostics() {
        let error = graphql_settings_internal_error(
            "Unable to update platform settings",
            "database password=secret table=platform_settings",
        );
        assert_eq!(error.message, "Unable to update platform settings");
        assert!(!error.message.contains("database password=secret"));
    }
}
