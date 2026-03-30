use leptos::prelude::*;
use serde::{Deserialize, Serialize};

use crate::entities::module::{
    BuildJob, InstalledModule, MarketplaceModule, ModuleInfo, ReleaseInfo, TenantModule,
    ToggleModuleResult,
};
use crate::shared::api::{request, ApiError};

pub const ENABLED_MODULES_QUERY: &str = "query EnabledModules { enabledModules }";
pub const MODULE_REGISTRY_QUERY: &str =
    "query ModuleRegistry { moduleRegistry { moduleSlug name description version kind dependencies enabled ownership trustLevel recommendedAdminSurfaces showcaseAdminSurfaces } }";
pub const INSTALLED_MODULES_QUERY: &str =
    "query InstalledModules { installedModules { slug source crateName version required dependencies } }";
pub const TENANT_MODULES_QUERY: &str =
    "query TenantModules { tenantModules { moduleSlug enabled settings } }";
pub const MARKETPLACE_QUERY: &str =
    "query Marketplace($search: String, $category: String, $source: String, $trustLevel: String, $onlyCompatible: Boolean, $installedOnly: Boolean) { marketplace(search: $search, category: $category, source: $source, trustLevel: $trustLevel, onlyCompatible: $onlyCompatible, installedOnly: $installedOnly) { slug name latestVersion description source kind category tags iconUrl bannerUrl screenshots crateName dependencies ownership trustLevel rustokMinVersion rustokMaxVersion publisher checksumSha256 signaturePresent versions { version changelog yanked publishedAt checksumSha256 signaturePresent } compatible recommendedAdminSurfaces showcaseAdminSurfaces settingsSchema { key type required defaultValue description min max options } installed installedVersion updateAvailable } }";
pub const MARKETPLACE_MODULE_QUERY: &str =
    "query MarketplaceModule($slug: String!) { marketplaceModule(slug: $slug) { slug name latestVersion description source kind category tags iconUrl bannerUrl screenshots crateName dependencies ownership trustLevel rustokMinVersion rustokMaxVersion publisher checksumSha256 signaturePresent versions { version changelog yanked publishedAt checksumSha256 signaturePresent } compatible recommendedAdminSurfaces showcaseAdminSurfaces settingsSchema { key type required defaultValue description min max options } installed installedVersion updateAvailable } }";
pub const ACTIVE_BUILD_QUERY: &str =
    "query ActiveBuild { activeBuild { id status stage progress profile manifestRef manifestHash modulesDelta requestedBy reason releaseId logsUrl errorMessage startedAt createdAt updatedAt finishedAt } }";
pub const ACTIVE_RELEASE_QUERY: &str =
    "query ActiveRelease { activeRelease { id buildId status environment manifestHash modules previousReleaseId deployedAt rolledBackAt createdAt updatedAt } }";
pub const BUILD_HISTORY_QUERY: &str =
    "query BuildHistory($limit: Int!, $offset: Int!) { buildHistory(limit: $limit, offset: $offset) { id status stage progress profile manifestRef manifestHash modulesDelta requestedBy reason releaseId logsUrl errorMessage startedAt createdAt updatedAt finishedAt } }";
pub const BUILD_PROGRESS_SUBSCRIPTION: &str =
    "subscription BuildProgress { buildProgress { buildId status stage progress releaseId errorMessage } }";
pub const TOGGLE_MODULE_MUTATION: &str =
    "mutation ToggleModule($moduleSlug: String!, $enabled: Boolean!) { toggleModule(moduleSlug: $moduleSlug, enabled: $enabled) { moduleSlug enabled settings } }";
pub const UPDATE_MODULE_SETTINGS_MUTATION: &str =
    "mutation UpdateModuleSettings($moduleSlug: String!, $settings: String!) { updateModuleSettings(moduleSlug: $moduleSlug, settings: $settings) { moduleSlug enabled settings } }";
pub const INSTALL_MODULE_MUTATION: &str =
    "mutation InstallModule($slug: String!, $version: String!) { installModule(slug: $slug, version: $version) { id status stage progress modulesDelta requestedBy reason createdAt updatedAt finishedAt } }";
pub const UNINSTALL_MODULE_MUTATION: &str =
    "mutation UninstallModule($slug: String!) { uninstallModule(slug: $slug) { id status stage progress modulesDelta requestedBy reason createdAt updatedAt finishedAt } }";
pub const UPGRADE_MODULE_MUTATION: &str =
    "mutation UpgradeModule($slug: String!, $version: String!) { upgradeModule(slug: $slug, version: $version) { id status stage progress profile manifestRef manifestHash modulesDelta requestedBy reason releaseId logsUrl errorMessage startedAt createdAt updatedAt finishedAt } }";
pub const ROLLBACK_BUILD_MUTATION: &str =
    "mutation RollbackBuild($buildId: String!) { rollbackBuild(buildId: $buildId) { id status stage progress profile manifestRef manifestHash modulesDelta requestedBy reason releaseId logsUrl errorMessage startedAt createdAt updatedAt finishedAt } }";

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct EnabledModulesResponse {
    #[serde(rename = "enabledModules")]
    pub enabled_modules: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ModuleRegistryResponse {
    #[serde(rename = "moduleRegistry")]
    pub module_registry: Vec<ModuleInfo>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct InstalledModulesResponse {
    #[serde(rename = "installedModules")]
    pub installed_modules: Vec<InstalledModule>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct TenantModulesResponse {
    #[serde(rename = "tenantModules")]
    pub tenant_modules: Vec<TenantModule>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct MarketplaceResponse {
    pub marketplace: Vec<MarketplaceModule>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct MarketplaceModuleResponse {
    #[serde(rename = "marketplaceModule")]
    pub marketplace_module: Option<MarketplaceModule>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ActiveBuildResponse {
    #[serde(rename = "activeBuild")]
    pub active_build: Option<BuildJob>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ActiveReleaseResponse {
    #[serde(rename = "activeRelease")]
    pub active_release: Option<ReleaseInfo>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct BuildHistoryResponse {
    #[serde(rename = "buildHistory")]
    pub build_history: Vec<BuildJob>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct BuildProgressEvent {
    #[serde(rename = "buildId")]
    pub build_id: String,
    pub status: String,
    pub stage: String,
    pub progress: i32,
    #[serde(rename = "releaseId")]
    pub release_id: Option<String>,
    #[serde(rename = "errorMessage")]
    pub error_message: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ToggleModuleResponse {
    #[serde(rename = "toggleModule")]
    pub toggle_module: ToggleModuleResult,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct UpdateModuleSettingsResponse {
    #[serde(rename = "updateModuleSettings")]
    pub update_module_settings: TenantModule,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct InstallModuleResponse {
    #[serde(rename = "installModule")]
    pub install_module: BuildJob,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct UninstallModuleResponse {
    #[serde(rename = "uninstallModule")]
    pub uninstall_module: BuildJob,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct UpgradeModuleResponse {
    #[serde(rename = "upgradeModule")]
    pub upgrade_module: BuildJob,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct RollbackBuildResponse {
    #[serde(rename = "rollbackBuild")]
    pub rollback_build: BuildJob,
}

#[derive(Clone, Debug, Serialize)]
pub struct ToggleModuleVariables {
    #[serde(rename = "moduleSlug")]
    pub module_slug: String,
    pub enabled: bool,
}

#[derive(Clone, Debug, Serialize)]
pub struct UpdateModuleSettingsVariables {
    #[serde(rename = "moduleSlug")]
    pub module_slug: String,
    pub settings: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct BuildHistoryVariables {
    pub limit: i32,
    pub offset: i32,
}

#[derive(Clone, Debug, Serialize)]
pub struct MarketplaceVariables {
    pub search: Option<String>,
    pub category: Option<String>,
    pub source: Option<String>,
    #[serde(rename = "trustLevel")]
    pub trust_level: Option<String>,
    #[serde(rename = "onlyCompatible")]
    pub only_compatible: Option<bool>,
    #[serde(rename = "installedOnly")]
    pub installed_only: Option<bool>,
}

#[derive(Clone, Debug, Serialize)]
pub struct MarketplaceModuleVariables {
    pub slug: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct InstallModuleVariables {
    pub slug: String,
    pub version: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct UninstallModuleVariables {
    pub slug: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct UpgradeModuleVariables {
    pub slug: String,
    pub version: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct RollbackBuildVariables {
    #[serde(rename = "buildId")]
    pub build_id: String,
}

#[cfg(feature = "ssr")]
#[derive(Debug, Deserialize, Default)]
struct RuntimeModulesManifest {
    #[serde(default)]
    modules: std::collections::HashMap<String, RuntimeManifestModuleSpec>,
}

#[cfg(feature = "ssr")]
#[derive(Debug, Deserialize, Default)]
struct RuntimeManifestModuleSpec {
    source: String,
    #[serde(rename = "crate", default)]
    crate_name: String,
    #[serde(default)]
    version: Option<String>,
    #[serde(default)]
    required: bool,
    #[serde(default)]
    depends_on: Vec<String>,
}

#[cfg(feature = "ssr")]
fn server_error(message: impl Into<String>) -> ServerFnError {
    ServerFnError::ServerError(message.into())
}

fn combine_native_and_graphql_error(server_err: ServerFnError, graphql_err: ApiError) -> ApiError {
    ApiError::Graphql(format!(
        "native path failed: {}; graphql path failed: {}",
        server_err, graphql_err
    ))
}

#[cfg(feature = "ssr")]
async fn modules_server_context() -> Result<
    (
        loco_rs::app::AppContext,
        rustok_api::AuthContext,
        rustok_api::TenantContext,
    ),
    ServerFnError,
> {
    use leptos::prelude::expect_context;
    use leptos_axum::extract;
    use loco_rs::app::AppContext;
    use rustok_api::{has_any_effective_permission, AuthContext, TenantContext};
    use rustok_core::Permission;

    let app_ctx = expect_context::<AppContext>();
    let auth = extract::<AuthContext>()
        .await
        .map_err(|err| server_error(err.to_string()))?;
    let tenant = extract::<TenantContext>()
        .await
        .map_err(|err| server_error(err.to_string()))?;

    if !has_any_effective_permission(
        &auth.permissions,
        &[
            Permission::MODULES_READ,
            Permission::MODULES_LIST,
            Permission::MODULES_MANAGE,
        ],
    ) {
        return Err(ServerFnError::new(
            "modules:read, modules:list, or modules:manage required",
        ));
    }

    Ok((app_ctx, auth, tenant))
}

#[cfg(feature = "ssr")]
fn upper_snake(value: &str) -> String {
    value
        .replace('-', "_")
        .split('_')
        .filter(|part| !part.is_empty())
        .map(|part| part.to_ascii_uppercase())
        .collect::<Vec<_>>()
        .join("_")
}

#[cfg(feature = "ssr")]
fn build_modules_delta_summary(value: Option<&serde_json::Value>) -> String {
    let Some(value) = value else {
        return String::new();
    };

    if let Some(summary) = value.as_str() {
        return summary.to_string();
    }

    if let Some(summary) = value.get("summary").and_then(serde_json::Value::as_str) {
        return summary.to_string();
    }

    if let Some(object) = value.as_object() {
        let mut slugs = object.keys().cloned().collect::<Vec<_>>();
        slugs.sort();
        return slugs.join(",");
    }

    value.to_string()
}

#[cfg(feature = "ssr")]
fn runtime_modules_manifest_path() -> std::path::PathBuf {
    if let Ok(path) = std::env::var("RUSTOK_MODULES_MANIFEST") {
        return std::path::PathBuf::from(path);
    }

    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../modules.toml")
}

#[cfg(feature = "ssr")]
fn load_runtime_modules_manifest() -> Result<RuntimeModulesManifest, ServerFnError> {
    let path = runtime_modules_manifest_path();
    let raw = std::fs::read_to_string(&path)
        .map_err(|err| server_error(format!("failed to read {}: {err}", path.display())))?;
    toml::from_str(&raw)
        .map_err(|err| server_error(format!("failed to parse {}: {err}", path.display())))
}

#[cfg(feature = "ssr")]
fn map_build_job_row(row: sea_orm::QueryResult) -> Result<BuildJob, ServerFnError> {
    let modules_delta = row
        .try_get::<Option<serde_json::Value>>("", "modules_delta")
        .map_err(|err| server_error(err.to_string()))?;

    Ok(BuildJob {
        id: row
            .try_get::<uuid::Uuid>("", "id")
            .map(|value| value.to_string())
            .map_err(|err| server_error(err.to_string()))?,
        status: upper_snake(
            &row.try_get::<String>("", "status")
                .map_err(|err| server_error(err.to_string()))?,
        ),
        stage: upper_snake(
            &row.try_get::<String>("", "stage")
                .map_err(|err| server_error(err.to_string()))?,
        ),
        progress: row
            .try_get("", "progress")
            .map_err(|err| server_error(err.to_string()))?,
        profile: upper_snake(
            &row.try_get::<String>("", "profile")
                .map_err(|err| server_error(err.to_string()))?,
        ),
        manifest_ref: row
            .try_get("", "manifest_ref")
            .map_err(|err| server_error(err.to_string()))?,
        manifest_hash: row
            .try_get("", "manifest_hash")
            .map_err(|err| server_error(err.to_string()))?,
        modules_delta: build_modules_delta_summary(modules_delta.as_ref()),
        requested_by: row
            .try_get("", "requested_by")
            .map_err(|err| server_error(err.to_string()))?,
        reason: row
            .try_get("", "reason")
            .map_err(|err| server_error(err.to_string()))?,
        release_id: row
            .try_get("", "release_id")
            .map_err(|err| server_error(err.to_string()))?,
        logs_url: row
            .try_get("", "logs_url")
            .map_err(|err| server_error(err.to_string()))?,
        error_message: row
            .try_get("", "error_message")
            .map_err(|err| server_error(err.to_string()))?,
        started_at: row
            .try_get::<Option<chrono::DateTime<chrono::Utc>>>("", "started_at")
            .map(|value| value.map(|value| value.to_rfc3339()))
            .map_err(|err| server_error(err.to_string()))?,
        created_at: row
            .try_get::<chrono::DateTime<chrono::Utc>>("", "created_at")
            .map(|value| value.to_rfc3339())
            .map_err(|err| server_error(err.to_string()))?,
        updated_at: row
            .try_get::<chrono::DateTime<chrono::Utc>>("", "updated_at")
            .map(|value| value.to_rfc3339())
            .map_err(|err| server_error(err.to_string()))?,
        finished_at: row
            .try_get::<Option<chrono::DateTime<chrono::Utc>>>("", "finished_at")
            .map(|value| value.map(|value| value.to_rfc3339()))
            .map_err(|err| server_error(err.to_string()))?,
    })
}

#[cfg(feature = "ssr")]
fn map_release_info_row(row: sea_orm::QueryResult) -> Result<ReleaseInfo, ServerFnError> {
    let modules = row
        .try_get::<serde_json::Value>("", "modules")
        .ok()
        .and_then(|value| serde_json::from_value::<Vec<String>>(value).ok())
        .unwrap_or_default();

    Ok(ReleaseInfo {
        id: row
            .try_get("", "id")
            .map_err(|err| server_error(err.to_string()))?,
        build_id: row
            .try_get::<uuid::Uuid>("", "build_id")
            .map(|value| value.to_string())
            .map_err(|err| server_error(err.to_string()))?,
        status: upper_snake(
            &row.try_get::<String>("", "status")
                .map_err(|err| server_error(err.to_string()))?,
        ),
        environment: row
            .try_get("", "environment")
            .map_err(|err| server_error(err.to_string()))?,
        manifest_hash: row
            .try_get("", "manifest_hash")
            .map_err(|err| server_error(err.to_string()))?,
        modules,
        previous_release_id: row
            .try_get("", "previous_release_id")
            .map_err(|err| server_error(err.to_string()))?,
        deployed_at: row
            .try_get::<Option<chrono::DateTime<chrono::Utc>>>("", "deployed_at")
            .map(|value| value.map(|value| value.to_rfc3339()))
            .map_err(|err| server_error(err.to_string()))?,
        rolled_back_at: row
            .try_get::<Option<chrono::DateTime<chrono::Utc>>>("", "rolled_back_at")
            .map(|value| value.map(|value| value.to_rfc3339()))
            .map_err(|err| server_error(err.to_string()))?,
        created_at: row
            .try_get::<chrono::DateTime<chrono::Utc>>("", "created_at")
            .map(|value| value.to_rfc3339())
            .map_err(|err| server_error(err.to_string()))?,
        updated_at: row
            .try_get::<chrono::DateTime<chrono::Utc>>("", "updated_at")
            .map(|value| value.to_rfc3339())
            .map_err(|err| server_error(err.to_string()))?,
    })
}

pub async fn fetch_enabled_modules(
    token: Option<String>,
    tenant_slug: Option<String>,
) -> Result<Vec<String>, ApiError> {
    match fetch_enabled_modules_server().await {
        Ok(modules) => Ok(modules),
        Err(_) => fetch_enabled_modules_graphql(token, tenant_slug).await,
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

#[server(prefix = "/api/fn", endpoint = "admin/list-enabled-modules")]
async fn list_enabled_modules_native() -> Result<Vec<String>, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use leptos::prelude::expect_context;
        use leptos_axum::extract;
        use loco_rs::app::AppContext;
        use rustok_api::{has_any_effective_permission, AuthContext, TenantContext};
        use rustok_core::Permission;
        use rustok_tenant::TenantService;

        let app_ctx = expect_context::<AppContext>();
        let auth = extract::<AuthContext>().await.map_err(ServerFnError::new)?;
        let tenant = extract::<TenantContext>()
            .await
            .map_err(ServerFnError::new)?;

        if !has_any_effective_permission(
            &auth.permissions,
            &[
                Permission::MODULES_READ,
                Permission::MODULES_LIST,
                Permission::MODULES_MANAGE,
            ],
        ) {
            return Err(ServerFnError::new(
                "modules:read, modules:list, or modules:manage required",
            ));
        }

        let mut modules = TenantService::new(app_ctx.db.clone())
            .list_tenant_modules(tenant.id)
            .await
            .map_err(ServerFnError::new)?
            .into_iter()
            .filter(|module| module.enabled)
            .map(|module| module.module_slug)
            .collect::<Vec<_>>();

        modules.sort();
        Ok(modules)
    }
    #[cfg(not(feature = "ssr"))]
    {
        Err(ServerFnError::new(
            "admin/list-enabled-modules requires the `ssr` feature",
        ))
    }
}

pub async fn fetch_modules(
    token: Option<String>,
    tenant_slug: Option<String>,
) -> Result<Vec<ModuleInfo>, ApiError> {
    match list_module_registry_native().await {
        Ok(modules) => Ok(modules),
        Err(server_err) => {
            let response: ModuleRegistryResponse = request(
                MODULE_REGISTRY_QUERY,
                serde_json::json!({}),
                token,
                tenant_slug,
            )
            .await
            .map_err(|graphql_err| combine_native_and_graphql_error(server_err, graphql_err))?;
            Ok(response.module_registry)
        }
    }
}

#[server(prefix = "/api/fn", endpoint = "admin/module-registry")]
async fn list_module_registry_native() -> Result<Vec<ModuleInfo>, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use crate::app::modules::module_runtime_metadata;
        use leptos::prelude::expect_context;
        use rustok_core::ModuleRegistry;
        use rustok_tenant::TenantService;

        let (app_ctx, _auth, tenant) = modules_server_context().await?;
        let registry = expect_context::<ModuleRegistry>();
        let enabled_modules = TenantService::new(app_ctx.db.clone())
            .list_tenant_modules(tenant.id)
            .await
            .map_err(|err| server_error(err.to_string()))?
            .into_iter()
            .filter(|module| module.enabled)
            .map(|module| module.module_slug)
            .collect::<std::collections::HashSet<_>>();

        Ok(registry
            .list()
            .into_iter()
            .map(|module| {
                let metadata = module_runtime_metadata(module.slug());
                ModuleInfo {
                    module_slug: module.slug().to_string(),
                    name: module.name().to_string(),
                    description: module.description().to_string(),
                    version: module.version().to_string(),
                    kind: if registry.is_core(module.slug()) {
                        "core".to_string()
                    } else {
                        "optional".to_string()
                    },
                    dependencies: module
                        .dependencies()
                        .iter()
                        .map(|dependency| dependency.to_string())
                        .collect(),
                    enabled: registry.is_core(module.slug())
                        || enabled_modules.contains(module.slug()),
                    ownership: metadata
                        .map(|metadata| metadata.ownership.to_string())
                        .unwrap_or_else(|| "third_party".to_string()),
                    trust_level: metadata
                        .map(|metadata| metadata.trust_level.to_string())
                        .unwrap_or_else(|| "unverified".to_string()),
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
            .collect())
    }
    #[cfg(not(feature = "ssr"))]
    {
        Err(ServerFnError::new(
            "admin/module-registry requires the `ssr` feature",
        ))
    }
}

pub async fn fetch_installed_modules(
    token: Option<String>,
    tenant_slug: Option<String>,
) -> Result<Vec<InstalledModule>, ApiError> {
    match list_installed_modules_native().await {
        Ok(modules) => Ok(modules),
        Err(server_err) => {
            let response: InstalledModulesResponse = request(
                INSTALLED_MODULES_QUERY,
                serde_json::json!({}),
                token,
                tenant_slug,
            )
            .await
            .map_err(|graphql_err| combine_native_and_graphql_error(server_err, graphql_err))?;
            Ok(response.installed_modules)
        }
    }
}

#[server(prefix = "/api/fn", endpoint = "admin/installed-modules")]
async fn list_installed_modules_native() -> Result<Vec<InstalledModule>, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        let (_app_ctx, _auth, _tenant) = modules_server_context().await?;
        let manifest = load_runtime_modules_manifest()?;

        let mut modules = manifest
            .modules
            .into_iter()
            .map(|(slug, spec)| InstalledModule {
                slug,
                source: spec.source,
                crate_name: spec.crate_name,
                version: spec.version,
                required: spec.required,
                dependencies: spec.depends_on,
            })
            .collect::<Vec<_>>();
        modules.sort_by(|left, right| left.slug.cmp(&right.slug));
        Ok(modules)
    }
    #[cfg(not(feature = "ssr"))]
    {
        Err(ServerFnError::new(
            "admin/installed-modules requires the `ssr` feature",
        ))
    }
}

pub async fn fetch_tenant_modules(
    token: Option<String>,
    tenant_slug: Option<String>,
) -> Result<Vec<TenantModule>, ApiError> {
    match list_tenant_modules_native().await {
        Ok(modules) => Ok(modules),
        Err(server_err) => {
            let response: TenantModulesResponse = request(
                TENANT_MODULES_QUERY,
                serde_json::json!({}),
                token,
                tenant_slug,
            )
            .await
            .map_err(|graphql_err| combine_native_and_graphql_error(server_err, graphql_err))?;
            Ok(response.tenant_modules)
        }
    }
}

pub async fn fetch_marketplace_modules(
    token: Option<String>,
    tenant_slug: Option<String>,
    variables: MarketplaceVariables,
) -> Result<Vec<MarketplaceModule>, ApiError> {
    let response: MarketplaceResponse =
        request(MARKETPLACE_QUERY, variables, token, tenant_slug).await?;
    Ok(response.marketplace)
}

pub async fn fetch_marketplace_module(
    slug: String,
    token: Option<String>,
    tenant_slug: Option<String>,
) -> Result<Option<MarketplaceModule>, ApiError> {
    let response: MarketplaceModuleResponse = request(
        MARKETPLACE_MODULE_QUERY,
        MarketplaceModuleVariables { slug },
        token,
        tenant_slug,
    )
    .await?;
    Ok(response.marketplace_module)
}

pub async fn fetch_active_build(
    token: Option<String>,
    tenant_slug: Option<String>,
) -> Result<Option<BuildJob>, ApiError> {
    match active_build_native().await {
        Ok(build) => Ok(build),
        Err(server_err) => {
            let response: ActiveBuildResponse = request(
                ACTIVE_BUILD_QUERY,
                serde_json::json!({}),
                token,
                tenant_slug,
            )
            .await
            .map_err(|graphql_err| combine_native_and_graphql_error(server_err, graphql_err))?;
            Ok(response.active_build)
        }
    }
}

pub async fn fetch_active_release(
    token: Option<String>,
    tenant_slug: Option<String>,
) -> Result<Option<ReleaseInfo>, ApiError> {
    match active_release_native().await {
        Ok(release) => Ok(release),
        Err(server_err) => {
            let response: ActiveReleaseResponse = request(
                ACTIVE_RELEASE_QUERY,
                serde_json::json!({}),
                token,
                tenant_slug,
            )
            .await
            .map_err(|graphql_err| combine_native_and_graphql_error(server_err, graphql_err))?;
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
    match build_history_native(limit, offset).await {
        Ok(history) => Ok(history),
        Err(server_err) => {
            let response: BuildHistoryResponse = request(
                BUILD_HISTORY_QUERY,
                BuildHistoryVariables { limit, offset },
                token,
                tenant_slug,
            )
            .await
            .map_err(|graphql_err| combine_native_and_graphql_error(server_err, graphql_err))?;
            Ok(response.build_history)
        }
    }
}

#[server(prefix = "/api/fn", endpoint = "admin/list-tenant-modules")]
async fn list_tenant_modules_native() -> Result<Vec<TenantModule>, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use rustok_tenant::TenantService;

        let (app_ctx, _auth, tenant) = modules_server_context().await?;

        TenantService::new(app_ctx.db.clone())
            .list_tenant_modules(tenant.id)
            .await
            .map(|modules| {
                let mut modules = modules
                    .into_iter()
                    .map(|module| TenantModule {
                        module_slug: module.module_slug,
                        enabled: module.enabled,
                        settings: module.settings.to_string(),
                    })
                    .collect::<Vec<_>>();
                modules.sort_by(|left, right| left.module_slug.cmp(&right.module_slug));
                modules
            })
            .map_err(|err| server_error(err.to_string()))
    }
    #[cfg(not(feature = "ssr"))]
    {
        Err(ServerFnError::new(
            "admin/list-tenant-modules requires the `ssr` feature",
        ))
    }
}

#[server(prefix = "/api/fn", endpoint = "admin/active-build")]
async fn active_build_native() -> Result<Option<BuildJob>, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use sea_orm::{ConnectionTrait, DbBackend, Statement};

        let (app_ctx, _auth, _tenant) = modules_server_context().await?;
        let backend = app_ctx.db.get_database_backend();
        let statement = match backend {
            DbBackend::Sqlite => Statement::from_string(
                DbBackend::Sqlite,
                r#"
                SELECT
                    id,
                    status,
                    stage,
                    progress,
                    profile,
                    manifest_ref,
                    manifest_hash,
                    modules_delta,
                    requested_by,
                    reason,
                    release_id,
                    logs_url,
                    error_message,
                    started_at,
                    created_at,
                    updated_at,
                    finished_at
                FROM builds
                WHERE status IN ('queued', 'running')
                ORDER BY created_at DESC
                LIMIT 1
                "#,
            ),
            _ => Statement::from_string(
                DbBackend::Postgres,
                r#"
                SELECT
                    id,
                    status,
                    stage,
                    progress,
                    profile,
                    manifest_ref,
                    manifest_hash,
                    modules_delta,
                    requested_by,
                    reason,
                    release_id,
                    logs_url,
                    error_message,
                    started_at,
                    created_at,
                    updated_at,
                    finished_at
                FROM builds
                WHERE status IN ('queued', 'running')
                ORDER BY created_at DESC
                LIMIT 1
                "#,
            ),
        };

        app_ctx
            .db
            .query_one(statement)
            .await
            .map_err(|err| server_error(err.to_string()))?
            .map(map_build_job_row)
            .transpose()
    }
    #[cfg(not(feature = "ssr"))]
    {
        Err(ServerFnError::new(
            "admin/active-build requires the `ssr` feature",
        ))
    }
}

#[server(prefix = "/api/fn", endpoint = "admin/active-release")]
async fn active_release_native() -> Result<Option<ReleaseInfo>, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use sea_orm::{ConnectionTrait, DbBackend, Statement};

        let (app_ctx, _auth, _tenant) = modules_server_context().await?;
        let backend = app_ctx.db.get_database_backend();
        let statement = match backend {
            DbBackend::Sqlite => Statement::from_string(
                DbBackend::Sqlite,
                r#"
                SELECT
                    id,
                    build_id,
                    status,
                    environment,
                    manifest_hash,
                    modules,
                    previous_release_id,
                    deployed_at,
                    rolled_back_at,
                    created_at,
                    updated_at
                FROM releases
                WHERE status = 'active'
                ORDER BY updated_at DESC
                LIMIT 1
                "#,
            ),
            _ => Statement::from_string(
                DbBackend::Postgres,
                r#"
                SELECT
                    id,
                    build_id,
                    status,
                    environment,
                    manifest_hash,
                    modules,
                    previous_release_id,
                    deployed_at,
                    rolled_back_at,
                    created_at,
                    updated_at
                FROM releases
                WHERE status = 'active'
                ORDER BY updated_at DESC
                LIMIT 1
                "#,
            ),
        };

        app_ctx
            .db
            .query_one(statement)
            .await
            .map_err(|err| server_error(err.to_string()))?
            .map(map_release_info_row)
            .transpose()
    }
    #[cfg(not(feature = "ssr"))]
    {
        Err(ServerFnError::new(
            "admin/active-release requires the `ssr` feature",
        ))
    }
}

#[server(prefix = "/api/fn", endpoint = "admin/build-history")]
async fn build_history_native(limit: i32, offset: i32) -> Result<Vec<BuildJob>, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use sea_orm::{ConnectionTrait, DbBackend, Statement};

        let (app_ctx, _auth, _tenant) = modules_server_context().await?;
        let backend = app_ctx.db.get_database_backend();
        let limit = limit.clamp(1, 100);
        let offset = offset.max(0);
        let statement = match backend {
            DbBackend::Sqlite => Statement::from_sql_and_values(
                DbBackend::Sqlite,
                r#"
                SELECT
                    id,
                    status,
                    stage,
                    progress,
                    profile,
                    manifest_ref,
                    manifest_hash,
                    modules_delta,
                    requested_by,
                    reason,
                    release_id,
                    logs_url,
                    error_message,
                    started_at,
                    created_at,
                    updated_at,
                    finished_at
                FROM builds
                ORDER BY created_at DESC
                LIMIT ?1
                OFFSET ?2
                "#,
                vec![limit.into(), offset.into()],
            ),
            _ => Statement::from_sql_and_values(
                DbBackend::Postgres,
                r#"
                SELECT
                    id,
                    status,
                    stage,
                    progress,
                    profile,
                    manifest_ref,
                    manifest_hash,
                    modules_delta,
                    requested_by,
                    reason,
                    release_id,
                    logs_url,
                    error_message,
                    started_at,
                    created_at,
                    updated_at,
                    finished_at
                FROM builds
                ORDER BY created_at DESC
                LIMIT $1
                OFFSET $2
                "#,
                vec![limit.into(), offset.into()],
            ),
        };

        app_ctx
            .db
            .query_all(statement)
            .await
            .map_err(|err| server_error(err.to_string()))?
            .into_iter()
            .map(map_build_job_row)
            .collect()
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = (limit, offset);
        Err(ServerFnError::new(
            "admin/build-history requires the `ssr` feature",
        ))
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

pub async fn update_module_settings(
    module_slug: String,
    settings: String,
    token: Option<String>,
    tenant_slug: Option<String>,
) -> Result<TenantModule, ApiError> {
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
    let response: RollbackBuildResponse = request(
        ROLLBACK_BUILD_MUTATION,
        RollbackBuildVariables { build_id },
        token,
        tenant_slug,
    )
    .await?;
    Ok(response.rollback_build)
}
