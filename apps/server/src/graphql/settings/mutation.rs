use async_graphql::{Context, FieldError, Object, Result};
use rustok_events::DomainEvent;
use rustok_outbox::TransactionalEventBus;

use crate::context::{AuthContext, TenantContext};
use crate::services::server_runtime_context::ServerRuntimeContext;
use crate::services::settings_service::{SettingsService, ValidatorRegistry};
use rustok_api::{Permission, graphql::GraphQLError, has_effective_permission};

use super::types::{UpdatePlatformSettingsInput, UpdatePlatformSettingsPayload};

#[derive(Default)]
pub struct SettingsMutation;

#[Object]
impl SettingsMutation {
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
