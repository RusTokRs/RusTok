use async_graphql::{Context, ErrorExtensions, FieldError, Object, Result};
use sea_orm::DatabaseConnection;

use rustok_api::{Permission, graphql::GraphQLError};
use rustok_core::ModuleRegistry;

use crate::context::{AuthContext, TenantContext};
use crate::graphql::types::TenantModule;
use crate::services::module_lifecycle::UpdateModuleSettingsError;
use crate::services::module_rollout_promotion_settings::{
    ModuleRolloutPromotionSettingsOutcome, ModuleRolloutPromotionSettingsService,
};
use crate::services::rbac_service::RbacService;

const MODULE_SETTINGS_SNAPSHOT_CONFLICT: &str = "MODULE_SETTINGS_SNAPSHOT_CONFLICT";

/// Conditional module-settings write surface for reviewed/control-plane automation.
///
/// This mutation is deliberately not an approval authority. Callers that require a
/// separate review or rollout decision must validate that policy before invoking it.
/// The server owns tenant/RBAC admission and the exact snapshot compare-and-swap.
#[derive(Default)]
pub struct ModuleSettingsCasMutation;

#[Object]
impl ModuleSettingsCasMutation {
    async fn compare_and_swap_module_settings(
        &self,
        ctx: &Context<'_>,
        module_slug: String,
        expected_enabled: bool,
        expected_settings: String,
        settings: String,
    ) -> Result<TenantModule> {
        let auth = ctx
            .data::<AuthContext>()
            .map_err(|_| <FieldError as GraphQLError>::unauthenticated())?;
        let tenant = ctx.data::<TenantContext>()?;
        let db = ctx.data::<DatabaseConnection>()?;

        let can_manage_modules =
            RbacService::has_permission(db, &tenant.id, &auth.user_id, &Permission::MODULES_MANAGE)
                .await
                .map_err(|error| {
                    <FieldError as GraphQLError>::internal_error(&error.to_string())
                })?;
        if !can_manage_modules {
            return Err(<FieldError as GraphQLError>::permission_denied(
                "Permission denied: modules:manage required",
            ));
        }

        let expected_settings = serde_json::from_str(&expected_settings).map_err(|error| {
            <FieldError as GraphQLError>::bad_user_input(&format!(
                "Invalid JSON in expected settings: {error}"
            ))
        })?;
        let settings = serde_json::from_str(&settings).map_err(|error| {
            <FieldError as GraphQLError>::bad_user_input(&format!(
                "Invalid JSON in settings: {error}"
            ))
        })?;
        let registry = ctx.data::<ModuleRegistry>()?;

        match ModuleRolloutPromotionSettingsService::update_if_current(
            db,
            registry,
            tenant.id,
            &module_slug,
            expected_enabled,
            expected_settings,
            settings,
        )
        .await
        .map_err(map_settings_error)?
        {
            ModuleRolloutPromotionSettingsOutcome::Updated(module) => Ok(TenantModule {
                module_slug: module.module_slug,
                enabled: module.enabled,
                settings: module.settings.to_string(),
            }),
            ModuleRolloutPromotionSettingsOutcome::Conflict => Err(FieldError::new(
                "Module settings changed since the reviewed snapshot",
            )
            .extend_with(|_, extensions| {
                extensions.set("code", MODULE_SETTINGS_SNAPSHOT_CONFLICT);
                extensions.set("retryable_issue", false);
                extensions.set("requires_rereview", true);
            })),
        }
    }
}

fn map_settings_error(error: UpdateModuleSettingsError) -> FieldError {
    match error {
        UpdateModuleSettingsError::UnknownModule => {
            <FieldError as GraphQLError>::bad_user_input("Unknown module")
        }
        UpdateModuleSettingsError::ModuleNotEnabled(module_slug) => {
            <FieldError as GraphQLError>::bad_user_input(&format!(
                "Module is not enabled for this tenant: {module_slug}"
            ))
        }
        UpdateModuleSettingsError::InvalidSettings => {
            <FieldError as GraphQLError>::bad_user_input(
                "Module settings and expected settings must be JSON objects",
            )
        }
        UpdateModuleSettingsError::Validation(message) => {
            <FieldError as GraphQLError>::bad_user_input(&message)
        }
        UpdateModuleSettingsError::Manifest(error) => {
            <FieldError as GraphQLError>::internal_error(&error.to_string())
        }
        UpdateModuleSettingsError::Policy(error) => {
            <FieldError as GraphQLError>::internal_error(&error)
        }
        UpdateModuleSettingsError::Database(error) => {
            <FieldError as GraphQLError>::internal_error(&error.to_string())
        }
    }
}
