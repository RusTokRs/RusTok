use async_graphql::{Context, ErrorExtensions, FieldError, Json, Object, Result};
use axum::http::StatusCode;
use sea_orm::{ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, Set};

use crate::common::RequestContext;
use crate::context::{AuthContext, TenantContext};
use crate::error::Error as ServerError;
use crate::graphql::artifact_lifecycle::{
    map_artifact_installation_lifecycle_error, map_artifact_tenant_lifecycle_error,
};
use crate::graphql::queries::ensure_modules_read_permission;
use crate::graphql::types::{
    ArtifactActivation, ArtifactDataPurgeReceipt, ArtifactDeactivation, ArtifactRollback,
    ArtifactSettingsPurgeReceipt, ArtifactSettingsRecoveryPointReceipt,
    ArtifactSettingsRestoreReceipt, ArtifactTenantLifecycle, ArtifactUninstall, BuildJob,
    CreateUserInput, DeleteUserPayload, ModuleOperationRecoveryPlan, TenantModule, UpdateUserInput,
    User,
};
use crate::services::artifact_purge_recovery_host::{
    ServerArtifactDataPurgeAuthorizer, ServerArtifactSettingsRecoveryAuthorizer,
    ServerArtifactSettingsRecoveryCipher,
};
use rustok_modules::{
    ArtifactDataError, ArtifactDataPurgeRequest, ArtifactDataScope, ArtifactSettingsPurgeRequest,
    ArtifactSettingsRecoveryError, ArtifactSettingsRecoveryPointCreateRequest,
    ArtifactSettingsRestoreRequest,
};
use crate::models::_entities::users::Column as UsersColumn;
use crate::models::users;
use crate::modules::ManifestError;
use crate::services::artifact_ui::execute_artifact_ui_action as execute_artifact_ui_action_service;
#[cfg(test)]
use crate::services::auth_lifecycle::AuthLifecycleError;
use crate::services::build_event_hub::{
    BuildEventHubPublisher, CompositeBuildEventPublisher, build_event_hub_from_context,
};
use crate::services::event_bus::event_bus_from_context;
#[cfg(test)]
use crate::services::flex_attached_values::{
    FlexAttachedValuesService, PreparedAttachedValuesWrite,
};
use crate::services::module_lifecycle::{
    ModuleLifecycleService, ModuleOperationRecoveryError, ToggleModuleError,
    UpdateModuleSettingsError,
};
use crate::services::platform_composition::{
    PlatformCompositionBuildError, PlatformCompositionBuildService, PlatformCompositionError,
    PlatformCompositionModuleChange, PlatformCompositionModuleMutation,
};
use crate::services::rbac_service::RbacService;
use crate::services::server_runtime_context::ServerRuntimeContext;
use rustok_api::graphql::GraphQLError;
use rustok_api::{AuthPrincipalContext, Permission, PortError, PortErrorKind};
use rustok_auth::{
    AuthAdminMutationContext, AuthAdminMutationError, CreateUserCommand, UpdateUserCommand,
    UserAdminMutationRuntime, UserMutationRecord,
};
use rustok_build::{BuildEventPublicationContext, BuildEventScope, EventBusBuildEventPublisher};
use rustok_core::{ModuleRegistry, ModuleRuntimeExtensions, UserRole};
use rustok_modules::{
    ArtifactActivationRequest, ArtifactDeactivationRequest,
    ArtifactRollbackRequest, ArtifactTenantDisableRequest, ArtifactTenantEnableRequest,
    ArtifactUninstallRequest, ModuleCommandContext, ModuleCompositionError, ModuleControlPlane,
    ModuleInstallationScope,
};
use rustok_rbac::{RbacControlPlanePrincipal, require_direct_control_plane_user};
use std::sync::Arc;
use uuid::Uuid;

#[derive(Default)]
pub struct RootMutation;

const TOGGLE_ERR_UNKNOWN_MODULE: &str = "Unknown module";

fn toggle_err_core_module_cannot_be_disabled(module_slug: &str) -> String {
    format!("Core module cannot be disabled: {module_slug}")
}

fn toggle_err_missing_dependencies(missing: &str) -> String {
    format!("Missing module dependencies: {missing}")
}

fn toggle_err_has_dependents(dependents: &str) -> String {
    format!("Module is required by: {dependents}")
}

fn toggle_err_hook_failed(reason: &str) -> String {
    format!("Module lifecycle hook failed: {reason}")
}

#[cfg(test)]
fn map_custom_field_error(error: rustok_core::field_schema::FlexError) -> FieldError {
    match error {
        rustok_core::field_schema::FlexError::ValidationFailed(errors) => {
            let messages: Vec<String> = errors
                .iter()
                .map(|e| format!("{}: {}", e.field_key, e.message))
                .collect();
            FieldError::new(format!(
                "Custom field validation failed: {}",
                messages.join("; ")
            ))
            .extend_with(|_, ext| {
                ext.set("code", "CUSTOM_FIELD_VALIDATION_FAILED");
                if let Ok(v) = serde_json::to_value(&errors)
                    && let Ok(gql_value) = async_graphql::Value::from_json(v)
                {
                    ext.set("fields", gql_value);
                }
            })
        }
        other => <FieldError as GraphQLError>::internal_error(&other.to_string()),
    }
}

fn effective_request_locale(ctx: &Context<'_>, tenant: &TenantContext) -> String {
    ctx.data_opt::<RequestContext>()
        .map(|request| request.locale.clone())
        .unwrap_or_else(|| tenant.default_locale.clone())
}

#[cfg(test)]
async fn prepare_user_custom_fields_write(
    db: &sea_orm::DatabaseConnection,
    tenant_id: uuid::Uuid,
    locale: &str,
    entity_id: Option<Uuid>,
    existing_metadata: Option<&serde_json::Value>,
    custom_fields: Option<serde_json::Value>,
) -> Result<PreparedAttachedValuesWrite> {
    let prepared = match (entity_id, existing_metadata) {
        (Some(entity_id), Some(existing_metadata)) => {
            FlexAttachedValuesService::prepare_update(
                db,
                tenant_id,
                "user",
                entity_id,
                locale,
                existing_metadata,
                custom_fields,
            )
            .await
        }
        _ => {
            FlexAttachedValuesService::prepare_create(db, tenant_id, "user", locale, custom_fields)
                .await
        }
    };

    prepared.map_err(map_custom_field_error)
}

#[cfg(test)]
async fn validate_custom_fields(
    db: &sea_orm::DatabaseConnection,
    tenant_id: uuid::Uuid,
    custom_fields: Option<serde_json::Value>,
) -> Result<Option<serde_json::Value>> {
    Ok(prepare_user_custom_fields_write(
        db,
        tenant_id,
        rustok_api::PLATFORM_FALLBACK_LOCALE,
        None,
        None,
        custom_fields,
    )
    .await?
    .metadata)
}

#[cfg(test)]
fn map_create_user_error(err: AuthLifecycleError) -> FieldError {
    match err {
        AuthLifecycleError::EmailAlreadyExists => {
            FieldError::new("User with this email already exists")
        }
        AuthLifecycleError::Internal(inner) => {
            <FieldError as GraphQLError>::internal_error(&inner.to_string())
        }
        _ => <FieldError as GraphQLError>::internal_error("Failed to create user"),
    }
}

fn user_mutation_runtime(ctx: &Context<'_>) -> Result<UserAdminMutationRuntime> {
    ctx.data::<Arc<ModuleRuntimeExtensions>>()?
        .get::<UserAdminMutationRuntime>()
        .cloned()
        .ok_or_else(|| {
            <FieldError as GraphQLError>::internal_error(
                "UserAdminMutationRuntime is not registered; initialize shared host runtime providers",
            )
        })
}

fn map_user_mutation_error(error: AuthAdminMutationError) -> FieldError {
    match error {
        AuthAdminMutationError::Unauthorized => <FieldError as GraphQLError>::unauthenticated(),
        AuthAdminMutationError::Forbidden(message) => {
            <FieldError as GraphQLError>::permission_denied(&message)
        }
        AuthAdminMutationError::Validation(message) | AuthAdminMutationError::Conflict(message) => {
            <FieldError as GraphQLError>::bad_user_input(&message)
        }
        AuthAdminMutationError::CustomFieldsValidation(fields) => {
            FieldError::new("Custom field validation failed").extend_with(|_, ext| {
                ext.set("code", "CUSTOM_FIELD_VALIDATION_FAILED");
                if let Ok(value) = async_graphql::Value::from_json(fields) {
                    ext.set("fields", value);
                }
            })
        }
        AuthAdminMutationError::NotFound(message) => {
            <FieldError as GraphQLError>::not_found(&message)
        }
        AuthAdminMutationError::Internal(message) => {
            <FieldError as GraphQLError>::internal_error(&message)
        }
    }
}

fn user_mutation_context(
    auth: &AuthContext,
    tenant: &TenantContext,
    locale: String,
) -> AuthAdminMutationContext {
    AuthAdminMutationContext {
        actor_id: auth.user_id,
        tenant_id: tenant.id,
        request_id: None,
        locale: Some(locale),
    }
}

fn user_from_mutation_record(record: UserMutationRecord) -> User {
    User {
        id: record.id,
        email: record.email,
        name: record.name,
        status: record.status,
        created_at: record.created_at.to_rfc3339(),
        tenant_id: record.tenant_id,
        metadata: record.metadata,
    }
}

fn map_manifest_error(err: ManifestError) -> FieldError {
    match err {
        ManifestError::UnknownModule(_)
        | ManifestError::ModuleAlreadyInstalled(_)
        | ManifestError::ModuleNotInstalled(_)
        | ManifestError::RequiredModule(_)
        | ManifestError::HasDependents { .. }
        | ManifestError::MissingDependencies { .. }
        | ManifestError::UnknownDefaultEnabled(_)
        | ManifestError::VersionUnchanged(_, _)
        | ManifestError::InvalidVersion
        | ManifestError::InvalidBuildSurface(_)
        | ManifestError::MissingInRegistry(_)
        | ManifestError::RequiredMismatch(_)
        | ManifestError::DependencyMismatch(_)
        | ManifestError::MissingModulePackageManifest { .. }
        | ManifestError::ModulePackageSlugMismatch { .. }
        | ManifestError::InvalidModuleVersion { .. }
        | ManifestError::InvalidModuleDependency { .. }
        | ManifestError::InvalidModuleConflict { .. }
        | ManifestError::InvalidDependencyVersionReq { .. }
        | ManifestError::MissingDependencyVersion { .. }
        | ManifestError::IncompatibleDependencyVersion { .. }
        | ManifestError::ConflictingModule { .. }
        | ManifestError::IncompatibleRustokVersion { .. }
        | ManifestError::InvalidModuleOwnership { .. }
        | ManifestError::InvalidModuleTrustLevel { .. }
        | ManifestError::InvalidModuleUiClassification { .. }
        | ManifestError::InvalidModuleAdminSurface { .. }
        | ManifestError::ConflictingModuleAdminSurface { .. }
        | ManifestError::InvalidModuleSettingKey { .. }
        | ManifestError::InvalidModuleSettingSchema { .. }
        | ManifestError::InvalidModuleSettingValue { .. }
        | ManifestError::InvalidModuleMarketplaceMetadata { .. }
        | ManifestError::InvalidModuleUiWiring { .. }
        | ManifestError::InvalidModuleHttpWiring { .. } => {
            <FieldError as GraphQLError>::bad_user_input(&err.to_string())
        }
        ManifestError::Read { .. }
        | ManifestError::Parse { .. }
        | ManifestError::Write { .. }
        | ManifestError::ModulePackageRead { .. }
        | ManifestError::ModulePackageParse { .. } => {
            <FieldError as GraphQLError>::internal_error(&err.to_string())
        }
    }
}

async fn ensure_modules_manage_permission(
    ctx: &Context<'_>,
) -> Result<(AuthContext, TenantContext)> {
    let auth = ctx
        .data::<AuthContext>()
        .map_err(|_| <FieldError as GraphQLError>::unauthenticated())?
        .clone();
    let tenant = ctx.data::<TenantContext>()?.clone();
    let db = ctx.data::<DatabaseConnection>()?;

    let can_manage_modules =
        RbacService::has_permission(db, &tenant.id, &auth.user_id, &Permission::MODULES_MANAGE)
            .await
            .map_err(|err| <FieldError as GraphQLError>::internal_error(&err.to_string()))?;

    if !can_manage_modules {
        return Err(<FieldError as GraphQLError>::permission_denied(
            "Permission denied: modules:manage required",
        ));
    }

    Ok((auth, tenant))
}

/// Platform-native composition changes have global effect. They are therefore
/// available only to a direct SuperAdmin authenticated for the current tenant;
/// ordinary tenant `modules:manage` permission is intentionally insufficient.
async fn ensure_platform_composition_operator(ctx: &Context<'_>) -> Result<AuthContext> {
    let auth = ctx
        .data::<AuthContext>()
        .map_err(|_| <FieldError as GraphQLError>::unauthenticated())?
        .clone();
    let tenant = ctx.data::<TenantContext>()?.clone();
    let principal_context = *ctx
        .data::<AuthPrincipalContext>()
        .map_err(|_| <FieldError as GraphQLError>::unauthenticated())?;
    let db = ctx.data::<DatabaseConnection>()?;
    let role = RbacService::get_user_role(db, &tenant.id, &auth.user_id)
        .await
        .map_err(|error| <FieldError as GraphQLError>::internal_error(&error.to_string()))?;
    let can_manage_modules =
        RbacService::has_permission(db, &tenant.id, &auth.user_id, &Permission::MODULES_MANAGE)
            .await
            .map_err(|error| <FieldError as GraphQLError>::internal_error(&error.to_string()))?;
    require_platform_composition_operator(
        &auth,
        principal_context,
        tenant.id,
        role,
        can_manage_modules,
    )?;

    Ok(auth)
}

fn require_platform_composition_operator(
    auth: &AuthContext,
    principal_context: AuthPrincipalContext,
    tenant_id: Uuid,
    role: UserRole,
    can_manage_modules: bool,
) -> Result<()> {
    let principal = RbacControlPlanePrincipal {
        tenant_id: auth.tenant_id,
        principal_kind: principal_context.kind,
    };
    require_direct_control_plane_user(principal, tenant_id).map_err(|error| {
        <FieldError as GraphQLError>::permission_denied(&format!(
            "Platform composition requires a direct SuperAdmin: {error}"
        ))
    })?;
    if role != UserRole::SuperAdmin {
        return Err(<FieldError as GraphQLError>::permission_denied(
            "Platform composition requires the SuperAdmin role",
        ));
    }
    if !can_manage_modules {
        return Err(<FieldError as GraphQLError>::permission_denied(
            "Permission denied: modules:manage required",
        ));
    }
    Ok(())
}

fn artifact_lifecycle_expected_revision(
    installation_id: Uuid,
    expected_revision: i64,
    reason: &str,
    idempotency_key: Uuid,
) -> Result<u64> {
    if installation_id.is_nil() || idempotency_key.is_nil() || expected_revision <= 0 {
        return Err(<FieldError as GraphQLError>::bad_user_input(
            "Artifact lifecycle requires non-nil installation and idempotency identities plus a positive expected revision",
        ));
    }
    if reason.trim().is_empty() {
        return Err(<FieldError as GraphQLError>::bad_user_input(
            "Artifact lifecycle requires a non-empty reason",
        ));
    }
    u64::try_from(expected_revision).map_err(|_| {
        <FieldError as GraphQLError>::bad_user_input(
            "Artifact lifecycle revision is outside the supported range",
        )
    })
}

fn artifact_lifecycle_revision(revision: u64) -> Result<i64> {
    i64::try_from(revision).map_err(|_| {
        <FieldError as GraphQLError>::internal_error(
            "Artifact lifecycle revision is outside the GraphQL range",
        )
    })
}

fn tenant_artifact_scope(tenant_id: Uuid) -> ModuleInstallationScope {
    ModuleInstallationScope::Tenant { tenant_id }
}

/// The GraphQL adapter preserves the active distributed trace when one exists.
/// A deterministic local root is retained for deployments that have not yet
/// installed a tracing subscriber, so every durable owner command remains
/// traceable and idempotent without a transport-specific fallback DTO.
pub(crate) fn module_command_context(
    actor_id: Uuid,
    tenant_id: Option<Uuid>,
    idempotency_key: Uuid,
) -> ModuleCommandContext {
    let trace_id = rustok_telemetry::current_trace_id()
        .filter(|trace_id| !trace_id.trim().is_empty())
        .unwrap_or_else(|| format!("graphql:{idempotency_key}"));
    ModuleCommandContext {
        actor_id,
        tenant_id,
        trace_id,
        correlation_id: idempotency_key,
        idempotency_key,
    }
}

async fn request_module_composition_build(
    runtime_ctx: &ServerRuntimeContext,
    registry: &ModuleRegistry,
    mutation: PlatformCompositionModuleMutation,
) -> Result<BuildJob> {
    let event_publisher = Arc::new(CompositeBuildEventPublisher::new(vec![
        Arc::new(BuildEventHubPublisher::new(build_event_hub_from_context(
            runtime_ctx,
        ))),
        Arc::new(EventBusBuildEventPublisher::new(
            event_bus_from_context(runtime_ctx),
            BuildEventScope::Platform,
            BuildEventPublicationContext {
                actor_id: mutation.context.actor_id,
                correlation_id: mutation.context.correlation_id,
                trace_id: mutation.context.trace_id.clone(),
            },
        )),
    ]));

    let result = PlatformCompositionBuildService::apply_module_mutation_and_request_build(
        runtime_ctx.db(),
        event_publisher,
        registry,
        mutation,
    )
    .await
    .map_err(map_platform_composition_build_error)?;

    Ok(BuildJob::from_snapshot(&rustok_build::build_snapshot(
        &result.build,
    )))
}

fn map_platform_composition_build_error(error: PlatformCompositionBuildError) -> FieldError {
    match error {
        PlatformCompositionBuildError::Composition(error) => map_platform_composition_error(error),
        PlatformCompositionBuildError::Build(error) => {
            <FieldError as GraphQLError>::internal_error(&error)
        }
    }
}

fn map_toggle_module_error(error: ToggleModuleError) -> FieldError {
    match error {
        ToggleModuleError::InvalidCommandIdentity => <FieldError as GraphQLError>::bad_user_input(
            "Module lifecycle command identity is invalid",
        ),
        ToggleModuleError::InvalidIdempotencyKey => <FieldError as GraphQLError>::bad_user_input(
            "Module lifecycle idempotency key is invalid",
        ),
        ToggleModuleError::IdempotencyConflict => {
            FieldError::new("Module lifecycle idempotency key was reused for a different command")
                .extend_with(|_, ext| {
                    ext.set("code", "IDEMPOTENCY_CONFLICT");
                    ext.set("retryable_issue", false);
                })
        }
        ToggleModuleError::RevisionConflict { expected, current } => FieldError::new(format!(
            "Static module lifecycle changed since revision {expected}; current revision is {current}",
        ))
        .extend_with(|_, ext| {
            ext.set("code", "REVISION_CONFLICT");
            ext.set("retryable_issue", false);
            ext.set("expected_revision", expected);
            ext.set("current_revision", current);
        }),
        ToggleModuleError::OperationInProgress => {
            FieldError::new("A static module lifecycle operation is already active")
                .extend_with(|_, ext| {
                    ext.set("code", "MODULE_LIFECYCLE_OPERATION_IN_PROGRESS");
                    ext.set("retryable_issue", false);
                })
        }
        ToggleModuleError::UnknownModule => {
            <FieldError as GraphQLError>::bad_user_input(TOGGLE_ERR_UNKNOWN_MODULE)
        }
        ToggleModuleError::CoreModuleCannotBeDisabled(module_slug) => {
            <FieldError as GraphQLError>::bad_user_input(
                &toggle_err_core_module_cannot_be_disabled(&module_slug),
            )
        }
        ToggleModuleError::MissingDependencies(missing) => {
            <FieldError as GraphQLError>::bad_user_input(&toggle_err_missing_dependencies(&missing))
        }
        ToggleModuleError::HasDependents(dependents) => {
            <FieldError as GraphQLError>::bad_user_input(&toggle_err_has_dependents(&dependents))
        }
        ToggleModuleError::Database(_) => {
            <FieldError as GraphQLError>::internal_error("Internal server error")
        }
        ToggleModuleError::PreHookFailed(err) => FieldError::new(toggle_err_hook_failed(&err))
            .extend_with(|_, ext| {
                ext.set("code", "MODULE_HOOK_FAILED");
                ext.set("retryable_issue", false);
                ext.set("operation_issue", "pre_hook_failed");
            }),
        ToggleModuleError::PostHookFailed(err) => FieldError::new(toggle_err_hook_failed(&err))
            .extend_with(|_, ext| {
                ext.set("code", "MODULE_HOOK_FAILED");
                ext.set("retryable_issue", true);
                ext.set("operation_issue", "post_hook_failed");
            }),
        ToggleModuleError::Policy(_) => {
            <FieldError as GraphQLError>::internal_error("Internal server error")
        }
    }
}

fn map_module_operation_recovery_error(error: ModuleOperationRecoveryError) -> FieldError {
    match error {
        ModuleOperationRecoveryError::OperationNotFound => {
            <FieldError as GraphQLError>::bad_user_input("Module operation not found")
        }
        ModuleOperationRecoveryError::InvalidCommandIdentity => {
            <FieldError as GraphQLError>::bad_user_input(
                "Module recovery command identity is invalid",
            )
        }
        ModuleOperationRecoveryError::InvalidIdempotencyKey => {
            <FieldError as GraphQLError>::bad_user_input(
                "Module operation idempotency key is invalid",
            )
        }
        ModuleOperationRecoveryError::NotRetryable(reason) => {
            FieldError::new(format!("Module operation is not retryable: {reason}"))
                .extend_with(|_, ext| {
                    ext.set("code", "MODULE_OPERATION_NOT_RETRYABLE");
                    ext.set("retryable_issue", false);
                })
        }
        ModuleOperationRecoveryError::StateMismatch {
            requested_enabled,
            current_enabled,
        } => FieldError::new(format!(
            "Module operation state mismatch: requested enabled={requested_enabled}, current enabled={current_enabled}"
        ))
        .extend_with(|_, ext| {
            ext.set("code", "MODULE_OPERATION_STATE_MISMATCH");
            ext.set("retryable_issue", false);
        }),
        ModuleOperationRecoveryError::PostHookFailed(err) => {
            FieldError::new(format!("Module hook failed: {err}"))
                .extend_with(|_, ext| {
                    ext.set("code", "MODULE_HOOK_FAILED");
                    ext.set("retryable_issue", true);
                    ext.set("operation_issue", "post_hook_failed");
                })
        }
        ModuleOperationRecoveryError::IdempotencyConflict => FieldError::new(
            "Module operation idempotency key was reused for a different command",
        )
        .extend_with(|_, ext| {
            ext.set("code", "IDEMPOTENCY_CONFLICT");
            ext.set("retryable_issue", false);
        }),
        ModuleOperationRecoveryError::RevisionConflict { expected, current } => FieldError::new(
            format!(
                "Static module lifecycle changed since revision {expected}; current revision is {current}",
            ),
        )
        .extend_with(|_, ext| {
            ext.set("code", "REVISION_CONFLICT");
            ext.set("retryable_issue", false);
            ext.set("expected_revision", expected);
            ext.set("current_revision", current);
        }),
        ModuleOperationRecoveryError::OperationInProgress => {
            FieldError::new("A static module lifecycle operation is already active")
                .extend_with(|_, ext| {
                    ext.set("code", "MODULE_LIFECYCLE_OPERATION_IN_PROGRESS");
                    ext.set("retryable_issue", false);
                })
        }
        ModuleOperationRecoveryError::Database(err) => {
            <FieldError as GraphQLError>::internal_error(&err.to_string())
        }
        ModuleOperationRecoveryError::Policy(err) => {
            <FieldError as GraphQLError>::internal_error(&err)
        }
        ModuleOperationRecoveryError::Toggle(err) => map_toggle_module_error(err),
    }
}

fn map_platform_composition_error(error: PlatformCompositionError) -> FieldError {
    match error {
        PlatformCompositionError::RevisionConflict { expected, current }
        | PlatformCompositionError::Owner(ModuleCompositionError::RevisionConflict {
            expected,
            current,
        }) => <FieldError as GraphQLError>::bad_user_input(&format!(
            "Platform composition revision conflict: expected {expected}, current {current}"
        )),
        PlatformCompositionError::Owner(ModuleCompositionError::OperationReceipt(error)) => {
            map_composition_operation_receipt_error(error)
        }
        other @ (PlatformCompositionError::Owner(
            ModuleCompositionError::InvalidExpectedRevision,
        )
        | PlatformCompositionError::Owner(ModuleCompositionError::InvalidOperationScope)) => {
            <FieldError as GraphQLError>::bad_user_input(&other.to_string())
        }
        PlatformCompositionError::Manifest(error) => map_manifest_error(error),
        other => <FieldError as GraphQLError>::internal_error(&other.to_string()),
    }
}

fn map_composition_operation_receipt_error(error: PortError) -> FieldError {
    match error.kind {
        PortErrorKind::Validation | PortErrorKind::Conflict => {
            <FieldError as GraphQLError>::bad_user_input(&error.message)
        }
        PortErrorKind::NotFound => <FieldError as GraphQLError>::not_found(&error.message),
        PortErrorKind::Forbidden => <FieldError as GraphQLError>::permission_denied(&error.message),
        PortErrorKind::Unavailable | PortErrorKind::Timeout => {
            <FieldError as GraphQLError>::internal_error(
                "Module composition service is temporarily unavailable",
            )
        }
        PortErrorKind::InvariantViolation => <FieldError as GraphQLError>::internal_error(
            "Module composition operation requires operator review",
        ),
    }
}

fn map_artifact_ui_action_error(error: ServerError) -> FieldError {
    match error {
        ServerError::NotFound => {
            <FieldError as GraphQLError>::not_found("Artifact UI action is unavailable")
        }
        ServerError::BadRequest(_) | ServerError::Validation(_) => {
            <FieldError as GraphQLError>::bad_user_input("Artifact UI action input is invalid")
        }
        ServerError::Http(error) if error.status == StatusCode::FORBIDDEN => {
            <FieldError as GraphQLError>::permission_denied(
                "Permission denied for artifact UI action",
            )
        }
        ServerError::Http(error) if error.status == StatusCode::CONFLICT => {
            FieldError::new("Artifact UI action conflicts with the current operation state")
                .extend_with(|_, extensions| {
                    extensions.set("code", "ARTIFACT_UI_ACTION_CONFLICT");
                    extensions.set("retryable_issue", false);
                })
        }
        ServerError::Http(error)
            if error.status == StatusCode::PAYLOAD_TOO_LARGE
                || error.status == StatusCode::UNSUPPORTED_MEDIA_TYPE =>
        {
            <FieldError as GraphQLError>::bad_user_input("Artifact UI action input is invalid")
        }
        error => {
            tracing::error!(%error, "artifact UI action GraphQL mutation failed");
            <FieldError as GraphQLError>::internal_error("Artifact UI action is unavailable")
        }
    }
}

fn map_artifact_settings_recovery_error(err: ArtifactSettingsRecoveryError) -> FieldError {
    match err {
        ArtifactSettingsRecoveryError::PolicyDenied => {
            <FieldError as GraphQLError>::permission_denied("Policy denied artifact settings recovery operation")
        }
        ArtifactSettingsRecoveryError::RecoveryUnavailable
        | ArtifactSettingsRecoveryError::InstallationUnavailable
        | ArtifactSettingsRecoveryError::SettingsUnavailable => {
            <FieldError as GraphQLError>::not_found("Requested artifact recovery resource not found")
        }
        ArtifactSettingsRecoveryError::RecoveryPrecondition
        | ArtifactSettingsRecoveryError::PurgePrecondition
        | ArtifactSettingsRecoveryError::RestorePrecondition
        | ArtifactSettingsRecoveryError::RetentionPrecondition
        | ArtifactSettingsRecoveryError::RewrapPrecondition => {
            <FieldError as GraphQLError>::bad_user_input(&format!("Recovery precondition failed: {err}"))
        }
        ArtifactSettingsRecoveryError::IdempotencyConflict => {
            <FieldError as GraphQLError>::bad_user_input("Idempotency key conflict")
        }
        ArtifactSettingsRecoveryError::CiphertextIntegrity => {
            <FieldError as GraphQLError>::bad_user_input("Ciphertext integrity verification failed")
        }
        other => <FieldError as GraphQLError>::internal_error(&other.to_string()),
    }
}

fn map_artifact_data_purge_error(err: ArtifactDataError) -> FieldError {
    match err {
        ArtifactDataError::PurgePrecondition => {
            <FieldError as GraphQLError>::bad_user_input(
                "Data purge precondition failed: namespace must be uninstalled/retired and reason non-empty",
            )
        }
        ArtifactDataError::NamespacePurged => {
            <FieldError as GraphQLError>::bad_user_input("Data namespace has already been purged")
        }
        other => <FieldError as GraphQLError>::internal_error(&other.to_string()),
    }
}

#[Object]
impl RootMutation {
    async fn create_user(&self, ctx: &Context<'_>, input: CreateUserInput) -> Result<User> {
        let auth = ctx
            .data::<AuthContext>()
            .map_err(|_| <FieldError as GraphQLError>::unauthenticated())?;
        let tenant = ctx.data::<TenantContext>()?;
        let locale = effective_request_locale(ctx, tenant);
        user_mutation_runtime(ctx)?
            .port()
            .create_user(
                &user_mutation_context(auth, tenant, locale),
                CreateUserCommand {
                    email: input.email,
                    password: input.password,
                    name: input.name,
                    role: input
                        .role
                        .map(|role| rustok_core::UserRole::from(role).to_string()),
                    status: input
                        .status
                        .map(|status| rustok_core::UserStatus::from(status).to_string()),
                    custom_fields: input.custom_fields,
                },
            )
            .await
            .map(user_from_mutation_record)
            .map_err(map_user_mutation_error)
    }

    async fn update_user(
        &self,
        ctx: &Context<'_>,
        id: uuid::Uuid,
        input: UpdateUserInput,
    ) -> Result<User> {
        let auth = ctx
            .data::<AuthContext>()
            .map_err(|_| <FieldError as GraphQLError>::unauthenticated())?;
        let tenant = ctx.data::<TenantContext>()?;
        let locale = effective_request_locale(ctx, tenant);
        user_mutation_runtime(ctx)?
            .port()
            .update_user(
                &user_mutation_context(auth, tenant, locale),
                UpdateUserCommand {
                    id,
                    email: input.email,
                    password: input.password,
                    name: input.name,
                    role: input
                        .role
                        .map(|role| rustok_core::UserRole::from(role).to_string()),
                    status: input
                        .status
                        .map(|status| rustok_core::UserStatus::from(status).to_string()),
                    custom_fields: input.custom_fields,
                },
            )
            .await
            .map(user_from_mutation_record)
            .map_err(map_user_mutation_error)
    }

    async fn disable_user(&self, ctx: &Context<'_>, id: uuid::Uuid) -> Result<User> {
        let auth = ctx
            .data::<AuthContext>()
            .map_err(|_| <FieldError as GraphQLError>::unauthenticated())?;
        let tenant = ctx.data::<TenantContext>()?;
        let db = ctx.data::<DatabaseConnection>()?;

        let can_manage_users = RbacService::has_permission(
            db,
            &tenant.id,
            &auth.user_id,
            &rustok_api::Permission::USERS_MANAGE,
        )
        .await
        .map_err(|err| <FieldError as GraphQLError>::internal_error(&err.to_string()))?;

        if !can_manage_users {
            return Err(<FieldError as GraphQLError>::permission_denied(
                "Permission denied: users:manage required",
            ));
        }

        let user = users::Entity::find_by_id(id)
            .filter(UsersColumn::TenantId.eq(tenant.id))
            .one(db)
            .await
            .map_err(|err| <FieldError as GraphQLError>::internal_error(&err.to_string()))?
            .ok_or_else(|| FieldError::new("User not found"))?;

        let mut model: users::ActiveModel = user.into();
        model.status = Set(rustok_core::UserStatus::Inactive);

        let user = model
            .update(db)
            .await
            .map_err(|err| <FieldError as GraphQLError>::internal_error(&err.to_string()))?;

        Ok(User::from(&user))
    }

    async fn delete_user(&self, ctx: &Context<'_>, id: uuid::Uuid) -> Result<DeleteUserPayload> {
        let auth = ctx
            .data::<AuthContext>()
            .map_err(|_| <FieldError as GraphQLError>::unauthenticated())?;
        let tenant = ctx.data::<TenantContext>()?;
        let locale = effective_request_locale(ctx, tenant);
        user_mutation_runtime(ctx)?
            .port()
            .delete_user(&user_mutation_context(auth, tenant, locale), id)
            .await
            .map_err(map_user_mutation_error)?;
        Ok(DeleteUserPayload { success: true })
    }

    async fn install_module(
        &self,
        ctx: &Context<'_>,
        slug: String,
        version: String,
        expected_revision: i64,
        idempotency_key: Uuid,
    ) -> Result<BuildJob> {
        let auth = ensure_platform_composition_operator(ctx).await?;
        let runtime_ctx = ctx.data::<ServerRuntimeContext>()?;
        let registry = ctx.data::<ModuleRegistry>()?;

        request_module_composition_build(
            runtime_ctx,
            registry,
            PlatformCompositionModuleMutation {
                context: module_command_context(auth.user_id, None, idempotency_key),
                expected_revision,
                change: PlatformCompositionModuleChange::Install {
                    module_slug: slug,
                    version,
                },
            },
        )
        .await
    }

    async fn uninstall_module(
        &self,
        ctx: &Context<'_>,
        slug: String,
        expected_revision: i64,
        idempotency_key: Uuid,
    ) -> Result<BuildJob> {
        let auth = ensure_platform_composition_operator(ctx).await?;
        let runtime_ctx = ctx.data::<ServerRuntimeContext>()?;
        let registry = ctx.data::<ModuleRegistry>()?;

        request_module_composition_build(
            runtime_ctx,
            registry,
            PlatformCompositionModuleMutation {
                context: module_command_context(auth.user_id, None, idempotency_key),
                expected_revision,
                change: PlatformCompositionModuleChange::Uninstall { module_slug: slug },
            },
        )
        .await
    }

    async fn upgrade_module(
        &self,
        ctx: &Context<'_>,
        slug: String,
        version: String,
        expected_revision: i64,
        idempotency_key: Uuid,
    ) -> Result<BuildJob> {
        let auth = ensure_platform_composition_operator(ctx).await?;
        let runtime_ctx = ctx.data::<ServerRuntimeContext>()?;
        let registry = ctx.data::<ModuleRegistry>()?;

        request_module_composition_build(
            runtime_ctx,
            registry,
            PlatformCompositionModuleMutation {
                context: module_command_context(auth.user_id, None, idempotency_key),
                expected_revision,
                change: PlatformCompositionModuleChange::Upgrade {
                    module_slug: slug,
                    version,
                },
            },
        )
        .await
    }

    async fn toggle_module(
        &self,
        ctx: &Context<'_>,
        module_slug: String,
        enabled: bool,
        expected_revision: i64,
        idempotency_key: Uuid,
    ) -> Result<TenantModule> {
        let (auth, tenant) = ensure_modules_manage_permission(ctx).await?;
        if idempotency_key.is_nil() {
            return Err(<FieldError as GraphQLError>::bad_user_input(
                "Module lifecycle idempotency key must not be nil",
            ));
        }
        if expected_revision < 0 {
            return Err(<FieldError as GraphQLError>::bad_user_input(
                "Static module lifecycle revision must not be negative",
            ));
        }
        let expected_revision = u64::try_from(expected_revision).map_err(|_| {
            <FieldError as GraphQLError>::bad_user_input(
                "Static module lifecycle revision is outside the supported range",
            )
        })?;
        let db = ctx.data::<DatabaseConnection>()?;
        let registry = ctx.data::<ModuleRegistry>()?;

        let module = ModuleLifecycleService::toggle_module(
            db,
            registry,
            tenant.id,
            &module_slug,
            enabled,
            module_command_context(auth.user_id, Some(tenant.id), idempotency_key),
            expected_revision,
        )
        .await
        .map_err(map_toggle_module_error)?;

        Ok(TenantModule {
            module_slug: module.module_slug,
            enabled: module.enabled,
            settings: module.settings.to_string(),
            revision: i64::try_from(module.revision).map_err(|_| {
                <FieldError as GraphQLError>::internal_error(
                    "Static module lifecycle revision is outside the GraphQL range",
                )
            })?,
        })
    }

    async fn set_artifact_tenant_enabled(
        &self,
        ctx: &Context<'_>,
        installation_id: Uuid,
        enabled: bool,
        expected_revision: i64,
        reason: String,
        idempotency_key: Uuid,
    ) -> Result<ArtifactTenantLifecycle> {
        let (auth, tenant) = ensure_modules_manage_permission(ctx).await?;
        if installation_id.is_nil() || idempotency_key.is_nil() || expected_revision <= 0 {
            return Err(<FieldError as GraphQLError>::bad_user_input(
                "Artifact tenant lifecycle requires non-nil installation and idempotency identities plus a positive expected revision",
            ));
        }
        if reason.trim().is_empty() {
            return Err(<FieldError as GraphQLError>::bad_user_input(
                "Artifact tenant lifecycle requires a non-empty reason",
            ));
        }
        let expected_revision = u64::try_from(expected_revision).map_err(|_| {
            <FieldError as GraphQLError>::bad_user_input(
                "Artifact tenant lifecycle revision is outside the supported range",
            )
        })?;
        let db = ctx.data::<DatabaseConnection>()?;
        let installation = ModuleControlPlane::new(db.clone()).installation();
        let command_context =
            module_command_context(auth.user_id, Some(tenant.id), idempotency_key);
        let revision = if enabled {
            installation
                .enable_artifact_for_tenant(ArtifactTenantEnableRequest {
                    installation_id,
                    tenant_id: tenant.id,
                    expected_revision,
                    context: command_context,
                    reason,
                })
                .await
                .map(|result| result.revision)
        } else {
            installation
                .disable_artifact_for_tenant(ArtifactTenantDisableRequest {
                    installation_id,
                    tenant_id: tenant.id,
                    expected_revision,
                    context: command_context,
                    reason,
                })
                .await
                .map(|result| result.revision)
        }
        .map_err(map_artifact_tenant_lifecycle_error)?;
        let revision = i64::try_from(revision).map_err(|_| {
            <FieldError as GraphQLError>::internal_error(
                "Artifact tenant lifecycle revision is outside the GraphQL range",
            )
        })?;
        Ok(ArtifactTenantLifecycle {
            installation_id,
            enabled,
            revision,
            expected_revision: revision,
        })
    }

    /// Activates one admitted artifact only in the authenticated tenant scope.
    /// Platform-scope activation has no tenant-derived authorization fallback.
    async fn activate_tenant_artifact(
        &self,
        ctx: &Context<'_>,
        installation_id: Uuid,
        expected_revision: i64,
        reason: String,
        idempotency_key: Uuid,
    ) -> Result<ArtifactActivation> {
        let (auth, tenant) = ensure_modules_manage_permission(ctx).await?;
        let expected_revision = artifact_lifecycle_expected_revision(
            installation_id,
            expected_revision,
            &reason,
            idempotency_key,
        )?;
        let db = ctx.data::<DatabaseConnection>()?;
        let result = ModuleControlPlane::new(db.clone())
            .installation()
            .activate_artifact(ArtifactActivationRequest {
                installation_id,
                scope: tenant_artifact_scope(tenant.id),
                expected_revision,
                context: module_command_context(auth.user_id, Some(tenant.id), idempotency_key),
                reason,
            })
            .await
            .map_err(map_artifact_installation_lifecycle_error)?;

        Ok(ArtifactActivation {
            installation_id,
            operation_id: result.operation_id,
            predecessor_installation_id: result.predecessor_installation_id,
            installation_revision: artifact_lifecycle_revision(result.installation_revision)?,
            predecessor_revision: result
                .predecessor_revision
                .map(artifact_lifecycle_revision)
                .transpose()?,
        })
    }

    /// Removes runtime bindings for one active artifact only in the authenticated
    /// tenant scope. It does not delete retained data or evidence.
    async fn deactivate_tenant_artifact(
        &self,
        ctx: &Context<'_>,
        installation_id: Uuid,
        expected_revision: i64,
        reason: String,
        idempotency_key: Uuid,
    ) -> Result<ArtifactDeactivation> {
        let (auth, tenant) = ensure_modules_manage_permission(ctx).await?;
        let expected_revision = artifact_lifecycle_expected_revision(
            installation_id,
            expected_revision,
            &reason,
            idempotency_key,
        )?;
        let db = ctx.data::<DatabaseConnection>()?;
        let result = ModuleControlPlane::new(db.clone())
            .installation()
            .deactivate_artifact(ArtifactDeactivationRequest {
                installation_id,
                scope: tenant_artifact_scope(tenant.id),
                expected_revision,
                context: module_command_context(auth.user_id, Some(tenant.id), idempotency_key),
                reason,
            })
            .await
            .map_err(map_artifact_installation_lifecycle_error)?;

        Ok(ArtifactDeactivation {
            installation_id,
            operation_id: result.operation_id,
            revision: artifact_lifecycle_revision(result.revision)?,
        })
    }

    /// Removes one inactive artifact selection only in the authenticated tenant
    /// scope. CAS and retained data collection remain separate owner policies.
    async fn uninstall_tenant_artifact(
        &self,
        ctx: &Context<'_>,
        installation_id: Uuid,
        expected_revision: i64,
        reason: String,
        idempotency_key: Uuid,
    ) -> Result<ArtifactUninstall> {
        let (auth, tenant) = ensure_modules_manage_permission(ctx).await?;
        let expected_revision = artifact_lifecycle_expected_revision(
            installation_id,
            expected_revision,
            &reason,
            idempotency_key,
        )?;
        let db = ctx.data::<DatabaseConnection>()?;
        let result = ModuleControlPlane::new(db.clone())
            .installation()
            .uninstall_artifact(ArtifactUninstallRequest {
                installation_id,
                scope: tenant_artifact_scope(tenant.id),
                expected_revision,
                context: module_command_context(auth.user_id, Some(tenant.id), idempotency_key),
                reason,
            })
            .await
            .map_err(map_artifact_installation_lifecycle_error)?;

        Ok(ArtifactUninstall {
            installation_id,
            operation_id: result.operation_id,
            revision: artifact_lifecycle_revision(result.revision)?,
        })
    }

    /// Rolls back only to the retained direct predecessor in the authenticated
    /// tenant scope. The client cannot supply a target installation selector.
    async fn rollback_tenant_artifact(
        &self,
        ctx: &Context<'_>,
        installation_id: Uuid,
        expected_revision: i64,
        reason: String,
        idempotency_key: Uuid,
        target_capability_grant_revision: i64,
    ) -> Result<ArtifactRollback> {
        let (auth, tenant) = ensure_modules_manage_permission(ctx).await?;
        let expected_revision = artifact_lifecycle_expected_revision(
            installation_id,
            expected_revision,
            &reason,
            idempotency_key,
        )?;
        if target_capability_grant_revision <= 0 {
            return Err(<FieldError as GraphQLError>::bad_user_input(
                "Artifact rollback requires a positive target capability-grant revision",
            ));
        }
        let target_capability_grant_revision = u64::try_from(target_capability_grant_revision)
            .map_err(|_| {
                <FieldError as GraphQLError>::bad_user_input(
                    "Artifact rollback capability-grant revision is outside the supported range",
                )
            })?;
        let db = ctx.data::<DatabaseConnection>()?;
        let result = ModuleControlPlane::new(db.clone())
            .installation()
            .rollback_artifact(ArtifactRollbackRequest {
                installation_id,
                scope: tenant_artifact_scope(tenant.id),
                expected_revision,
                context: module_command_context(auth.user_id, Some(tenant.id), idempotency_key),
                reason,
                target_capability_grant_revision,
            })
            .await
            .map_err(map_artifact_installation_lifecycle_error)?;

        Ok(ArtifactRollback {
            operation_id: result.operation_id,
            source_installation_id: installation_id,
            target_installation_id: result.target_installation_id,
            source_revision: artifact_lifecycle_revision(result.source_revision)?,
            target_revision: artifact_lifecycle_revision(result.target_revision)?,
        })
    }

    /// Executes exactly the admitted Command binding selected by an Action or
    /// Form contribution. The host never receives a raw dynamic-binding
    /// mutation: contribution lookup, dynamic RBAC, schema validation,
    /// idempotency, and audited sandbox execution share the HTTP adapter.
    async fn execute_artifact_ui_action(
        &self,
        ctx: &Context<'_>,
        installation_id: Uuid,
        contribution_id: String,
        input: Json<serde_json::Value>,
        idempotency_key: Uuid,
    ) -> Result<Json<serde_json::Value>> {
        ensure_modules_read_permission(ctx).await?;
        if installation_id.is_nil() || contribution_id.trim().is_empty() || idempotency_key.is_nil()
        {
            return Err(<FieldError as GraphQLError>::bad_user_input(
                "Artifact UI action requires non-nil installation and idempotency identities plus a contribution ID",
            ));
        }
        let auth = ctx
            .data::<AuthContext>()
            .map_err(|_| <FieldError as GraphQLError>::unauthenticated())?;
        let tenant = ctx.data::<TenantContext>()?;
        let runtime_ctx = ctx.data::<ServerRuntimeContext>()?;
        let output = execute_artifact_ui_action_service(
            runtime_ctx,
            tenant.id,
            auth.user_id,
            installation_id,
            &contribution_id,
            input.0,
            Some(idempotency_key),
        )
        .await
        .map_err(map_artifact_ui_action_error)?;
        Ok(Json(output))
    }

    /// Materializes an encrypted settings recovery point for a retired installation.
    /// The source installation must already be inactive/uninstalled.
    async fn create_tenant_artifact_settings_recovery_point(
        &self,
        ctx: &Context<'_>,
        installation_id: Uuid,
        expected_installation_revision: i64,
        expected_settings_revision: i64,
        reason: String,
        idempotency_key: Uuid,
    ) -> Result<ArtifactSettingsRecoveryPointReceipt> {
        let (auth, tenant) = ensure_modules_manage_permission(ctx).await?;
        if idempotency_key.is_nil() || expected_installation_revision < 0 || expected_settings_revision < 0 {
            return Err(<FieldError as GraphQLError>::bad_user_input(
                "Recovery point creation requires non-nil idempotency key and non-negative revisions",
            ));
        }
        let db = ctx.data::<DatabaseConnection>()?;
        let control_plane = ModuleControlPlane::new(db.clone());
        let service = control_plane.artifact_settings_recovery(
            ServerArtifactSettingsRecoveryAuthorizer,
            ServerArtifactSettingsRecoveryCipher,
        );

        let result = service
            .create_recovery_point(ArtifactSettingsRecoveryPointCreateRequest {
                tenant_id: tenant.id,
                installation_id,
                expected_installation_revision: expected_installation_revision as u64,
                expected_settings_revision: expected_settings_revision as u64,
                context: module_command_context(auth.user_id, Some(tenant.id), idempotency_key),
                reason,
            })
            .await
            .map_err(map_artifact_settings_recovery_error)?;

        Ok(ArtifactSettingsRecoveryPointReceipt {
            recovery_point_id: result.recovery_point_id,
            settings_instance_id: result.settings_instance_id,
            settings_revision: result.settings_revision as i64,
            retain_until: result.retain_until.to_rfc3339(),
        })
    }

    /// Purges dynamic artifact settings for a retired installation. Requires an
    /// existing recovery point and is denied if the artifact is active/installed.
    async fn purge_tenant_artifact_settings(
        &self,
        ctx: &Context<'_>,
        installation_id: Uuid,
        recovery_point_id: Uuid,
        expected_installation_revision: i64,
        expected_settings_revision: i64,
        reason: String,
        idempotency_key: Uuid,
    ) -> Result<ArtifactSettingsPurgeReceipt> {
        let (auth, tenant) = ensure_modules_manage_permission(ctx).await?;
        if idempotency_key.is_nil() || expected_installation_revision < 0 || expected_settings_revision < 0 {
            return Err(<FieldError as GraphQLError>::bad_user_input(
                "Settings purge requires non-nil idempotency key and non-negative revisions",
            ));
        }
        let db = ctx.data::<DatabaseConnection>()?;
        let control_plane = ModuleControlPlane::new(db.clone());
        let service = control_plane.artifact_settings_recovery(
            ServerArtifactSettingsRecoveryAuthorizer,
            ServerArtifactSettingsRecoveryCipher,
        );

        let result = service
            .purge(ArtifactSettingsPurgeRequest {
                tenant_id: tenant.id,
                installation_id,
                recovery_point_id,
                expected_installation_revision: expected_installation_revision as u64,
                expected_settings_revision: expected_settings_revision as u64,
                context: module_command_context(auth.user_id, Some(tenant.id), idempotency_key),
                reason,
            })
            .await
            .map_err(map_artifact_settings_recovery_error)?;

        Ok(ArtifactSettingsPurgeReceipt {
            purge_operation_id: result.purge_operation_id,
            recovery_point_id: result.recovery_point_id,
            tombstone_revision: result.tombstone_revision as i64,
        })
    }

    /// Restores settings from a recovery point into a fresh non-serving settings instance.
    async fn restore_tenant_artifact_settings(
        &self,
        ctx: &Context<'_>,
        recovery_point_id: Uuid,
        target_installation_id: Option<Uuid>,
        expected_target_installation_revision: Option<i64>,
        reason: String,
        idempotency_key: Uuid,
    ) -> Result<ArtifactSettingsRestoreReceipt> {
        let (auth, tenant) = ensure_modules_manage_permission(ctx).await?;
        if idempotency_key.is_nil() {
            return Err(<FieldError as GraphQLError>::bad_user_input(
                "Settings restore requires a non-nil idempotency key",
            ));
        }
        let db = ctx.data::<DatabaseConnection>()?;
        let control_plane = ModuleControlPlane::new(db.clone());
        let service = control_plane.artifact_settings_recovery(
            ServerArtifactSettingsRecoveryAuthorizer,
            ServerArtifactSettingsRecoveryCipher,
        );

        let result = service
            .restore(ArtifactSettingsRestoreRequest {
                tenant_id: tenant.id,
                recovery_point_id,
                target_installation_id,
                expected_target_installation_revision: expected_target_installation_revision.map(|r| r as u64),
                context: module_command_context(auth.user_id, Some(tenant.id), idempotency_key),
                reason,
            })
            .await
            .map_err(map_artifact_settings_recovery_error)?;

        Ok(ArtifactSettingsRestoreReceipt {
            restore_operation_id: result.restore_operation_id,
            recovery_point_id: result.recovery_point_id,
            new_settings_instance_id: result.settings_instance_id,
            target_installation_id: result.target_installation_id,
        })
    }

    /// Purges structured data records for a retired artifact namespace.
    /// Denied if the artifact is currently active or installed.
    async fn purge_tenant_artifact_data(
        &self,
        ctx: &Context<'_>,
        module_slug: String,
        data_contract_revision: i64,
        policy_revision: i64,
        expected_namespace_revision: i64,
        reason: String,
        idempotency_key: Uuid,
    ) -> Result<ArtifactDataPurgeReceipt> {
        let (auth, tenant) = ensure_modules_manage_permission(ctx).await?;
        if idempotency_key.is_nil() || expected_namespace_revision < 0 || data_contract_revision < 0 || policy_revision < 0 {
            return Err(<FieldError as GraphQLError>::bad_user_input(
                "Data purge requires non-nil idempotency key and non-negative revisions",
            ));
        }
        let db = ctx.data::<DatabaseConnection>()?;
        let control_plane = ModuleControlPlane::new(db.clone());
        let service = control_plane.artifact_data_purge(ServerArtifactDataPurgeAuthorizer);

        let scope = ArtifactDataScope {
            tenant_id: tenant.id,
            module_slug,
            data_contract_revision: data_contract_revision as u64,
            policy_revision: policy_revision as u64,
        };

        let result = service
            .purge(ArtifactDataPurgeRequest {
                scope,
                expected_namespace_revision: expected_namespace_revision as u64,
                context: module_command_context(auth.user_id, Some(tenant.id), idempotency_key),
                reason,
            })
            .await
            .map_err(map_artifact_data_purge_error)?;

        Ok(ArtifactDataPurgeReceipt {
            namespace_revision: result.namespace_revision as i64,
            purged_records: result.purged_records as i64,
        })
    }

    async fn retry_failed_module_operation_post_hook(
        &self,
        ctx: &Context<'_>,
        operation_id: Uuid,
        idempotency_key: Uuid,
        expected_revision: i64,
    ) -> Result<ModuleOperationRecoveryPlan> {
        let (auth, tenant) = ensure_modules_manage_permission(ctx).await?;
        if expected_revision < 0 {
            return Err(<FieldError as GraphQLError>::bad_user_input(
                "Static module lifecycle revision must not be negative",
            ));
        }
        let expected_revision = u64::try_from(expected_revision).map_err(|_| {
            <FieldError as GraphQLError>::bad_user_input(
                "Static module lifecycle revision is outside the supported range",
            )
        })?;
        let db = ctx.data::<DatabaseConnection>()?;
        let registry = ctx.data::<ModuleRegistry>()?;

        let plan = ModuleLifecycleService::retry_failed_post_hook_operation(
            db,
            registry,
            tenant.id,
            operation_id,
            module_command_context(auth.user_id, Some(tenant.id), idempotency_key),
            expected_revision,
        )
        .await
        .map_err(map_module_operation_recovery_error)?;

        Ok(ModuleOperationRecoveryPlan::from(&plan))
    }

    async fn compensate_failed_module_operation(
        &self,
        ctx: &Context<'_>,
        operation_id: Uuid,
        idempotency_key: Uuid,
        expected_revision: i64,
    ) -> Result<TenantModule> {
        let (auth, tenant) = ensure_modules_manage_permission(ctx).await?;
        if expected_revision < 0 {
            return Err(<FieldError as GraphQLError>::bad_user_input(
                "Static module lifecycle revision must not be negative",
            ));
        }
        let expected_revision = u64::try_from(expected_revision).map_err(|_| {
            <FieldError as GraphQLError>::bad_user_input(
                "Static module lifecycle revision is outside the supported range",
            )
        })?;
        let db = ctx.data::<DatabaseConnection>()?;
        let registry = ctx.data::<ModuleRegistry>()?;

        let module = ModuleLifecycleService::compensate_failed_operation(
            db,
            registry,
            tenant.id,
            operation_id,
            module_command_context(auth.user_id, Some(tenant.id), idempotency_key),
            expected_revision,
        )
        .await
        .map_err(map_module_operation_recovery_error)?;

        Ok(TenantModule {
            module_slug: module.module_slug,
            enabled: module.enabled,
            settings: module.settings.to_string(),
            revision: i64::try_from(module.revision).map_err(|_| {
                <FieldError as GraphQLError>::internal_error(
                    "Static module lifecycle revision is outside the GraphQL range",
                )
            })?,
        })
    }

    async fn update_module_settings(
        &self,
        ctx: &Context<'_>,
        module_slug: String,
        settings: String,
        expected_revision: i64,
        idempotency_key: Uuid,
    ) -> Result<TenantModule> {
        let (auth, tenant) = ensure_modules_manage_permission(ctx).await?;
        if idempotency_key.is_nil() {
            return Err(<FieldError as GraphQLError>::bad_user_input(
                "Module settings idempotency key must not be nil",
            ));
        }
        if expected_revision < 0 {
            return Err(<FieldError as GraphQLError>::bad_user_input(
                "Static module lifecycle revision must not be negative",
            ));
        }
        let expected_revision = u64::try_from(expected_revision).map_err(|_| {
            <FieldError as GraphQLError>::bad_user_input(
                "Static module lifecycle revision is outside the supported range",
            )
        })?;
        let db = ctx.data::<DatabaseConnection>()?;
        let registry = ctx.data::<ModuleRegistry>()?;

        let settings_json: serde_json::Value = serde_json::from_str(&settings)
            .map_err(|err| FieldError::new(format!("Invalid JSON in settings: {err}")))?;

        let module = ModuleLifecycleService::update_module_settings(
            db,
            registry,
            tenant.id,
            &module_slug,
            settings_json,
            module_command_context(auth.user_id, Some(tenant.id), idempotency_key),
            expected_revision,
        )
        .await
        .map_err(|err| match err {
            UpdateModuleSettingsError::UnknownModule => FieldError::new("Unknown module"),
            UpdateModuleSettingsError::ModuleNotEnabled(_) => {
                <FieldError as GraphQLError>::bad_user_input(
                    "Module is not enabled for this tenant",
                )
            }
            UpdateModuleSettingsError::InvalidSettings => {
                <FieldError as GraphQLError>::bad_user_input(
                    "Module settings must be a JSON object",
                )
            }
            UpdateModuleSettingsError::Validation(message) => {
                <FieldError as GraphQLError>::bad_user_input(&message)
            }
            UpdateModuleSettingsError::IdempotencyConflict => FieldError::new(
                "Module settings idempotency key was reused for a different command",
            )
            .extend_with(|_, extensions| {
                extensions.set("code", "IDEMPOTENCY_CONFLICT");
                extensions.set("retryable_issue", false);
            }),
            UpdateModuleSettingsError::SettingsSnapshotConflict => FieldError::new(
                "Module settings changed since the reviewed snapshot",
            )
            .extend_with(|_, extensions| {
                extensions.set("code", "MODULE_SETTINGS_SNAPSHOT_CONFLICT");
                extensions.set("retryable_issue", false);
                extensions.set("requires_rereview", true);
            }),
            UpdateModuleSettingsError::ModuleNotEnabledRecorded => {
                <FieldError as GraphQLError>::bad_user_input(
                    "Module is not enabled for this tenant",
                )
            }
            UpdateModuleSettingsError::RevisionConflict { expected, current } => {
                FieldError::new(format!(
                    "Module lifecycle revision conflict: expected {expected}, current {current}"
                ))
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
                })
            }
            UpdateModuleSettingsError::RevisionConflictRecorded => FieldError::new(
                "Module lifecycle revision changed; reload the current module state",
            )
            .extend_with(|_, extensions| {
                extensions.set("code", "REVISION_CONFLICT");
                extensions.set("retryable_issue", false);
                extensions.set("requires_reload", true);
            }),
            UpdateModuleSettingsError::OperationInProgress => FieldError::new(
                "Module lifecycle operation is already active",
            )
            .extend_with(|_, extensions| {
                extensions.set("code", "MODULE_LIFECYCLE_OPERATION_IN_PROGRESS");
            }),
            UpdateModuleSettingsError::Manifest(err) => map_manifest_error(err),
            UpdateModuleSettingsError::Policy(err) => {
                <FieldError as GraphQLError>::internal_error(&err)
            }
            UpdateModuleSettingsError::Database(err) => {
                <FieldError as GraphQLError>::internal_error(&err.to_string())
            }
        })?;

        Ok(TenantModule {
            module_slug: module.module_slug,
            enabled: module.enabled,
            settings: module.settings.to_string(),
            revision: i64::try_from(module.revision).map_err(|_| {
                <FieldError as GraphQLError>::internal_error(
                    "Static module lifecycle revision is outside the GraphQL range",
                )
            })?,
        })
    }

    /// Trigger an emergency or operator-directed single-attempt rollback to the direct predecessor.
    async fn trigger_module_recovery(
        &self,
        ctx: &Context<'_>,
        operation_id: Uuid,
        reason: String,
    ) -> Result<crate::graphql::transition_lifecycle::ModuleTransitionCheckpointGql> {
        let db = ctx.data::<DatabaseConnection>()?;
        let checkpoint =
            rustok_modules::TransitionCheckpointStore::load_checkpoint(db, operation_id)
                .await
                .map_err(crate::graphql::transition_lifecycle::map_transition_store_error)?
                .ok_or_else(|| {
                    crate::graphql::transition_lifecycle::map_transition_store_error(
                        rustok_modules::TransitionStoreError::CheckpointNotFound(operation_id),
                    )
                })?;

        let mut coordinator = rustok_modules::ModuleTransitionCoordinator::new(checkpoint);
        coordinator
            .record_recovery_trigger(reason)
            .map_err(crate::graphql::transition_lifecycle::map_transition_coordinator_error)?;

        rustok_modules::TransitionCheckpointStore::save_checkpoint(db, coordinator.checkpoint())
            .await
            .map_err(crate::graphql::transition_lifecycle::map_transition_store_error)?;

        Ok(coordinator.checkpoint().clone().into())
    }

    /// Finalize a converged module release transition, closing the rollback window.
    async fn finalize_module_transition(
        &self,
        ctx: &Context<'_>,
        operation_id: Uuid,
    ) -> Result<crate::graphql::transition_lifecycle::ModuleTransitionCheckpointGql> {
        let db = ctx.data::<DatabaseConnection>()?;
        let checkpoint =
            rustok_modules::TransitionCheckpointStore::load_checkpoint(db, operation_id)
                .await
                .map_err(crate::graphql::transition_lifecycle::map_transition_store_error)?
                .ok_or_else(|| {
                    crate::graphql::transition_lifecycle::map_transition_store_error(
                        rustok_modules::TransitionStoreError::CheckpointNotFound(operation_id),
                    )
                })?;

        let security_registry = rustok_modules::SecurityEpochRegistry::new();
        let mut coordinator = rustok_modules::ModuleTransitionCoordinator::new(checkpoint);
        coordinator
            .finalize_convergence(&security_registry)
            .map_err(crate::graphql::transition_lifecycle::map_transition_coordinator_error)?;

        rustok_modules::TransitionCheckpointStore::save_checkpoint(db, coordinator.checkpoint())
            .await
            .map_err(crate::graphql::transition_lifecycle::map_transition_store_error)?;

        Ok(coordinator.checkpoint().clone().into())
    }
}

#[cfg(test)]
mod tests {
    use super::{
        AuthLifecycleError, ManifestError, ModuleCompositionError, PlatformCompositionBuildError,
        PlatformCompositionError, TOGGLE_ERR_UNKNOWN_MODULE, ToggleModuleError,
        map_create_user_error, map_manifest_error, map_platform_composition_build_error,
        map_platform_composition_error, map_toggle_module_error, prepare_user_custom_fields_write,
        require_platform_composition_operator, toggle_err_core_module_cannot_be_disabled,
        toggle_err_has_dependents, toggle_err_hook_failed, toggle_err_missing_dependencies,
        validate_custom_fields,
    };
    use crate::graphql::artifact_lifecycle::{
        map_artifact_installation_lifecycle_error, map_artifact_tenant_lifecycle_error,
    };
    use crate::models::user_field_definitions::ActiveModel as UserFieldDefinitionActiveModel;
    use async_graphql::ErrorExtensions;
    use rustok_api::{AuthContext, AuthPrincipalContext, AuthPrincipalKind, Permission};
    use rustok_core::UserRole;
    use rustok_migrations::SqliteTestMigrator as Migrator;
    use rustok_test_utils::db::setup_test_db_with_migrations;
    use sea_orm::{
        ActiveModelTrait, DatabaseConnection, Set, entity::prelude::DateTimeWithTimeZone,
    };
    use uuid::Uuid;

    fn platform_operator_auth(tenant_id: Uuid) -> AuthContext {
        AuthContext {
            user_id: Uuid::new_v4(),
            session_id: Uuid::new_v4(),
            tenant_id,
            permissions: vec![Permission::MODULES_MANAGE],
            client_id: None,
            scopes: Vec::new(),
            grant_type: "direct".to_string(),
        }
    }

    #[test]
    fn platform_composition_requires_a_direct_super_admin_with_modules_manage() {
        let tenant_id = Uuid::new_v4();
        let auth = platform_operator_auth(tenant_id);
        assert!(
            require_platform_composition_operator(
                &auth,
                AuthPrincipalContext::new(AuthPrincipalKind::DirectUser),
                tenant_id,
                UserRole::SuperAdmin,
                true,
            )
            .is_ok()
        );

        for (principal_kind, role, can_manage_modules) in [
            (AuthPrincipalKind::DelegatedUser, UserRole::SuperAdmin, true),
            (AuthPrincipalKind::Service, UserRole::SuperAdmin, true),
            (AuthPrincipalKind::DirectUser, UserRole::Admin, true),
            (AuthPrincipalKind::DirectUser, UserRole::SuperAdmin, false),
        ] {
            assert!(
                require_platform_composition_operator(
                    &auth,
                    AuthPrincipalContext::new(principal_kind),
                    tenant_id,
                    role,
                    can_manage_modules,
                )
                .is_err()
            );
        }
    }

    fn field_definition_model(
        tenant_id: Uuid,
        field_key: &str,
        field_type: &str,
        is_localized: bool,
        is_required: bool,
        default_value: Option<serde_json::Value>,
    ) -> UserFieldDefinitionActiveModel {
        let now: DateTimeWithTimeZone = chrono::Utc::now().into();
        UserFieldDefinitionActiveModel {
            id: Set(Uuid::new_v4()),
            tenant_id: Set(tenant_id),
            field_key: Set(field_key.to_string()),
            field_type: Set(field_type.to_string()),
            label: Set(serde_json::json!({"en": field_key})),
            description: Set(None),
            is_localized: Set(is_localized),
            is_required: Set(is_required),
            default_value: Set(default_value),
            validation: Set(None),
            position: Set(0),
            is_active: Set(true),
            created_at: Set(now),
            updated_at: Set(now),
        }
    }

    async fn db_with_definitions(
        definitions: Vec<UserFieldDefinitionActiveModel>,
    ) -> DatabaseConnection {
        let db = setup_test_db_with_migrations::<Migrator>().await;
        for definition in definitions {
            definition
                .insert(&db)
                .await
                .expect("failed to insert user field definition");
        }
        db
    }

    #[test]
    fn create_user_maps_email_exists() {
        let err = map_create_user_error(AuthLifecycleError::EmailAlreadyExists);
        assert!(err.message.contains("already exists"));
    }

    #[test]
    fn create_user_maps_internal_error() {
        let err = map_create_user_error(AuthLifecycleError::Internal(
            crate::error::Error::InternalServerError,
        ));
        assert!(!err.message.is_empty());
    }

    #[test]
    fn toggle_error_maps_database_and_policy_to_internal_errors() {
        let db_err = map_toggle_module_error(ToggleModuleError::Database(sea_orm::DbErr::Custom(
            "db down".to_string(),
        )));
        assert!(!db_err.message.is_empty());

        let policy_err = map_toggle_module_error(ToggleModuleError::Policy("policy".to_string()));
        assert!(!policy_err.message.is_empty());
    }

    #[test]
    fn artifact_tenant_lifecycle_conflicts_are_sanitized_and_non_retryable() {
        let mapped = map_artifact_tenant_lifecycle_error(
            rustok_modules::ModuleInstallationError::AdmissionRevisionConflict(
                "tenant lifecycle revision is stale".to_string(),
            ),
        );
        assert_eq!(
            error_code(&mapped),
            Some("ARTIFACT_TENANT_LIFECYCLE_CONFLICT".to_string())
        );
        assert_eq!(extension_bool(&mapped, "retryable_issue"), Some(false));
        assert!(!mapped.message.contains("stale"));
    }

    #[test]
    fn artifact_installation_lifecycle_conflicts_are_sanitized_and_non_retryable() {
        let mapped = map_artifact_installation_lifecycle_error(
            rustok_modules::ModuleInstallationError::AdmissionRevisionConflict(
                "artifact installation revision is stale".to_string(),
            ),
        );
        assert_eq!(
            error_code(&mapped),
            Some("ARTIFACT_INSTALLATION_LIFECYCLE_CONFLICT".to_string())
        );
        assert_eq!(extension_bool(&mapped, "retryable_issue"), Some(false));
        assert!(!mapped.message.contains("stale"));
    }

    fn error_code(error: &async_graphql::Error) -> Option<String> {
        error
            .extensions
            .as_ref()
            .and_then(|extensions| extensions.get("code"))
            .cloned()
            .and_then(|value| value.into_json().ok())
            .and_then(|value| value.as_str().map(ToOwned::to_owned))
    }

    fn extension_string(error: &async_graphql::Error, key: &str) -> Option<String> {
        error
            .extensions
            .as_ref()
            .and_then(|extensions| extensions.get(key))
            .cloned()
            .and_then(|value| value.into_json().ok())
            .and_then(|value| value.as_str().map(ToOwned::to_owned))
    }

    fn extension_bool(error: &async_graphql::Error, key: &str) -> Option<bool> {
        error
            .extensions
            .as_ref()
            .and_then(|extensions| extensions.get(key))
            .cloned()
            .and_then(|value| value.into_json().ok())
            .and_then(|value| value.as_bool())
    }

    struct ToggleCase {
        error: ToggleModuleError,
        expected_message: String,
        expected_code: Option<&'static str>,
        case_name: &'static str,
    }

    fn toggle_error_contract_cases() -> Vec<ToggleCase> {
        vec![
            ToggleCase {
                error: ToggleModuleError::InvalidCommandIdentity,
                expected_message: "Module lifecycle command identity is invalid".to_string(),
                expected_code: Some("BAD_USER_INPUT"),
                case_name: "invalid-command-identity",
            },
            ToggleCase {
                error: ToggleModuleError::InvalidIdempotencyKey,
                expected_message: "Module lifecycle idempotency key is invalid".to_string(),
                expected_code: Some("BAD_USER_INPUT"),
                case_name: "invalid-idempotency-key",
            },
            ToggleCase {
                error: ToggleModuleError::IdempotencyConflict,
                expected_message:
                    "Module lifecycle idempotency key was reused for a different command"
                        .to_string(),
                expected_code: Some("IDEMPOTENCY_CONFLICT"),
                case_name: "idempotency-conflict",
            },
            ToggleCase {
                error: ToggleModuleError::UnknownModule,
                expected_message: TOGGLE_ERR_UNKNOWN_MODULE.to_string(),
                expected_code: Some("BAD_USER_INPUT"),
                case_name: "unknown-module",
            },
            ToggleCase {
                error: ToggleModuleError::CoreModuleCannotBeDisabled("core".into()),
                expected_message: toggle_err_core_module_cannot_be_disabled("core"),
                expected_code: Some("BAD_USER_INPUT"),
                case_name: "core-disable",
            },
            ToggleCase {
                error: ToggleModuleError::MissingDependencies("pricing".into()),
                expected_message: toggle_err_missing_dependencies("pricing"),
                expected_code: Some("BAD_USER_INPUT"),
                case_name: "missing-dependencies",
            },
            ToggleCase {
                error: ToggleModuleError::HasDependents("checkout".into()),
                expected_message: toggle_err_has_dependents("checkout"),
                expected_code: Some("BAD_USER_INPUT"),
                case_name: "has-dependents",
            },
            ToggleCase {
                error: ToggleModuleError::PreHookFailed("boom".into()),
                expected_message: toggle_err_hook_failed("boom"),
                expected_code: Some("MODULE_HOOK_FAILED"),
                case_name: "pre-hook-failed",
            },
            ToggleCase {
                error: ToggleModuleError::PostHookFailed("downstream timeout".into()),
                expected_message: toggle_err_hook_failed("downstream timeout"),
                expected_code: Some("MODULE_HOOK_FAILED"),
                case_name: "post-hook-failed",
            },
            ToggleCase {
                error: ToggleModuleError::Database(sea_orm::DbErr::Custom("db down".to_string())),
                expected_message: "Internal server error".to_string(),
                expected_code: Some("INTERNAL_ERROR"),
                case_name: "database",
            },
            ToggleCase {
                error: ToggleModuleError::Policy("policy".to_string()),
                expected_message: "Internal server error".to_string(),
                expected_code: Some("INTERNAL_ERROR"),
                case_name: "policy",
            },
        ]
    }

    fn toggle_user_input_error_cases() -> Vec<ToggleCase> {
        toggle_error_contract_cases()
            .into_iter()
            .filter(|case| case.expected_code == Some("BAD_USER_INPUT"))
            .collect()
    }

    fn toggle_internal_error_cases() -> Vec<ToggleCase> {
        toggle_error_contract_cases()
            .into_iter()
            .filter(|case| case.expected_code == Some("INTERNAL_ERROR"))
            .collect()
    }

    fn toggle_hook_failed_cases() -> Vec<ToggleCase> {
        toggle_error_contract_cases()
            .into_iter()
            .filter(|case| case.expected_code == Some("MODULE_HOOK_FAILED"))
            .collect()
    }

    fn toggle_idempotency_conflict_cases() -> Vec<ToggleCase> {
        toggle_error_contract_cases()
            .into_iter()
            .filter(|case| case.expected_code == Some("IDEMPOTENCY_CONFLICT"))
            .collect()
    }

    #[test]
    fn toggle_error_taxonomy_partitions_are_disjoint_and_complete() {
        let all = toggle_error_contract_cases();
        let user = toggle_user_input_error_cases();
        let internal = toggle_internal_error_cases();
        let hook = toggle_hook_failed_cases();
        let idempotency_conflict = toggle_idempotency_conflict_cases();

        assert_eq!(
            all.len(),
            user.len() + internal.len() + hook.len() + idempotency_conflict.len(),
            "toggle taxonomy partition drifted: categories must cover all cases exactly"
        );

        for user_case in &user {
            assert!(
                internal
                    .iter()
                    .all(|case| case.case_name != user_case.case_name),
                "toggle taxonomy partition overlap detected for case: {}",
                user_case.case_name
            );
        }

        assert!(
            all.iter().all(|case| {
                case.expected_code == Some("BAD_USER_INPUT")
                    || case.expected_code == Some("MODULE_HOOK_FAILED")
                    || case.expected_code == Some("INTERNAL_ERROR")
                    || case.expected_code == Some("IDEMPOTENCY_CONFLICT")
            }),
            "toggle taxonomy contains unsupported error code category"
        );

        let mut seen_case_names = std::collections::BTreeSet::new();
        for case in &all {
            assert!(
                seen_case_names.insert(case.case_name),
                "toggle taxonomy contains duplicated case_name: {}",
                case.case_name
            );
        }
    }

    #[test]
    fn toggle_error_mapping_sets_expected_error_codes() {
        for case in toggle_error_contract_cases() {
            let gql = map_toggle_module_error(case.error).extend();
            assert_eq!(
                error_code(&gql).as_deref(),
                case.expected_code,
                "toggle error code drifted for case: {}",
                case.case_name
            );
        }
    }

    #[test]
    fn toggle_user_input_taxonomy_maps_only_to_bad_user_input_code() {
        for case in toggle_user_input_error_cases() {
            let gql = map_toggle_module_error(case.error).extend();
            assert_eq!(
                error_code(&gql).as_deref(),
                Some("BAD_USER_INPUT"),
                "toggle user-input taxonomy must map to BAD_USER_INPUT code for case: {}",
                case.case_name
            );
        }
    }

    #[test]
    fn toggle_hook_failed_taxonomy_maps_only_to_module_hook_failed_code() {
        for case in toggle_hook_failed_cases() {
            let gql = map_toggle_module_error(case.error).extend();
            assert_eq!(
                error_code(&gql).as_deref(),
                Some("MODULE_HOOK_FAILED"),
                "toggle hook-failed taxonomy must map to MODULE_HOOK_FAILED code for case: {}",
                case.case_name
            );
        }
    }

    #[test]
    fn toggle_idempotency_conflicts_are_non_retryable() {
        for case in toggle_idempotency_conflict_cases() {
            let gql = map_toggle_module_error(case.error).extend();
            assert_eq!(
                error_code(&gql).as_deref(),
                Some("IDEMPOTENCY_CONFLICT"),
                "toggle idempotency conflict code drifted for case: {}",
                case.case_name
            );
            assert_eq!(extension_bool(&gql, "retryable_issue"), Some(false));
        }
    }

    #[test]
    fn toggle_internal_error_taxonomy_maps_only_to_internal_error_code() {
        for case in toggle_internal_error_cases() {
            let gql = map_toggle_module_error(case.error).extend();
            assert_eq!(
                error_code(&gql).as_deref(),
                Some("INTERNAL_ERROR"),
                "toggle internal taxonomy must map to INTERNAL_ERROR code for case: {}",
                case.case_name
            );
        }
    }

    #[test]
    fn toggle_internal_error_taxonomy_uses_generic_internal_message() {
        for case in toggle_internal_error_cases() {
            let mapped = map_toggle_module_error(case.error);
            assert_eq!(
                mapped.message, "Internal server error",
                "toggle internal taxonomy must not leak implementation details for case: {}",
                case.case_name
            );
        }
    }

    #[test]
    fn toggle_error_mapping_matrix_preserves_message_and_code_contract() {
        for case in toggle_error_contract_cases() {
            let mapped = map_toggle_module_error(case.error);
            assert_eq!(
                mapped.message, case.expected_message,
                "toggle message drifted for case: {}",
                case.case_name
            );
            let gql = mapped.extend();
            assert_eq!(
                error_code(&gql).as_deref(),
                case.expected_code,
                "toggle error code drifted for case: {}",
                case.case_name
            );
            assert!(
                !mapped.message.contains("rolled back"),
                "toggle error contract must not reference partial rollback semantics for case: {}",
                case.case_name
            );
        }
    }

    #[test]
    fn toggle_hook_failed_pre_hook_sets_non_retryable_issue_extensions() {
        let mapped = map_toggle_module_error(ToggleModuleError::PreHookFailed("boom".to_string()));
        let gql = mapped.extend();

        assert_eq!(error_code(&gql).as_deref(), Some("MODULE_HOOK_FAILED"));
        assert_eq!(extension_bool(&gql, "retryable_issue"), Some(false));
        assert_eq!(
            extension_string(&gql, "operation_issue").as_deref(),
            Some("pre_hook_failed")
        );
    }

    #[test]
    fn toggle_hook_failed_post_hook_sets_retryable_issue_extensions() {
        let mapped = map_toggle_module_error(ToggleModuleError::PostHookFailed(
            "downstream timeout".to_string(),
        ));
        let gql = mapped.extend();

        assert_eq!(error_code(&gql).as_deref(), Some("MODULE_HOOK_FAILED"));
        assert_eq!(extension_bool(&gql, "retryable_issue"), Some(true));
        assert_eq!(
            extension_string(&gql, "operation_issue").as_deref(),
            Some("post_hook_failed")
        );
    }

    #[test]
    fn toggle_error_taxonomy_matrix_stays_stable() {
        for case in toggle_user_input_error_cases() {
            let field_error = map_toggle_module_error(case.error);
            assert_eq!(
                field_error.message, case.expected_message,
                "toggle error taxonomy drifted for case: {}",
                case.case_name
            );
            assert!(
                !field_error.message.contains("rolled back"),
                "toggle error taxonomy unexpectedly references partial rollback for case: {}",
                case.case_name
            );
        }
    }

    #[test]
    fn manifest_error_maps_validation_errors_to_user_messages() {
        let err = map_manifest_error(ManifestError::RequiredModule("pages".to_string()));
        assert!(err.message.contains("required"));
    }

    #[test]
    fn platform_composition_error_maps_revision_conflict_with_expected_and_current() {
        let err = map_platform_composition_error(PlatformCompositionError::RevisionConflict {
            expected: 3,
            current: 5,
        });
        assert_eq!(
            err.message,
            "Platform composition revision conflict: expected 3, current 5"
        );
        let gql = err.extend();
        assert_eq!(
            error_code(&gql).as_deref(),
            Some("BAD_USER_INPUT"),
            "revision conflict must stay in user-facing conflict taxonomy"
        );
    }

    #[test]
    fn platform_composition_error_matrix_preserves_taxonomy_for_internal_and_user_paths() {
        struct Case {
            name: &'static str,
            error: PlatformCompositionError,
            expected_code: &'static str,
            message_fragment: &'static str,
        }

        let cases = vec![
            Case {
                name: "revision conflict",
                error: PlatformCompositionError::RevisionConflict {
                    expected: 7,
                    current: 9,
                },
                expected_code: "BAD_USER_INPUT",
                message_fragment: "revision conflict",
            },
            Case {
                name: "serialize failure",
                error: PlatformCompositionError::Serialize("serde exploded".to_string()),
                expected_code: "INTERNAL_ERROR",
                message_fragment: "serde exploded",
            },
            Case {
                name: "deserialize failure",
                error: PlatformCompositionError::Deserialize("bad snapshot".to_string()),
                expected_code: "INTERNAL_ERROR",
                message_fragment: "bad snapshot",
            },
            Case {
                name: "database failure",
                error: PlatformCompositionError::Database(sea_orm::DbErr::Custom(
                    "db is unavailable".to_string(),
                )),
                expected_code: "INTERNAL_ERROR",
                message_fragment: "db is unavailable",
            },
            Case {
                name: "manifest validation direct mapping",
                error: PlatformCompositionError::Manifest(ManifestError::RequiredModule(
                    "pages".to_string(),
                )),
                expected_code: "BAD_USER_INPUT",
                message_fragment: "required",
            },
        ];

        for case in cases {
            let mapped = map_platform_composition_error(case.error);
            assert!(
                mapped
                    .message
                    .to_lowercase()
                    .contains(case.message_fragment),
                "message contract drifted for case `{}`",
                case.name
            );
            let gql = mapped.extend();
            assert_eq!(
                error_code(&gql).as_deref(),
                Some(case.expected_code),
                "error code contract drifted for case `{}`",
                case.name
            );
            assert!(
                !mapped.message.to_lowercase().contains("rolled back"),
                "error message must not reintroduce partial rollback wording for case `{}`",
                case.name
            );
        }
    }

    #[test]
    fn platform_composition_build_error_matrix_preserves_message_and_code_contract() {
        struct Case {
            name: &'static str,
            error: PlatformCompositionBuildError,
            expected_code: &'static str,
            expected_message_fragment: &'static str,
            exact_message: Option<&'static str>,
        }

        let cases = vec![
            Case {
                name: "build enqueue failure",
                error: PlatformCompositionBuildError::Build("enqueue failed".to_string()),
                expected_code: "INTERNAL_ERROR",
                expected_message_fragment: "enqueue failed",
                exact_message: None,
            },
            Case {
                name: "manifest validation failure",
                error: PlatformCompositionBuildError::Composition(
                    PlatformCompositionError::Manifest(ManifestError::RequiredModule(
                        "pages".to_string(),
                    )),
                ),
                expected_code: "BAD_USER_INPUT",
                expected_message_fragment: "required",
                exact_message: None,
            },
            Case {
                name: "serialize failure via composition wrapper",
                error: PlatformCompositionBuildError::Composition(
                    PlatformCompositionError::Serialize("serde exploded".to_string()),
                ),
                expected_code: "INTERNAL_ERROR",
                expected_message_fragment: "serde exploded",
                exact_message: None,
            },
            Case {
                name: "deserialize failure via composition wrapper",
                error: PlatformCompositionBuildError::Composition(
                    PlatformCompositionError::Deserialize("bad snapshot".to_string()),
                ),
                expected_code: "INTERNAL_ERROR",
                expected_message_fragment: "bad snapshot",
                exact_message: None,
            },
            Case {
                name: "database failure via composition wrapper",
                error: PlatformCompositionBuildError::Composition(
                    PlatformCompositionError::Database(sea_orm::DbErr::Custom(
                        "db is unavailable".to_string(),
                    )),
                ),
                expected_code: "INTERNAL_ERROR",
                expected_message_fragment: "db is unavailable",
                exact_message: None,
            },
            Case {
                name: "build path revision conflict",
                error: PlatformCompositionBuildError::Composition(
                    PlatformCompositionError::RevisionConflict {
                        expected: 11,
                        current: 13,
                    },
                ),
                expected_code: "BAD_USER_INPUT",
                expected_message_fragment: "revision conflict",
                exact_message: Some(
                    "Platform composition revision conflict: expected 11, current 13",
                ),
            },
            Case {
                name: "owner rejects a non-positive expected revision",
                error: PlatformCompositionBuildError::Composition(PlatformCompositionError::Owner(
                    ModuleCompositionError::InvalidExpectedRevision,
                )),
                expected_code: "BAD_USER_INPUT",
                expected_message_fragment: "positive expected revision",
                exact_message: None,
            },
            Case {
                name: "terminal owner validation receipt",
                error: PlatformCompositionBuildError::Composition(PlatformCompositionError::Owner(
                    ModuleCompositionError::OperationReceipt(rustok_api::PortError::validation(
                        "modules.composition_invalid_mutation",
                        "module selection is invalid",
                    )),
                )),
                expected_code: "BAD_USER_INPUT",
                expected_message_fragment: "module selection is invalid",
                exact_message: None,
            },
        ];

        for case in cases {
            let mapped = map_platform_composition_build_error(case.error);
            assert!(
                mapped.message.contains(case.expected_message_fragment),
                "message fragment drifted for case `{}`",
                case.name
            );
            if let Some(exact) = case.exact_message {
                assert_eq!(
                    mapped.message, exact,
                    "exact message drifted for case `{}`",
                    case.name
                );
            }
            let gql = mapped.extend();
            assert_eq!(
                error_code(&gql).as_deref(),
                Some(case.expected_code),
                "error code drifted for case `{}`",
                case.name
            );
        }
    }

    #[test]
    fn platform_composition_build_error_mapping_never_mentions_partial_rollback() {
        let errors = vec![
            PlatformCompositionBuildError::Build("enqueue failed".to_string()),
            PlatformCompositionBuildError::Composition(PlatformCompositionError::Manifest(
                ManifestError::RequiredModule("pages".to_string()),
            )),
            PlatformCompositionBuildError::Composition(PlatformCompositionError::Serialize(
                "serde exploded".to_string(),
            )),
            PlatformCompositionBuildError::Composition(PlatformCompositionError::Deserialize(
                "bad snapshot".to_string(),
            )),
            PlatformCompositionBuildError::Composition(PlatformCompositionError::Database(
                sea_orm::DbErr::Custom("db is unavailable".to_string()),
            )),
            PlatformCompositionBuildError::Composition(
                PlatformCompositionError::RevisionConflict {
                    expected: 1,
                    current: 2,
                },
            ),
        ];

        for error in errors {
            let mapped = map_platform_composition_build_error(error);
            assert!(
                !mapped.message.to_lowercase().contains("rolled back"),
                "platform composition build error contract must not mention partial rollback semantics"
            );
        }
    }

    #[test]
    fn platform_composition_build_wrapper_preserves_composition_mapping_contract() {
        let composition_error_pairs = vec![
            (
                PlatformCompositionError::RevisionConflict {
                    expected: 5,
                    current: 8,
                },
                PlatformCompositionError::RevisionConflict {
                    expected: 5,
                    current: 8,
                },
            ),
            (
                PlatformCompositionError::Manifest(ManifestError::RequiredModule(
                    "pages".to_string(),
                )),
                PlatformCompositionError::Manifest(ManifestError::RequiredModule(
                    "pages".to_string(),
                )),
            ),
            (
                PlatformCompositionError::Serialize("serde exploded".to_string()),
                PlatformCompositionError::Serialize("serde exploded".to_string()),
            ),
            (
                PlatformCompositionError::Deserialize("bad snapshot".to_string()),
                PlatformCompositionError::Deserialize("bad snapshot".to_string()),
            ),
            (
                PlatformCompositionError::Database(sea_orm::DbErr::Custom(
                    "db is unavailable".to_string(),
                )),
                PlatformCompositionError::Database(sea_orm::DbErr::Custom(
                    "db is unavailable".to_string(),
                )),
            ),
        ];

        for (direct_error, wrapped_error) in composition_error_pairs {
            let direct = map_platform_composition_error(direct_error);
            let wrapped = map_platform_composition_build_error(
                PlatformCompositionBuildError::Composition(wrapped_error),
            );

            assert_eq!(
                wrapped.message, direct.message,
                "build-wrapper mapping drifted from direct composition mapping"
            );

            let direct_gql = direct.extend();
            let wrapped_gql = wrapped.extend();
            assert_eq!(
                error_code(&wrapped_gql),
                error_code(&direct_gql),
                "build-wrapper error code drifted from direct composition mapping"
            );
        }
    }

    #[tokio::test]
    async fn validate_custom_fields_applies_defaults() {
        let tenant_id = Uuid::new_v4();
        let db = db_with_definitions(vec![field_definition_model(
            tenant_id,
            "department",
            "text",
            false,
            false,
            Some(serde_json::json!("sales")),
        )])
        .await;

        let result = validate_custom_fields(&db, tenant_id, Some(serde_json::json!({})))
            .await
            .expect("defaults should be applied");

        assert_eq!(result, Some(serde_json::json!({"department": "sales"})));
    }

    #[tokio::test]
    async fn validate_custom_fields_strips_unknown_keys() {
        let tenant_id = Uuid::new_v4();
        let db = db_with_definitions(vec![field_definition_model(
            tenant_id,
            "department",
            "text",
            false,
            false,
            None,
        )])
        .await;

        let result = validate_custom_fields(
            &db,
            tenant_id,
            Some(serde_json::json!({"department": "sales", "unknown": "drop"})),
        )
        .await
        .expect("unknown keys should be stripped");

        assert_eq!(result, Some(serde_json::json!({"department": "sales"})));
    }

    #[tokio::test]
    async fn validate_custom_fields_returns_input_when_schema_empty() {
        let tenant_id = Uuid::new_v4();
        let db = db_with_definitions(Vec::<UserFieldDefinitionActiveModel>::new()).await;
        let payload = Some(serde_json::json!({"nickname": "neo"}));

        let result = validate_custom_fields(&db, tenant_id, payload.clone())
            .await
            .expect("without schema payload should pass through");

        assert_eq!(result, payload);
    }

    #[tokio::test]
    async fn validate_custom_fields_error_contains_field_details() {
        let tenant_id = Uuid::new_v4();
        let db = db_with_definitions(vec![field_definition_model(
            tenant_id, "phone", "text", false, true, None,
        )])
        .await;

        let err = validate_custom_fields(&db, tenant_id, Some(serde_json::json!({})))
            .await
            .expect_err("missing required field must fail");

        let fields = err
            .extensions
            .as_ref()
            .and_then(|extensions| extensions.get("fields"))
            .cloned()
            .and_then(|value| value.into_json().ok())
            .and_then(|value| value.as_array().cloned())
            .unwrap_or_default();
        assert!(!fields.is_empty());
        let first_field = &fields[0];
        let key = first_field
            .get("field_key")
            .and_then(|v| v.as_str())
            .unwrap_or_default();
        let code = first_field
            .get("error_code")
            .and_then(|v| v.as_str())
            .unwrap_or_default();
        assert_eq!(key, "phone");
        assert_eq!(code, "required");
    }
    #[tokio::test]
    async fn validate_custom_fields_applies_defaults_when_input_is_none() {
        let tenant_id = Uuid::new_v4();
        let db = db_with_definitions(vec![field_definition_model(
            tenant_id,
            "department",
            "text",
            false,
            false,
            Some(serde_json::json!("sales")),
        )])
        .await;

        let result = validate_custom_fields(&db, tenant_id, None)
            .await
            .expect("defaults should be applied for empty input");

        assert_eq!(result, Some(serde_json::json!({"department": "sales"})));
    }

    #[tokio::test]
    async fn validate_custom_fields_returns_graphql_error_for_required_field() {
        let tenant_id = Uuid::new_v4();
        let db = db_with_definitions(vec![field_definition_model(
            tenant_id, "phone", "text", false, true, None,
        )])
        .await;

        let err = validate_custom_fields(&db, tenant_id, Some(serde_json::json!({})))
            .await
            .expect_err("missing required field must fail");

        assert!(err.message.contains("Custom field validation failed"));
        let code = err
            .extensions
            .as_ref()
            .and_then(|extensions| extensions.get("code"))
            .cloned()
            .and_then(|value| value.into_json().ok())
            .and_then(|value| value.as_str().map(ToOwned::to_owned))
            .unwrap_or_default();
        assert_eq!(code, "CUSTOM_FIELD_VALIDATION_FAILED");
    }

    #[tokio::test]
    async fn prepare_user_custom_fields_write_splits_localized_values_from_metadata() {
        let tenant_id = Uuid::new_v4();
        let db = db_with_definitions(vec![
            field_definition_model(tenant_id, "nickname", "text", false, false, None),
            field_definition_model(tenant_id, "bio", "text", true, false, None),
        ])
        .await;

        let prepared = prepare_user_custom_fields_write(
            &db,
            tenant_id,
            "ru",
            None,
            None,
            Some(serde_json::json!({"nickname": "neo", "bio": "Привет"})),
        )
        .await
        .expect("custom fields should split successfully");

        assert_eq!(
            prepared.metadata,
            Some(serde_json::json!({"nickname": "neo"}))
        );
        assert_eq!(
            prepared.localized_values,
            Some(serde_json::json!({"bio": "Привет"}))
        );
        assert_eq!(prepared.locale.as_deref(), Some("ru"));
    }
}
