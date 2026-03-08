use serde::{Deserialize, Serialize};

use crate::entities::module::{
    BuildJob, InstalledModule, MarketplaceModule, ModuleInfo, ReleaseInfo, ToggleModuleResult,
};
use crate::shared::api::{request, ApiError};

pub const ENABLED_MODULES_QUERY: &str = "query EnabledModules { enabledModules }";
pub const MODULE_REGISTRY_QUERY: &str =
    "query ModuleRegistry { moduleRegistry { moduleSlug name description version kind dependencies enabled ownership trustLevel recommendedAdminSurfaces showcaseAdminSurfaces } }";
pub const INSTALLED_MODULES_QUERY: &str =
    "query InstalledModules { installedModules { slug source crateName version required dependencies } }";
pub const MARKETPLACE_QUERY: &str =
    "query Marketplace($search: String, $category: String, $source: String, $trustLevel: String, $onlyCompatible: Boolean, $installedOnly: Boolean) { marketplace(search: $search, category: $category, source: $source, trustLevel: $trustLevel, onlyCompatible: $onlyCompatible, installedOnly: $installedOnly) { slug name latestVersion description source kind category crateName dependencies ownership trustLevel rustokMinVersion rustokMaxVersion publisher checksumSha256 signaturePresent versions { version changelog yanked publishedAt checksumSha256 signaturePresent } compatible recommendedAdminSurfaces showcaseAdminSurfaces installed installedVersion updateAvailable } }";
pub const MARKETPLACE_MODULE_QUERY: &str =
    "query MarketplaceModule($slug: String!) { marketplaceModule(slug: $slug) { slug name latestVersion description source kind category crateName dependencies ownership trustLevel rustokMinVersion rustokMaxVersion publisher checksumSha256 signaturePresent versions { version changelog yanked publishedAt checksumSha256 signaturePresent } compatible recommendedAdminSurfaces showcaseAdminSurfaces installed installedVersion updateAvailable } }";
pub const ACTIVE_BUILD_QUERY: &str =
    "query ActiveBuild { activeBuild { id status stage progress profile manifestRef manifestHash modulesDelta requestedBy reason releaseId logsUrl errorMessage startedAt createdAt updatedAt finishedAt } }";
pub const ACTIVE_RELEASE_QUERY: &str =
    "query ActiveRelease { activeRelease { id buildId status environment manifestHash modules previousReleaseId deployedAt rolledBackAt createdAt updatedAt } }";
pub const BUILD_HISTORY_QUERY: &str =
    "query BuildHistory($limit: Int!, $offset: Int!) { buildHistory(limit: $limit, offset: $offset) { id status stage progress profile manifestRef manifestHash modulesDelta requestedBy reason releaseId logsUrl errorMessage startedAt createdAt updatedAt finishedAt } }";
pub const TOGGLE_MODULE_MUTATION: &str =
    "mutation ToggleModule($moduleSlug: String!, $enabled: Boolean!) { toggleModule(moduleSlug: $moduleSlug, enabled: $enabled) { moduleSlug enabled settings } }";
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

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ToggleModuleResponse {
    #[serde(rename = "toggleModule")]
    pub toggle_module: ToggleModuleResult,
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

pub async fn fetch_enabled_modules(
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
    let response: ModuleRegistryResponse = request(
        MODULE_REGISTRY_QUERY,
        serde_json::json!({}),
        token,
        tenant_slug,
    )
    .await?;
    Ok(response.module_registry)
}

pub async fn fetch_installed_modules(
    token: Option<String>,
    tenant_slug: Option<String>,
) -> Result<Vec<InstalledModule>, ApiError> {
    let response: InstalledModulesResponse = request(
        INSTALLED_MODULES_QUERY,
        serde_json::json!({}),
        token,
        tenant_slug,
    )
    .await?;
    Ok(response.installed_modules)
}

pub async fn fetch_marketplace_modules(
    token: Option<String>,
    tenant_slug: Option<String>,
    search: Option<String>,
    category: Option<String>,
    source: Option<String>,
    trust_level: Option<String>,
    only_compatible: Option<bool>,
    installed_only: Option<bool>,
) -> Result<Vec<MarketplaceModule>, ApiError> {
    let response: MarketplaceResponse = request(
        MARKETPLACE_QUERY,
        MarketplaceVariables {
            search,
            category,
            source,
            trust_level,
            only_compatible,
            installed_only,
        },
        token,
        tenant_slug,
    )
    .await?;
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
    let response: ActiveBuildResponse = request(
        ACTIVE_BUILD_QUERY,
        serde_json::json!({}),
        token,
        tenant_slug,
    )
    .await?;
    Ok(response.active_build)
}

pub async fn fetch_active_release(
    token: Option<String>,
    tenant_slug: Option<String>,
) -> Result<Option<ReleaseInfo>, ApiError> {
    let response: ActiveReleaseResponse = request(
        ACTIVE_RELEASE_QUERY,
        serde_json::json!({}),
        token,
        tenant_slug,
    )
    .await?;
    Ok(response.active_release)
}

pub async fn fetch_build_history(
    token: Option<String>,
    tenant_slug: Option<String>,
    limit: i32,
    offset: i32,
) -> Result<Vec<BuildJob>, ApiError> {
    let response: BuildHistoryResponse = request(
        BUILD_HISTORY_QUERY,
        BuildHistoryVariables { limit, offset },
        token,
        tenant_slug,
    )
    .await?;
    Ok(response.build_history)
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
