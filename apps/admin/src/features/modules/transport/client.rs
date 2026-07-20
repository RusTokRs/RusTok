use leptos::prelude::*;
use rustok_ui_transport::UiTransportPath;

#[allow(unused_imports)]
use crate::entities::module::model::{
    MarketplaceModuleVersion, RegistryFollowUpGateLifecycle, RegistryGovernanceActionLifecycle,
    RegistryGovernanceEventLifecycle, RegistryGovernanceEventPayloadLifecycle,
    RegistryModuleLifecycle, RegistryOwnerLifecycle, RegistryPublishRequestLifecycle,
    RegistryReleaseLifecycle, RegistryValidationStageLifecycle,
    registry_principal_label_from_value,
};
use crate::entities::module::{
    BuildJob, InstalledModule, MarketplaceModule, ModuleInfo, ModuleOperationRecoveryPlan,
    ReleaseInfo, TenantModule, ToggleModuleResult,
};
use crate::shared::api::{ApiError, map_server_fn_error, request};

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

pub fn bundled_humanize_module_slug(slug: &str) -> String {
    slug.split(['-', '_'])
        .filter(|part| !part.is_empty())
        .map(|part| {
            let mut chars = part.chars();
            match chars.next() {
                Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

pub fn bundled_module_category(nav_group: &str) -> String {
    match nav_group {
        "Content" => "content",
        "Commerce" => "commerce",
        "Runtime" => "runtime",
        "Governance" => "governance",
        "Automation" => "automation",
        _ => "extensions",
    }
    .to_string()
}

pub fn fallback_module_registry() -> Vec<ModuleInfo> {
    let core_slugs = crate::app::modules::core_module_slugs();
    let mut modules = crate::app::modules::module_navigation_entries()
        .iter()
        .map(|entry| {
            let metadata = crate::app::modules::module_runtime_metadata(entry.module_slug);
            let is_core = core_slugs.contains(&entry.module_slug);
            ModuleInfo {
                module_slug: entry.module_slug.to_string(),
                name: entry.nav_label.to_string(),
                description: format!("{} module", entry.nav_label),
                version: "workspace".to_string(),
                kind: if is_core { "core" } else { "optional" }.to_string(),
                dependencies: Vec::new(),
                enabled: true,
                ownership: metadata
                    .map(|metadata| metadata.ownership.to_string())
                    .unwrap_or_else(|| "first_party".to_string()),
                trust_level: metadata
                    .map(|metadata| metadata.trust_level.to_string())
                    .unwrap_or_else(|| "trusted".to_string()),
                has_admin_ui: true,
                has_storefront_ui: false,
                ui_classification: "admin".to_string(),
                recommended_admin_surfaces: metadata
                    .map(|metadata| {
                        metadata
                            .recommended_admin_surfaces
                            .iter()
                            .map(|surface| surface.to_string())
                            .collect()
                    })
                    .unwrap_or_default(),
                showcase_admin_surfaces: metadata
                    .map(|metadata| {
                        metadata
                            .showcase_admin_surfaces
                            .iter()
                            .map(|surface| surface.to_string())
                            .collect()
                    })
                    .unwrap_or_default(),
            }
        })
        .collect::<Vec<_>>();
    modules.sort_by(|left, right| left.module_slug.cmp(&right.module_slug));
    modules.dedup_by(|left, right| left.module_slug == right.module_slug);
    modules
}

pub fn fallback_installed_modules() -> Vec<InstalledModule> {
    let core_slugs = crate::app::modules::core_module_slugs();
    let mut modules = crate::app::modules::module_navigation_entries()
        .iter()
        .map(|entry| InstalledModule {
            slug: entry.module_slug.to_string(),
            source: "bundled".to_string(),
            crate_name: format!("rustok-{}", entry.module_slug),
            version: Some("workspace".to_string()),
            required: core_slugs.contains(&entry.module_slug),
            dependencies: Vec::new(),
        })
        .collect::<Vec<_>>();
    modules.sort_by(|left, right| left.slug.cmp(&right.slug));
    modules.dedup_by(|left, right| left.slug == right.slug);
    modules
}

pub fn fallback_tenant_modules() -> Vec<TenantModule> {
    let mut modules = crate::app::modules::module_navigation_entries()
        .iter()
        .map(|entry| TenantModule {
            module_slug: entry.module_slug.to_string(),
            enabled: true,
            settings: "{}".to_string(),
        })
        .collect::<Vec<_>>();
    modules.sort_by(|left, right| left.module_slug.cmp(&right.module_slug));
    modules.dedup_by(|left, right| left.module_slug == right.module_slug);
    modules
}

pub fn fallback_marketplace_module_from_entry(
    entry: &crate::app::modules::GeneratedModuleNavigationEntry,
) -> MarketplaceModule {
    let metadata = crate::app::modules::module_runtime_metadata(entry.module_slug);
    MarketplaceModule {
        slug: entry.module_slug.to_string(),
        name: entry.nav_label.to_string(),
        latest_version: "workspace".to_string(),
        description: format!("{} module", entry.nav_label),
        source: "bundled".to_string(),
        kind: "optional".to_string(),
        category: bundled_module_category(entry.nav_group),
        tags: vec![entry.nav_group.to_ascii_lowercase()],
        icon_url: None,
        banner_url: None,
        screenshots: Vec::new(),
        crate_name: format!("rustok-{}", entry.module_slug),
        dependencies: Vec::new(),
        ownership: metadata
            .map(|metadata| metadata.ownership.to_string())
            .unwrap_or_else(|| "first_party".to_string()),
        trust_level: metadata
            .map(|metadata| metadata.trust_level.to_string())
            .unwrap_or_else(|| "trusted".to_string()),
        rustok_min_version: None,
        rustok_max_version: None,
        publisher: None,
        checksum_sha256: None,
        signature_present: false,
        versions: vec![MarketplaceModuleVersion {
            version: "workspace".to_string(),
            changelog: None,
            yanked: false,
            published_at: None,
            checksum_sha256: None,
            signature_present: false,
        }],
        has_admin_ui: true,
        has_storefront_ui: false,
        ui_classification: "admin".to_string(),
        registry_lifecycle: None,
        compatible: true,
        recommended_admin_surfaces: metadata
            .map(|metadata| {
                metadata
                    .recommended_admin_surfaces
                    .iter()
                    .map(|surface| surface.to_string())
                    .collect()
            })
            .unwrap_or_default(),
        showcase_admin_surfaces: metadata
            .map(|metadata| {
                metadata
                    .showcase_admin_surfaces
                    .iter()
                    .map(|surface| surface.to_string())
                    .collect()
            })
            .unwrap_or_default(),
        settings_schema: Vec::new(),
        installed: true,
        installed_version: Some("workspace".to_string()),
        update_available: false,
    }
}

pub fn fallback_marketplace_modules(variables: &MarketplaceVariables) -> Vec<MarketplaceModule> {
    let search = variables.search.as_ref().map(|value| value.to_lowercase());
    let category = variables
        .category
        .as_ref()
        .map(|value| value.to_lowercase());
    let tag = variables.tag.as_ref().map(|value| value.to_lowercase());
    let source = variables.source.as_ref().map(|value| value.to_lowercase());
    let trust_level = variables
        .trust_level
        .as_ref()
        .map(|value| value.to_lowercase());
    let installed_only = variables.installed_only.unwrap_or(false);

    let mut modules = crate::app::modules::module_navigation_entries()
        .iter()
        .map(fallback_marketplace_module_from_entry)
        .filter(|module| !installed_only || module.installed)
        .filter(|module| {
            category
                .as_ref()
                .is_none_or(|value| value == "all" || module.category.eq_ignore_ascii_case(value))
        })
        .filter(|module| {
            tag.as_ref().is_none_or(|value| {
                value == "all"
                    || module
                        .tags
                        .iter()
                        .any(|module_tag| module_tag.eq_ignore_ascii_case(value))
            })
        })
        .filter(|module| {
            source
                .as_ref()
                .is_none_or(|value| value == "all" || module.source.eq_ignore_ascii_case(value))
        })
        .filter(|module| {
            trust_level.as_ref().is_none_or(|value| {
                value == "all" || module.trust_level.eq_ignore_ascii_case(value)
            })
        })
        .filter(|module| {
            search.as_ref().is_none_or(|value| {
                module.slug.to_lowercase().contains(value)
                    || module.name.to_lowercase().contains(value)
                    || module.description.to_lowercase().contains(value)
                    || module.crate_name.to_lowercase().contains(value)
            })
        })
        .collect::<Vec<_>>();

    modules.sort_by(|left, right| left.slug.cmp(&right.slug));
    modules.dedup_by(|left, right| left.slug == right.slug);
    modules
}

pub fn fallback_marketplace_module(slug: &str) -> Option<MarketplaceModule> {
    let slug = slug.trim();
    crate::app::modules::module_navigation_entries()
        .iter()
        .find(|entry| entry.module_slug.eq_ignore_ascii_case(slug))
        .map(fallback_marketplace_module_from_entry)
        .or_else(|| {
            (!slug.is_empty()).then(|| {
                let label = bundled_humanize_module_slug(slug);
                MarketplaceModule {
                    slug: slug.to_string(),
                    name: label.clone(),
                    latest_version: "workspace".to_string(),
                    description: format!("{label} module"),
                    source: "bundled".to_string(),
                    kind: "optional".to_string(),
                    category: "extensions".to_string(),
                    tags: Vec::new(),
                    icon_url: None,
                    banner_url: None,
                    screenshots: Vec::new(),
                    crate_name: format!("rustok-{slug}"),
                    dependencies: Vec::new(),
                    ownership: "first_party".to_string(),
                    trust_level: "trusted".to_string(),
                    rustok_min_version: None,
                    rustok_max_version: None,
                    publisher: None,
                    checksum_sha256: None,
                    signature_present: false,
                    versions: vec![MarketplaceModuleVersion {
                        version: "workspace".to_string(),
                        changelog: None,
                        yanked: false,
                        published_at: None,
                        checksum_sha256: None,
                        signature_present: false,
                    }],
                    has_admin_ui: true,
                    has_storefront_ui: false,
                    ui_classification: "admin".to_string(),
                    registry_lifecycle: None,
                    compatible: true,
                    recommended_admin_surfaces: Vec::new(),
                    showcase_admin_surfaces: Vec::new(),
                    settings_schema: Vec::new(),
                    installed: true,
                    installed_version: Some("workspace".to_string()),
                    update_available: false,
                }
            })
        })
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
            let response: Result<ModuleRegistryResponse, ApiError> = request(
                MODULE_REGISTRY_QUERY,
                serde_json::json!({}),
                token,
                tenant_slug,
            )
            .await;
            match response {
                Ok(response) => Ok(response.module_registry),
                Err(_) => Ok(fallback_module_registry()),
            }
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
            let response: Result<InstalledModulesResponse, ApiError> = request(
                INSTALLED_MODULES_QUERY,
                serde_json::json!({}),
                token,
                tenant_slug,
            )
            .await;
            match response {
                Ok(response) => Ok(response.installed_modules),
                Err(_) => Ok(fallback_installed_modules()),
            }
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
            let response: Result<TenantModulesResponse, ApiError> = request(
                TENANT_MODULES_QUERY,
                serde_json::json!({}),
                token,
                tenant_slug,
            )
            .await;
            match response {
                Ok(response) => Ok(response.tenant_modules),
                Err(_) => Ok(fallback_tenant_modules()),
            }
        }
    }
}

pub async fn fetch_marketplace_modules(
    token: Option<String>,
    tenant_slug: Option<String>,
    variables: MarketplaceVariables,
) -> Result<Vec<MarketplaceModule>, ApiError> {
    if token.is_some() {
        let response: Result<MarketplaceResponse, ApiError> = request(
            MARKETPLACE_QUERY,
            variables.clone(),
            token.clone(),
            tenant_slug.clone(),
        )
        .await;
        return match response {
            Ok(response) => Ok(response.marketplace),
            Err(_) => Ok(fallback_marketplace_modules(&variables)),
        };
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
            let fallback_variables = variables.clone();
            let response: Result<MarketplaceResponse, ApiError> =
                request(MARKETPLACE_QUERY, variables, token, tenant_slug).await;
            match response {
                Ok(response) => Ok(response.marketplace),
                Err(_) => Ok(fallback_marketplace_modules(&fallback_variables)),
            }
        }
    }
}

pub async fn fetch_marketplace_module(
    slug: String,
    token: Option<String>,
    tenant_slug: Option<String>,
) -> Result<Option<MarketplaceModule>, ApiError> {
    if token.is_some() {
        let response: Result<MarketplaceModuleResponse, ApiError> = request(
            MARKETPLACE_MODULE_QUERY,
            MarketplaceModuleVariables { slug: slug.clone() },
            token.clone(),
            tenant_slug.clone(),
        )
        .await;
        return match response {
            Ok(response) => Ok(response.marketplace_module),
            Err(_) => Ok(fallback_marketplace_module(&slug)),
        };
    }

    match selected_transport_path() {
        UiTransportPath::NativeServer => marketplace_module_native(slug)
            .await
            .map_err(map_server_fn_error),
        UiTransportPath::Graphql => {
            let fallback_slug = slug.clone();
            let response: Result<MarketplaceModuleResponse, ApiError> = request(
                MARKETPLACE_MODULE_QUERY,
                MarketplaceModuleVariables { slug },
                token,
                tenant_slug,
            )
            .await;
            match response {
                Ok(response) => Ok(response.marketplace_module),
                Err(_) => Ok(fallback_marketplace_module(&fallback_slug)),
            }
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
