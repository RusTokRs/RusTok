use leptos::prelude::*;
use rustok_ui_transport::UiTransportPath;

#[allow(unused_imports)]
use crate::entities::module::model::{
    registry_principal_label_from_value, RegistryFollowUpGateLifecycle,
    RegistryGovernanceActionLifecycle, RegistryGovernanceEventLifecycle,
    RegistryGovernanceEventPayloadLifecycle, RegistryModuleLifecycle, RegistryOwnerLifecycle,
    RegistryPublishRequestLifecycle, RegistryReleaseLifecycle, RegistryValidationStageLifecycle,
};
use crate::entities::module::{
    BuildJob, InstalledModule, MarketplaceModule, ModuleInfo, ModuleOperationRecoveryPlan,
    ReleaseInfo, TenantModule, ToggleModuleResult,
};
use crate::shared::api::{map_server_fn_error, request, ApiError};

use super::native_server_adapter::*;
use super::types::*;

fn selected_transport_path() -> UiTransportPath {
    if cfg!(all(target_arch = "wasm32", not(feature = "hydrate"))) {
        UiTransportPath::Graphql
    } else {
        UiTransportPath::NativeServer
    }
}

pub async fn fetch_enabled_modules(
    token: Option<String>,
    tenant_slug: Option<String>,
) -> Result<Vec<String>, ApiError> {
    match selected_transport_path() {
        UiTransportPath::NativeServer => fetch_enabled_modules_server()
            .await
            .map_err(map_server_fn_error),
        UiTransportPath::Graphql => fetch_enabled_modules_graphql(token, tenant_slug).await,
    }
}

pub async fn fetch_enabled_modules_server() -> Result<Vec<String>, ServerFnError> {
    list_enabled_modules_native().await
}

pub async fn fetch_enabled_modules_graphql(
    token: Option<String>,
    tenant_slug: Option<String>,
) -> Result<Vec<String>, ApiError> {
    let response: EnabledModulesResponse = request(
        ENABLED_MODULES_QUERY,
        serde_json::json!({}),
        token,
        tenant_slug,
    )
    .await?;
    Ok(response.enabled_modules)
}

pub async fn fetch_modules(
    token: Option<String>,
    tenant_slug: Option<String>,
) -> Result<Vec<ModuleInfo>, ApiError> {
    match selected_transport_path() {
        UiTransportPath::NativeServer => list_module_registry_native()
            .await
            .map_err(map_server_fn_error),
        UiTransportPath::Graphql => {
            let response: ModuleRegistryResponse = request(
                MODULE_REGISTRY_QUERY,
                serde_json::json!({}),
                token,
                tenant_slug,
            )
            .await?;
            Ok(response.module_registry)
        }
    }
}

pub async fn fetch_installed_modules(
    token: Option<String>,
    tenant_slug: Option<String>,
) -> Result<Vec<InstalledModule>, ApiError> {
    match selected_transport_path() {
        UiTransportPath::NativeServer => list_installed_modules_native()
            .await
            .map_err(map_server_fn_error),
        UiTransportPath::Graphql => {
            let response: InstalledModulesResponse = request(
                INSTALLED_MODULES_QUERY,
                serde_json::json!({}),
                token,
                tenant_slug,
            )
            .await?;
            Ok(response.installed_modules)
        }
    }
}

pub async fn fetch_tenant_modules(
    token: Option<String>,
    tenant_slug: Option<String>,
) -> Result<Vec<TenantModule>, ApiError> {
    match selected_transport_path() {
        UiTransportPath::NativeServer => list_tenant_modules_native()
            .await
            .map_err(map_server_fn_error),
        UiTransportPath::Graphql => {
            let response: TenantModulesResponse = request(
                TENANT_MODULES_QUERY,
                serde_json::json!({}),
                token,
                tenant_slug,
            )
            .await?;
            Ok(response.tenant_modules)
        }
    }
}

pub async fn fetch_marketplace_modules(
    token: Option<String>,
    tenant_slug: Option<String>,
    variables: MarketplaceVariables,
) -> Result<Vec<MarketplaceModule>, ApiError> {
    if token.is_some() {
        let response: MarketplaceResponse =
            request(MARKETPLACE_QUERY, variables, token, tenant_slug).await?;
        return Ok(response.marketplace);
    }

    match selected_transport_path() {
        UiTransportPath::NativeServer => list_marketplace_modules_native(
            variables.search,
            variables.category,
            variables.tag,
            variables.source,
            variables.trust_level,
            variables.only_compatible,
            variables.installed_only,
        )
        .await
        .map_err(map_server_fn_error),
        UiTransportPath::Graphql => {
            let response: MarketplaceResponse =
                request(MARKETPLACE_QUERY, variables, token, tenant_slug).await?;
            Ok(response.marketplace)
        }
    }
}

pub async fn fetch_marketplace_module(
    slug: String,
    token: Option<String>,
    tenant_slug: Option<String>,
) -> Result<Option<MarketplaceModule>, ApiError> {
    if token.is_some() {
        let response: MarketplaceModuleResponse = request(
            MARKETPLACE_MODULE_QUERY,
            MarketplaceModuleVariables { slug },
            token,
            tenant_slug,
        )
        .await?;
        return Ok(response.marketplace_module);
    }

    match selected_transport_path() {
        UiTransportPath::NativeServer => marketplace_module_native(slug)
            .await
            .map_err(map_server_fn_error),
        UiTransportPath::Graphql => {
            let response: MarketplaceModuleResponse = request(
                MARKETPLACE_MODULE_QUERY,
                MarketplaceModuleVariables { slug },
                token,
                tenant_slug,
            )
            .await?;
            Ok(response.marketplace_module)
        }
    }
}

pub async fn fetch_active_build(
    token: Option<String>,
    tenant_slug: Option<String>,
) -> Result<Option<BuildJob>, ApiError> {
    match selected_transport_path() {
        UiTransportPath::NativeServer => active_build_native().await.map_err(map_server_fn_error),
        UiTransportPath::Graphql => {
            let response: ActiveBuildResponse = request(
                ACTIVE_BUILD_QUERY,
                serde_json::json!({}),
                token,
                tenant_slug,
            )
            .await?;
            Ok(response.active_build)
        }
    }
}

pub async fn fetch_active_release(
    token: Option<String>,
    tenant_slug: Option<String>,
) -> Result<Option<ReleaseInfo>, ApiError> {
    match selected_transport_path() {
        UiTransportPath::NativeServer => active_release_native().await.map_err(map_server_fn_error),
        UiTransportPath::Graphql => {
            let response: ActiveReleaseResponse = request(
                ACTIVE_RELEASE_QUERY,
                serde_json::json!({}),
                token,
                tenant_slug,
            )
            .await?;
            Ok(response.active_release)
        }
    }
}

pub async fn fetch_build_history(
    token: Option<String>,
    tenant_slug: Option<String>,
    limit: i32,
    offset: i32,
) -> Result<Vec<BuildJob>, ApiError> {
    match selected_transport_path() {
        UiTransportPath::NativeServer => build_history_native(limit, offset)
            .await
            .map_err(map_server_fn_error),
        UiTransportPath::Graphql => {
            let response: BuildHistoryResponse = request(
                BUILD_HISTORY_QUERY,
                BuildHistoryVariables { limit, offset },
                token,
                tenant_slug,
            )
            .await?;
            Ok(response.build_history)
        }
    }
}

pub async fn toggle_module(
    module_slug: String,
    enabled: bool,
    token: Option<String>,
    tenant_slug: Option<String>,
) -> Result<ToggleModuleResult, ApiError> {
    let response: ToggleModuleResponse = request(
        TOGGLE_MODULE_MUTATION,
        ToggleModuleVariables {
            module_slug,
            enabled,
        },
        token,
        tenant_slug,
    )
    .await?;
    Ok(response.toggle_module)
}

pub async fn module_operation_recovery_plan(
    operation_id: String,
    token: Option<String>,
    tenant_slug: Option<String>,
) -> Result<Option<ModuleOperationRecoveryPlan>, ApiError> {
    let response: ModuleOperationRecoveryPlanResponse = request(
        MODULE_OPERATION_RECOVERY_PLAN_QUERY,
        ModuleOperationRecoveryPlanVariables { operation_id },
        token,
        tenant_slug,
    )
    .await?;
    Ok(response.module_operation_recovery_plan)
}

pub async fn failed_module_operation_recovery_plans(
    module_slug: Option<String>,
    limit: Option<i32>,
    token: Option<String>,
    tenant_slug: Option<String>,
) -> Result<Vec<ModuleOperationRecoveryPlan>, ApiError> {
    let response: FailedModuleOperationRecoveryPlansResponse = request(
        FAILED_MODULE_OPERATION_RECOVERY_PLANS_QUERY,
        FailedModuleOperationRecoveryPlansVariables { module_slug, limit },
        token,
        tenant_slug,
    )
    .await?;
    Ok(response.failed_module_operation_recovery_plans)
}

pub async fn retry_failed_module_operation_post_hook(
    operation_id: String,
    token: Option<String>,
    tenant_slug: Option<String>,
) -> Result<ModuleOperationRecoveryPlan, ApiError> {
    let response: RetryFailedModuleOperationPostHookResponse = request(
        RETRY_FAILED_MODULE_OPERATION_POST_HOOK_MUTATION,
        ModuleOperationRecoveryPlanVariables { operation_id },
        token,
        tenant_slug,
    )
    .await?;
    Ok(response.retry_failed_module_operation_post_hook)
}

pub async fn compensate_failed_module_operation(
    operation_id: String,
    token: Option<String>,
    tenant_slug: Option<String>,
) -> Result<TenantModule, ApiError> {
    let response: CompensateFailedModuleOperationResponse = request(
        COMPENSATE_FAILED_MODULE_OPERATION_MUTATION,
        ModuleOperationRecoveryPlanVariables { operation_id },
        token,
        tenant_slug,
    )
    .await?;
    Ok(response.compensate_failed_module_operation)
}

pub async fn update_module_settings(
    module_slug: String,
    settings: String,
    token: Option<String>,
    tenant_slug: Option<String>,
) -> Result<TenantModule, ApiError> {
    match selected_transport_path() {
        UiTransportPath::NativeServer => update_module_settings_native(module_slug, settings)
            .await
            .map_err(map_server_fn_error),
        UiTransportPath::Graphql => {
            let response: UpdateModuleSettingsResponse = request(
                UPDATE_MODULE_SETTINGS_MUTATION,
                UpdateModuleSettingsVariables {
                    module_slug,
                    settings,
                },
                token,
                tenant_slug,
            )
            .await?;
            Ok(response.update_module_settings)
        }
    }
}

pub async fn install_module(
    slug: String,
    version: String,
    token: Option<String>,
    tenant_slug: Option<String>,
) -> Result<BuildJob, ApiError> {
    let response: InstallModuleResponse = request(
        INSTALL_MODULE_MUTATION,
        InstallModuleVariables { slug, version },
        token,
        tenant_slug,
    )
    .await?;
    Ok(response.install_module)
}

pub async fn uninstall_module(
    slug: String,
    token: Option<String>,
    tenant_slug: Option<String>,
) -> Result<BuildJob, ApiError> {
    let response: UninstallModuleResponse = request(
        UNINSTALL_MODULE_MUTATION,
        UninstallModuleVariables { slug },
        token,
        tenant_slug,
    )
    .await?;
    Ok(response.uninstall_module)
}

pub async fn upgrade_module(
    slug: String,
    version: String,
    token: Option<String>,
    tenant_slug: Option<String>,
) -> Result<BuildJob, ApiError> {
    let response: UpgradeModuleResponse = request(
        UPGRADE_MODULE_MUTATION,
        UpgradeModuleVariables { slug, version },
        token,
        tenant_slug,
    )
    .await?;
    Ok(response.upgrade_module)
}

pub async fn rollback_build(
    build_id: String,
    token: Option<String>,
    tenant_slug: Option<String>,
) -> Result<BuildJob, ApiError> {
    match selected_transport_path() {
        UiTransportPath::NativeServer => rollback_build_native(build_id)
            .await
            .map_err(map_server_fn_error),
        UiTransportPath::Graphql => {
            let response: RollbackBuildResponse = request(
                ROLLBACK_BUILD_MUTATION,
                RollbackBuildVariables { build_id },
                token,
                tenant_slug,
            )
            .await?;
            Ok(response.rollback_build)
        }
    }
}

pub async fn validate_registry_publish_request(
    request_id: String,
    dry_run: bool,
    token: Option<String>,
    tenant_slug: Option<String>,
) -> Result<RegistryMutationResult, ApiError> {
    let token = token.ok_or(ApiError::Unauthorized)?;
    validate_registry_publish_request_native(
        token,
        tenant_slug.unwrap_or_default(),
        request_id,
        dry_run,
    )
    .await
    .map_err(|error| ApiError::Graphql(error.to_string()))
}

pub async fn fetch_registry_publish_request_status(
    request_id: String,
    token: Option<String>,
    tenant_slug: Option<String>,
) -> Result<RegistryPublishStatusContract, ApiError> {
    let token = token.ok_or(ApiError::Unauthorized)?;
    fetch_registry_publish_request_status_native(token, tenant_slug.unwrap_or_default(), request_id)
        .await
        .map_err(|error| ApiError::Graphql(error.to_string()))
}

pub async fn approve_registry_publish_request(
    request_id: String,
    reason: Option<String>,
    reason_code: Option<String>,
    dry_run: bool,
    token: Option<String>,
    tenant_slug: Option<String>,
) -> Result<RegistryMutationResult, ApiError> {
    let token = token.ok_or(ApiError::Unauthorized)?;
    approve_registry_publish_request_native(
        token,
        tenant_slug.unwrap_or_default(),
        request_id,
        reason,
        reason_code,
        dry_run,
    )
    .await
    .map_err(|error| ApiError::Graphql(error.to_string()))
}

pub async fn reject_registry_publish_request(
    request_id: String,
    reason: String,
    reason_code: String,
    dry_run: bool,
    token: Option<String>,
    tenant_slug: Option<String>,
) -> Result<RegistryMutationResult, ApiError> {
    let token = token.ok_or(ApiError::Unauthorized)?;
    reject_registry_publish_request_native(
        token,
        tenant_slug.unwrap_or_default(),
        request_id,
        reason,
        reason_code,
        dry_run,
    )
    .await
    .map_err(|error| ApiError::Graphql(error.to_string()))
}

pub async fn request_changes_registry_publish_request(
    request_id: String,
    reason: String,
    reason_code: String,
    dry_run: bool,
    token: Option<String>,
    tenant_slug: Option<String>,
) -> Result<RegistryMutationResult, ApiError> {
    let token = token.ok_or(ApiError::Unauthorized)?;
    request_changes_registry_publish_request_native(
        token,
        tenant_slug.unwrap_or_default(),
        request_id,
        reason,
        reason_code,
        dry_run,
    )
    .await
    .map_err(|error| ApiError::Graphql(error.to_string()))
}

pub async fn hold_registry_publish_request(
    request_id: String,
    reason: String,
    reason_code: String,
    dry_run: bool,
    token: Option<String>,
    tenant_slug: Option<String>,
) -> Result<RegistryMutationResult, ApiError> {
    let token = token.ok_or(ApiError::Unauthorized)?;
    hold_registry_publish_request_native(
        token,
        tenant_slug.unwrap_or_default(),
        request_id,
        reason,
        reason_code,
        dry_run,
    )
    .await
    .map_err(|error| ApiError::Graphql(error.to_string()))
}

pub async fn resume_registry_publish_request(
    request_id: String,
    reason: String,
    reason_code: String,
    dry_run: bool,
    token: Option<String>,
    tenant_slug: Option<String>,
) -> Result<RegistryMutationResult, ApiError> {
    let token = token.ok_or(ApiError::Unauthorized)?;
    resume_registry_publish_request_native(
        token,
        tenant_slug.unwrap_or_default(),
        request_id,
        reason,
        reason_code,
        dry_run,
    )
    .await
    .map_err(|error| ApiError::Graphql(error.to_string()))
}

pub async fn transfer_registry_owner(
    slug: String,
    new_owner_user_id: String,
    reason: String,
    reason_code: String,
    dry_run: bool,
    token: Option<String>,
    tenant_slug: Option<String>,
) -> Result<RegistryMutationResult, ApiError> {
    let token = token.ok_or(ApiError::Unauthorized)?;
    transfer_registry_owner_native(
        token,
        tenant_slug.unwrap_or_default(),
        slug,
        new_owner_user_id,
        reason,
        reason_code,
        dry_run,
    )
    .await
    .map_err(|error| ApiError::Graphql(error.to_string()))
}

pub async fn yank_registry_release(
    slug: String,
    version: String,
    reason: String,
    reason_code: String,
    dry_run: bool,
    token: Option<String>,
    tenant_slug: Option<String>,
) -> Result<RegistryMutationResult, ApiError> {
    let token = token.ok_or(ApiError::Unauthorized)?;
    yank_registry_release_native(
        token,
        tenant_slug.unwrap_or_default(),
        slug,
        version,
        reason,
        reason_code,
        dry_run,
    )
    .await
    .map_err(|error| ApiError::Graphql(error.to_string()))
}
