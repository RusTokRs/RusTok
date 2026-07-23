use async_graphql::{Context, FieldError, Object, Result};
use rustok_events::DomainEvent;
use rustok_outbox::TransactionalEventBus;

use crate::context::{AuthContext, TenantContext};
use crate::services::server_runtime_context::ServerRuntimeContext;
use crate::services::settings_service::{SettingsService, ValidatorRegistry};
use rustok_api::{Permission, graphql::GraphQLError, has_effective_permission};

use super::types::{
    UpdateEventDeliveryConfigurationInput, UpdateEventDeliveryConfigurationPayload,
    UpdateIggyConnectorConfigurationInput, UpdateIggyConnectorConfigurationPayload,
    UpdatePlatformSettingsInput, UpdatePlatformSettingsPayload,
};

#[derive(Default)]
pub struct SettingsMutation;

#[Object]
impl SettingsMutation {
    async fn update_iggy_connector_configuration(
        &self,
        ctx: &Context<'_>,
        input: UpdateIggyConnectorConfigurationInput,
    ) -> Result<UpdateIggyConnectorConfigurationPayload> {
        let runtime_ctx = ctx.data::<ServerRuntimeContext>()?;
        let auth = ctx
            .data::<AuthContext>()
            .map_err(|_| <FieldError as GraphQLError>::unauthenticated())?;
        if !has_effective_permission(&auth.permissions, &Permission::SETTINGS_MANAGE) {
            return Err(<FieldError as GraphQLError>::permission_denied(
                "settings:manage required",
            ));
        }
        crate::services::iggy_connector_settings_service::IggyConnectorSettingsService::save(
            runtime_ctx,
            input.into(),
            auth.user_id,
            auth.tenant_id,
        )
        .await
        .map_err(|error| FieldError::new(error.to_string()))?;
        let snapshot = crate::services::iggy_connector_settings_service::IggyConnectorSettingsService::configuration(runtime_ctx)
            .await
            .map_err(|error| <FieldError as GraphQLError>::internal_error(&error.to_string()))?;
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
        let runtime_ctx = ctx.data::<ServerRuntimeContext>()?;
        let auth = ctx
            .data::<AuthContext>()
            .map_err(|_| <FieldError as GraphQLError>::unauthenticated())?;

        if !has_effective_permission(&auth.permissions, &Permission::SETTINGS_MANAGE) {
            return Err(<FieldError as GraphQLError>::permission_denied(
                "settings:manage required",
            ));
        }

        let profile = crate::common::settings::EventDeliveryProfile::parse(&input.profile)
            .ok_or_else(|| {
                FieldError::new("profile must be one of: memory, outbox_local, outbox_iggy")
            })?;
        crate::services::event_delivery_settings_service::EventDeliverySettingsService::save_profile(
            runtime_ctx,
            profile,
            auth.user_id,
        )
        .await
        .map_err(|error| FieldError::new(error.to_string()))?;

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

        if !has_effective_permission(&auth.permissions, &Permission::SETTINGS_MANAGE) {
            return Err(<FieldError as GraphQLError>::permission_denied(
                "settings:manage required",
            ));
        }

        let settings_json: serde_json::Value = serde_json::from_str(&input.settings)
            .map_err(|e| FieldError::new(format!("Invalid JSON in settings: {e}")))?;

        let validators = ValidatorRegistry::default();

        let stored = SettingsService::update(
            runtime_ctx,
            tenant.id,
            &input.category,
            settings_json,
            Some(auth.user_id),
            &validators,
        )
        .await
        .map_err(|e| match e {
            crate::services::settings_service::SettingsError::InvalidCategory(c) => {
                FieldError::new(format!("Unknown settings category: {c}"))
            }
            crate::services::settings_service::SettingsError::ValidationFailed(errs) => {
                FieldError::new(format!("Validation failed: {}", errs.join("; ")))
            }
            other => <FieldError as GraphQLError>::internal_error(&other.to_string()),
        })?;

        let event_bus = ctx.data::<TransactionalEventBus>()?;
        if let Err(e) = event_bus
            .publish(
                tenant.id,
                Some(auth.user_id),
                DomainEvent::PlatformSettingsChanged {
                    category: input.category.clone(),
                    changed_by: auth.user_id,
                },
            )
            .await
        {
            tracing::warn!(
                category = %input.category,
                actor = %auth.user_id,
                error = %e,
                "Failed to publish PlatformSettingsChanged event; settings were saved"
            );
        }

        let settings_str = serde_json::to_string(&stored)
            .map_err(|e| <FieldError as GraphQLError>::internal_error(&e.to_string()))?;

        Ok(UpdatePlatformSettingsPayload {
            success: true,
            category: input.category,
            settings: settings_str,
        })
    }
}
