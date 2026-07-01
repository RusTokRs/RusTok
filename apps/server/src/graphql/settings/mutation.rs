use async_graphql::{Context, FieldError, Object, Result};
use loco_rs::app::AppContext;
use rustok_events::DomainEvent;

use crate::context::{AuthContext, TenantContext};
use crate::services::event_bus::transactional_event_bus_from_context;
use crate::services::rbac_service::RbacService;
use crate::services::settings_service::{SettingsService, ValidatorRegistry};
use rustok_api::graphql::GraphQLError;

use super::types::{UpdatePlatformSettingsInput, UpdatePlatformSettingsPayload};

#[derive(Default)]
pub struct SettingsMutation;

#[Object]
impl SettingsMutation {
    /// Update platform settings for a single category.
    /// Requires `settings:manage` permission.
    async fn update_platform_settings(
        &self,
        ctx: &Context<'_>,
        input: UpdatePlatformSettingsInput,
    ) -> Result<UpdatePlatformSettingsPayload> {
        let app_ctx = ctx.data::<AppContext>()?;
        let auth = ctx
            .data::<AuthContext>()
            .map_err(|_| <FieldError as GraphQLError>::unauthenticated())?;
        let tenant = ctx.data::<TenantContext>()?;

        let can_manage = RbacService::has_permission(
            &app_ctx.db,
            &tenant.id,
            &auth.user_id,
            &rustok_api::Permission::SETTINGS_MANAGE,
        )
        .await
        .map_err(|e| <FieldError as GraphQLError>::internal_error(&e.to_string()))?;

        if !can_manage {
            return Err(<FieldError as GraphQLError>::permission_denied(
                "settings:manage required",
            ));
        }

        let settings_json: serde_json::Value = serde_json::from_str(&input.settings)
            .map_err(|e| FieldError::new(format!("Invalid JSON in settings: {e}")))?;

        let validators = ValidatorRegistry::default();

        let stored = SettingsService::update(
            app_ctx,
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

        // Emit audit event through outbox (best-effort: don't fail the mutation on event error)
        let event_bus = transactional_event_bus_from_context(app_ctx);
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
