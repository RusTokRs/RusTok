use async_graphql::{Context, ErrorExtensions, FieldError, Object, Result};
use sea_orm::DatabaseConnection;

use rustok_api::{Permission, graphql::GraphQLError};
use rustok_core::ModuleRegistry;

use crate::context::{AuthContext, TenantContext};
use crate::graphql::mutations::module_command_context;
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
        expected_revision: i64,
        idempotency_key: uuid::Uuid,
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
        if idempotency_key.is_nil() || expected_revision < 0 {
            return Err(<FieldError as GraphQLError>::bad_user_input(
                "Module settings CAS requires a non-nil idempotency key and non-negative lifecycle revision",
            ));
        }
        let expected_revision = u64::try_from(expected_revision).map_err(|_| {
            <FieldError as GraphQLError>::bad_user_input(
                "Static module lifecycle revision is outside the supported range",
            )
        })?;

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
            module_command_context(auth.user_id, Some(tenant.id), idempotency_key),
            expected_revision,
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
                revision: i64::try_from(module.revision).map_err(|_| {
                    <FieldError as GraphQLError>::internal_error(
                        "Static module lifecycle revision is outside the GraphQL range",
                    )
                })?,
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
        UpdateModuleSettingsError::ModuleNotEnabled(_) => {
            <FieldError as GraphQLError>::bad_user_input("Module is not enabled for this tenant")
        }
        UpdateModuleSettingsError::InvalidSettings => <FieldError as GraphQLError>::bad_user_input(
            "Module settings and expected settings must be JSON objects",
        ),
        UpdateModuleSettingsError::Validation(message) => {
            <FieldError as GraphQLError>::bad_user_input(&message)
        }
        UpdateModuleSettingsError::IdempotencyConflict => {
            FieldError::new("Module settings idempotency key was reused for a different command")
                .extend_with(|_, extensions| {
                    extensions.set("code", "IDEMPOTENCY_CONFLICT");
                    extensions.set("retryable_issue", false);
                })
        }
        UpdateModuleSettingsError::SettingsSnapshotConflict => FieldError::new(
            "Module settings changed since the reviewed snapshot",
        )
        .extend_with(|_, extensions| {
            extensions.set("code", MODULE_SETTINGS_SNAPSHOT_CONFLICT);
            extensions.set("retryable_issue", false);
            extensions.set("requires_rereview", true);
        }),
        UpdateModuleSettingsError::ModuleNotEnabledRecorded => {
            <FieldError as GraphQLError>::bad_user_input("Module is not enabled for this tenant")
        }
        UpdateModuleSettingsError::RevisionConflict { expected, current } => FieldError::new(
            format!("Module lifecycle revision conflict: expected {expected}, current {current}"),
        )
        .extend_with(|_, extensions| {
            extensions.set("code", "REVISION_CONFLICT");
            extensions.set(
                "expected_revision",
                i64::try_from(expected).unwrap_or(i64::MAX),
            );
            extensions.set(
                "current_revision",
                i64::try_from(current).unwrap_or(i64::MAX),
            );
        }),
        UpdateModuleSettingsError::RevisionConflictRecorded => {
            FieldError::new("Module lifecycle revision changed; reload the current module state")
                .extend_with(|_, extensions| {
                    extensions.set("code", "REVISION_CONFLICT");
                    extensions.set("retryable_issue", false);
                    extensions.set("requires_reload", true);
                })
        }
        UpdateModuleSettingsError::OperationInProgress => FieldError::new(
            "Module lifecycle operation is already active",
        )
        .extend_with(|_, extensions| {
            extensions.set("code", "MODULE_LIFECYCLE_OPERATION_IN_PROGRESS");
        }),
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

#[cfg(test)]
mod tests {
    use super::{MODULE_SETTINGS_SNAPSHOT_CONFLICT, map_settings_error};
    use crate::services::module_lifecycle::UpdateModuleSettingsError;

    fn error_code(error: &async_graphql::Error) -> Option<String> {
        error
            .extensions
            .as_ref()
            .and_then(|extensions| extensions.get("code"))
            .cloned()
            .and_then(|value| value.into_json().ok())
            .and_then(|value| value.as_str().map(ToOwned::to_owned))
    }

    #[test]
    fn retained_settings_failures_keep_their_transport_taxonomy() {
        let snapshot = map_settings_error(UpdateModuleSettingsError::SettingsSnapshotConflict);
        assert_eq!(
            error_code(&snapshot).as_deref(),
            Some(MODULE_SETTINGS_SNAPSHOT_CONFLICT),
        );

        let disabled = map_settings_error(UpdateModuleSettingsError::ModuleNotEnabledRecorded);
        assert_eq!(disabled.message, "Module is not enabled for this tenant");
        assert_eq!(error_code(&disabled).as_deref(), Some("BAD_USER_INPUT"));

        let revision = map_settings_error(UpdateModuleSettingsError::RevisionConflictRecorded);
        assert_eq!(error_code(&revision).as_deref(), Some("REVISION_CONFLICT"));
    }
}
