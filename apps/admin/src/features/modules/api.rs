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
    "query Marketplace($search: String, $category: String, $tag: String, $source: String, $trustLevel: String, $onlyCompatible: Boolean, $installedOnly: Boolean) { marketplace(search: $search, category: $category, tag: $tag, source: $source, trustLevel: $trustLevel, onlyCompatible: $onlyCompatible, installedOnly: $installedOnly) { slug name latestVersion description source kind category tags iconUrl bannerUrl screenshots crateName dependencies ownership trustLevel rustokMinVersion rustokMaxVersion publisher checksumSha256 signaturePresent versions { version changelog yanked publishedAt checksumSha256 signaturePresent } compatible recommendedAdminSurfaces showcaseAdminSurfaces settingsSchema { key type required defaultValue description min max options objectKeys itemType shape } installed installedVersion updateAvailable } }";
pub const MARKETPLACE_MODULE_QUERY: &str =
    "query MarketplaceModule($slug: String!) { marketplaceModule(slug: $slug) { slug name latestVersion description source kind category tags iconUrl bannerUrl screenshots crateName dependencies ownership trustLevel rustokMinVersion rustokMaxVersion publisher checksumSha256 signaturePresent versions { version changelog yanked publishedAt checksumSha256 signaturePresent } registryLifecycle { latestRequest { id status requestedBy approvedBy rejectedBy rejectionReason warnings errors createdAt updatedAt publishedAt } latestRelease { version status publisher checksumSha256 publishedAt yankedReason yankedBy yankedAt } } compatible recommendedAdminSurfaces showcaseAdminSurfaces settingsSchema { key type required defaultValue description min max options objectKeys itemType shape } installed installedVersion updateAvailable } }";
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
    pub tag: Option<String>,
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
#[derive(Debug, Clone, Deserialize, Serialize, Default)]
struct RuntimeModulesManifest {
    #[serde(default)]
    schema: u32,
    #[serde(default)]
    app: String,
    #[serde(default)]
    build: RuntimeBuildConfig,
    #[serde(default)]
    modules: std::collections::HashMap<String, RuntimeManifestModuleSpec>,
    #[serde(default)]
    settings: RuntimeSettingsManifest,
}

#[cfg(feature = "ssr")]
#[derive(Debug, Clone, Deserialize, Serialize, Default)]
struct RuntimeBuildConfig {
    #[serde(default)]
    target: String,
    #[serde(default)]
    profile: String,
    #[serde(default)]
    server: RuntimeServerBuildConfig,
    #[serde(default)]
    admin: RuntimeAdminBuildConfig,
    #[serde(default)]
    storefront: Vec<RuntimeStorefrontBuildConfig>,
}

#[cfg(feature = "ssr")]
#[derive(Debug, Clone, Deserialize, Serialize, Default)]
struct RuntimeServerBuildConfig {
    #[serde(default)]
    embed_admin: bool,
    #[serde(default)]
    embed_storefront: bool,
}

#[cfg(feature = "ssr")]
#[derive(Debug, Clone, Deserialize, Serialize, Default)]
struct RuntimeAdminBuildConfig {
    #[serde(default)]
    stack: String,
    #[serde(default)]
    public_url: String,
    #[serde(default)]
    redirect_uris: Vec<String>,
}

#[cfg(feature = "ssr")]
#[derive(Debug, Clone, Deserialize, Serialize, Default)]
struct RuntimeStorefrontBuildConfig {
    #[serde(default)]
    id: String,
    #[serde(default)]
    stack: String,
    #[serde(default)]
    public_url: String,
    #[serde(default)]
    redirect_uris: Vec<String>,
}

#[cfg(feature = "ssr")]
#[derive(Debug, Clone, Deserialize, Serialize, Default)]
struct RuntimeSettingsManifest {
    #[serde(default)]
    default_enabled: Vec<String>,
}

#[cfg(feature = "ssr")]
#[derive(Debug, Clone, Deserialize, Serialize, Default)]
struct RuntimeManifestModuleSpec {
    source: String,
    #[serde(rename = "crate", default)]
    crate_name: String,
    #[serde(default)]
    path: Option<String>,
    #[serde(default)]
    version: Option<String>,
    #[serde(default)]
    git: Option<String>,
    #[serde(default)]
    rev: Option<String>,
    #[serde(default)]
    required: bool,
    #[serde(default)]
    depends_on: Vec<String>,
}

#[cfg(feature = "ssr")]
#[derive(Debug, Deserialize, Default)]
struct RuntimeModulePackageManifest {
    module: RuntimeModuleMetadata,
    #[serde(default)]
    marketplace: RuntimeModuleMarketplaceMetadata,
    #[serde(default)]
    dependencies: std::collections::BTreeMap<String, RuntimeModuleDependencySpec>,
    #[serde(default)]
    settings: std::collections::BTreeMap<String, RuntimeModuleSettingSpec>,
}

#[cfg(feature = "ssr")]
#[derive(Debug, Deserialize, Default)]
struct RuntimeModuleMetadata {
    slug: String,
    name: String,
    version: String,
    #[serde(default)]
    description: String,
    #[serde(default = "default_module_ownership")]
    ownership: String,
    #[serde(default = "default_module_trust_level")]
    trust_level: String,
    #[serde(default)]
    recommended_admin_surfaces: Vec<String>,
    #[serde(default)]
    showcase_admin_surfaces: Vec<String>,
    #[serde(default)]
    rustok_min_version: Option<String>,
    #[serde(default)]
    rustok_max_version: Option<String>,
}

#[cfg(feature = "ssr")]
#[derive(Debug, Deserialize, Default)]
struct RuntimeModuleMarketplaceMetadata {
    #[serde(default)]
    category: Option<String>,
    #[serde(default)]
    tags: Vec<String>,
    #[serde(default)]
    icon_url: Option<String>,
    #[serde(default)]
    banner_url: Option<String>,
    #[serde(default)]
    screenshots: Vec<String>,
    #[serde(default)]
    publisher: Option<String>,
    #[serde(default)]
    checksum_sha256: Option<String>,
    #[serde(default)]
    signature: Option<String>,
}

#[cfg(feature = "ssr")]
#[derive(Debug, Deserialize, Default)]
struct RuntimeModuleDependencySpec {
    #[allow(dead_code)]
    #[serde(default)]
    version_req: Option<String>,
}

#[cfg(feature = "ssr")]
#[derive(Debug, Deserialize, Serialize, Default)]
struct RuntimeModuleSettingSpec {
    #[serde(rename = "type", default)]
    value_type: String,
    #[serde(default)]
    required: bool,
    #[serde(default)]
    default: Option<serde_json::Value>,
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    min: Option<f64>,
    #[serde(default)]
    max: Option<f64>,
    #[serde(default)]
    options: Vec<serde_json::Value>,
    #[serde(default)]
    object_keys: Vec<String>,
    #[serde(default)]
    item_type: Option<String>,
    #[serde(default)]
    properties: std::collections::BTreeMap<String, RuntimeModuleSettingSpec>,
    #[serde(default)]
    items: Option<Box<RuntimeModuleSettingSpec>>,
}

#[cfg(feature = "ssr")]
fn runtime_setting_shape(spec: &RuntimeModuleSettingSpec) -> Option<serde_json::Value> {
    let mut shape = serde_json::Map::new();

    if !spec.properties.is_empty() {
        let properties = spec
            .properties
            .iter()
            .map(|(key, property_spec)| {
                (
                    key.clone(),
                    serde_json::to_value(property_spec)
                        .expect("runtime setting property schema should serialize"),
                )
            })
            .collect::<serde_json::Map<String, serde_json::Value>>();
        shape.insert(
            "properties".to_string(),
            serde_json::Value::Object(properties),
        );
    }

    if let Some(items) = &spec.items {
        shape.insert(
            "items".to_string(),
            serde_json::to_value(items.as_ref())
                .expect("runtime setting item schema should serialize"),
        );
    }

    (!shape.is_empty()).then_some(serde_json::Value::Object(shape))
}

#[cfg(feature = "ssr")]
fn runtime_setting_object_keys(spec: &RuntimeModuleSettingSpec) -> Vec<String> {
    if spec.properties.is_empty() {
        spec.object_keys.clone()
    } else {
        spec.properties.keys().cloned().collect()
    }
}

#[cfg(feature = "ssr")]
fn runtime_setting_item_type(spec: &RuntimeModuleSettingSpec) -> Option<String> {
    spec.items
        .as_deref()
        .map(|item| item.value_type.trim().to_string())
        .filter(|value| !value.is_empty())
        .or_else(|| spec.item_type.clone())
}

#[cfg(feature = "ssr")]
#[derive(Debug, Deserialize, Default)]
struct RuntimeCargoManifest {
    package: RuntimeCargoPackage,
}

#[cfg(feature = "ssr")]
#[derive(Debug, Deserialize, Default)]
struct RuntimeCargoPackage {
    name: String,
}

#[cfg(feature = "ssr")]
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
enum RuntimeFrontendBuildTool {
    Cargo,
    Trunk,
}

#[cfg(feature = "ssr")]
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
enum RuntimeFrontendArtifactKind {
    File,
    Directory,
}

#[cfg(feature = "ssr")]
#[derive(Debug, Clone, Serialize)]
struct RuntimeFrontendBuildPlan {
    surface: String,
    tool: RuntimeFrontendBuildTool,
    package: String,
    workspace_path: String,
    profile: String,
    target: Option<String>,
    artifact_path: String,
    artifact_kind: RuntimeFrontendArtifactKind,
    command: String,
}

#[cfg(feature = "ssr")]
#[derive(Debug, Clone, Serialize)]
struct RuntimeBuildExecutionPlan {
    cargo_package: String,
    cargo_profile: String,
    cargo_target: Option<String>,
    cargo_features: Vec<String>,
    cargo_command: String,
    admin_build: Option<RuntimeFrontendBuildPlan>,
    storefront_build: Option<RuntimeFrontendBuildPlan>,
}

#[cfg(feature = "ssr")]
fn server_error(message: impl Into<String>) -> ServerFnError {
    ServerFnError::ServerError(message.into())
}

#[cfg(feature = "ssr")]
fn default_module_ownership() -> String {
    "third_party".to_string()
}

#[cfg(feature = "ssr")]
fn default_module_trust_level() -> String {
    "unverified".to_string()
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
fn save_runtime_modules_manifest(manifest: &RuntimeModulesManifest) -> Result<(), ServerFnError> {
    let path = runtime_modules_manifest_path();
    let raw = toml::to_string_pretty(manifest)
        .map_err(|err| server_error(format!("failed to serialize {}: {err}", path.display())))?;
    std::fs::write(&path, raw)
        .map_err(|err| server_error(format!("failed to write {}: {err}", path.display())))
}

#[cfg(feature = "ssr")]
fn runtime_workspace_root() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../")
}

#[cfg(feature = "ssr")]
fn runtime_builtin_default_enabled() -> std::collections::HashSet<&'static str> {
    std::collections::HashSet::from([
        "content",
        "cart",
        "customer",
        "product",
        "pricing",
        "inventory",
        "order",
        "payment",
        "fulfillment",
        "commerce",
        "pages",
    ])
}

#[cfg(feature = "ssr")]
fn humanize_module_slug(slug: &str) -> String {
    slug.split('-')
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

#[cfg(feature = "ssr")]
fn fallback_module_category(slug: &str) -> &'static str {
    match slug {
        "content" | "blog" | "forum" | "pages" => "content",
        "commerce" | "pricing" | "product" | "inventory" => "commerce",
        "tenant" | "rbac" | "index" | "outbox" => "platform",
        _ => "extensions",
    }
}

#[cfg(feature = "ssr")]
fn runtime_setting_fields(
    schema: &std::collections::BTreeMap<String, RuntimeModuleSettingSpec>,
) -> Vec<crate::entities::module::ModuleSettingField> {
    schema
        .iter()
        .map(|(key, spec)| crate::entities::module::ModuleSettingField {
            key: key.clone(),
            value_type: spec.value_type.clone(),
            required: spec.required,
            default_value: spec.default.clone(),
            description: spec.description.clone(),
            min: spec.min,
            max: spec.max,
            options: spec.options.clone(),
            object_keys: runtime_setting_object_keys(spec),
            item_type: runtime_setting_item_type(spec),
            shape: runtime_setting_shape(spec),
        })
        .collect()
}

#[cfg(feature = "ssr")]
fn load_toml_file<T: serde::de::DeserializeOwned>(
    path: &std::path::Path,
) -> Result<T, ServerFnError> {
    let raw = std::fs::read_to_string(path)
        .map_err(|err| server_error(format!("failed to read {}: {err}", path.display())))?;
    toml::from_str(&raw)
        .map_err(|err| server_error(format!("failed to parse {}: {err}", path.display())))
}

#[cfg(feature = "ssr")]
fn runtime_binary_output_dir_name(profile: &str) -> &str {
    if profile == "release" {
        "release"
    } else {
        profile
    }
}

#[cfg(feature = "ssr")]
fn runtime_executable_suffix(target: Option<&str>) -> &'static str {
    match target {
        Some(value) if value.contains("windows") => "exe",
        Some(_) => "",
        None => std::env::consts::EXE_EXTENSION,
    }
}

#[cfg(feature = "ssr")]
fn runtime_binary_file_name(package: &str, target: Option<&str>) -> String {
    let suffix = runtime_executable_suffix(target);
    if suffix.is_empty() {
        package.to_string()
    } else {
        format!("{package}.{suffix}")
    }
}

#[cfg(feature = "ssr")]
fn runtime_admin_frontend_build_plan(
    manifest: &RuntimeModulesManifest,
    cargo_profile: &str,
) -> Option<RuntimeFrontendBuildPlan> {
    let admin_stack = manifest.build.admin.stack.trim().to_ascii_lowercase();
    let requires_leptos_admin = manifest.build.server.embed_admin || admin_stack == "leptos";

    requires_leptos_admin.then(|| {
        let mut command_parts = vec!["trunk".to_string(), "build".to_string()];
        if cargo_profile == "release" {
            command_parts.push("--release".to_string());
        }

        RuntimeFrontendBuildPlan {
            surface: "admin".to_string(),
            tool: RuntimeFrontendBuildTool::Trunk,
            package: "rustok-admin".to_string(),
            workspace_path: "apps/admin".to_string(),
            profile: cargo_profile.to_string(),
            target: None,
            artifact_path: "apps/admin/dist".to_string(),
            artifact_kind: RuntimeFrontendArtifactKind::Directory,
            command: command_parts.join(" "),
        }
    })
}

#[cfg(feature = "ssr")]
fn runtime_storefront_frontend_build_plan(
    manifest: &RuntimeModulesManifest,
    cargo_profile: &str,
    cargo_target: Option<&str>,
) -> Option<RuntimeFrontendBuildPlan> {
    let has_leptos_storefront = manifest.build.server.embed_storefront
        || manifest
            .build
            .storefront
            .iter()
            .any(|storefront| storefront.stack.trim().eq_ignore_ascii_case("leptos"));

    has_leptos_storefront.then(|| {
        let mut command_parts = vec![
            "cargo".to_string(),
            "build".to_string(),
            "-p".to_string(),
            "rustok-storefront".to_string(),
        ];
        if cargo_profile == "release" {
            command_parts.push("--release".to_string());
        } else {
            command_parts.push("--profile".to_string());
            command_parts.push(cargo_profile.to_string());
        }
        if let Some(target) = cargo_target {
            command_parts.push("--target".to_string());
            command_parts.push(target.to_string());
        }

        let mut artifact_path = String::from("target/");
        if let Some(target) = cargo_target {
            artifact_path.push_str(target);
            artifact_path.push('/');
        }
        artifact_path.push_str(runtime_binary_output_dir_name(cargo_profile));
        artifact_path.push('/');
        artifact_path.push_str(&runtime_binary_file_name("rustok-storefront", cargo_target));

        RuntimeFrontendBuildPlan {
            surface: "storefront".to_string(),
            tool: RuntimeFrontendBuildTool::Cargo,
            package: "rustok-storefront".to_string(),
            workspace_path: ".".to_string(),
            profile: cargo_profile.to_string(),
            target: cargo_target.map(ToString::to_string),
            artifact_path,
            artifact_kind: RuntimeFrontendArtifactKind::File,
            command: command_parts.join(" "),
        }
    })
}

#[cfg(feature = "ssr")]
fn runtime_build_execution_plan(manifest: &RuntimeModulesManifest) -> RuntimeBuildExecutionPlan {
    let cargo_package = if manifest.app.trim().is_empty() {
        "rustok-server".to_string()
    } else {
        manifest.app.trim().to_string()
    };

    let cargo_profile = if manifest.build.profile.trim().is_empty() {
        "release".to_string()
    } else {
        manifest.build.profile.trim().to_string()
    };

    let cargo_target = (!manifest.build.target.trim().is_empty())
        .then(|| manifest.build.target.trim().to_string());

    let mut cargo_features = Vec::new();
    if manifest.build.server.embed_admin {
        cargo_features.push("embed-admin".to_string());
    }
    if manifest.build.server.embed_storefront {
        cargo_features.push("embed-storefront".to_string());
    }

    let mut command_parts = vec![
        "cargo".to_string(),
        "build".to_string(),
        "-p".to_string(),
        cargo_package.clone(),
    ];
    if cargo_profile == "release" {
        command_parts.push("--release".to_string());
    } else {
        command_parts.push("--profile".to_string());
        command_parts.push(cargo_profile.clone());
    }
    if let Some(target) = &cargo_target {
        command_parts.push("--target".to_string());
        command_parts.push(target.clone());
    }
    if !cargo_features.is_empty() {
        command_parts.push("--features".to_string());
        command_parts.push(cargo_features.join(","));
    }

    RuntimeBuildExecutionPlan {
        cargo_package,
        cargo_profile: cargo_profile.clone(),
        cargo_target: cargo_target.clone(),
        cargo_features,
        cargo_command: command_parts.join(" "),
        admin_build: runtime_admin_frontend_build_plan(manifest, &cargo_profile),
        storefront_build: runtime_storefront_frontend_build_plan(
            manifest,
            &cargo_profile,
            cargo_target.as_deref(),
        ),
    }
}

#[cfg(feature = "ssr")]
fn runtime_deployment_profile(manifest: &RuntimeModulesManifest) -> String {
    match (
        manifest.build.server.embed_admin,
        manifest.build.server.embed_storefront,
    ) {
        (true, true) => "monolith".to_string(),
        (true, false) => "server-with-admin".to_string(),
        (false, true) => "server-with-storefront".to_string(),
        (false, false) => "headless-api".to_string(),
    }
}

#[cfg(feature = "ssr")]
fn runtime_manifest_hash(manifest: &RuntimeModulesManifest) -> String {
    use std::collections::BTreeMap;
    use std::hash::{Hash, Hasher};

    let sorted = manifest
        .modules
        .iter()
        .map(|(slug, spec)| {
            (
                slug.clone(),
                (
                    spec.source.clone(),
                    spec.crate_name.clone(),
                    spec.version.clone(),
                    spec.git.clone(),
                    spec.rev.clone(),
                    spec.path.clone(),
                    spec.required,
                    spec.depends_on.clone(),
                ),
            )
        })
        .collect::<BTreeMap<_, _>>();
    let serialized = serde_json::to_string(&sorted).unwrap_or_default();
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    serialized.hash(&mut hasher);
    format!("{:016x}", hasher.finish())
}

#[cfg(feature = "ssr")]
fn runtime_modules_delta_json(
    manifest: &RuntimeModulesManifest,
    summary: String,
) -> serde_json::Value {
    let modules = manifest
        .modules
        .iter()
        .map(|(slug, spec)| {
            (
                slug.clone(),
                serde_json::json!({
                    "source": spec.source,
                    "crate_name": spec.crate_name,
                    "version": spec.version,
                    "git": spec.git,
                    "rev": spec.rev,
                    "path": spec.path,
                }),
            )
        })
        .collect::<serde_json::Map<String, serde_json::Value>>();

    serde_json::json!({
        "summary": summary,
        "modules": modules,
        "execution_plan": runtime_build_execution_plan(manifest),
    })
}

#[cfg(feature = "ssr")]
fn runtime_build_job_insert_sql(backend: sea_orm::DbBackend) -> &'static str {
    match backend {
        sea_orm::DbBackend::Sqlite => {
            r#"
            INSERT INTO builds (
                id, status, stage, progress, profile, manifest_ref, manifest_hash, modules_delta,
                requested_by, reason, release_id, logs_url, error_message, started_at, finished_at,
                created_at, updated_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, NULL, NULL, NULL, NULL, NULL, ?11, ?12)
            "#
        }
        _ => {
            r#"
            INSERT INTO builds (
                id, status, stage, progress, profile, manifest_ref, manifest_hash, modules_delta,
                requested_by, reason, release_id, logs_url, error_message, started_at, finished_at,
                created_at, updated_at
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, NULL, NULL, NULL, NULL, NULL, $11, $12)
            "#
        }
    }
}

#[cfg(feature = "ssr")]
fn runtime_build_job_select_sql(backend: sea_orm::DbBackend) -> &'static str {
    match backend {
        sea_orm::DbBackend::Sqlite => {
            r#"
            SELECT
                id, status, stage, progress, profile, manifest_ref, manifest_hash, modules_delta,
                requested_by, reason, release_id, logs_url, error_message, started_at,
                created_at, updated_at, finished_at
            FROM builds
            WHERE id = ?1
            LIMIT 1
            "#
        }
        _ => {
            r#"
            SELECT
                id, status, stage, progress, profile, manifest_ref, manifest_hash, modules_delta,
                requested_by, reason, release_id, logs_url, error_message, started_at,
                created_at, updated_at, finished_at
            FROM builds
            WHERE id = $1
            LIMIT 1
            "#
        }
    }
}

#[cfg(feature = "ssr")]
fn runtime_module_roots(
    manifest: &RuntimeModulesManifest,
) -> Result<Vec<std::path::PathBuf>, ServerFnError> {
    let workspace_root = runtime_workspace_root();
    let crates_root = workspace_root.join("crates");
    let mut roots = std::collections::BTreeSet::new();

    if crates_root.exists() {
        for entry in std::fs::read_dir(&crates_root).map_err(|err| {
            server_error(format!("failed to read {}: {err}", crates_root.display()))
        })? {
            let entry = entry.map_err(|err| server_error(err.to_string()))?;
            let path = entry.path();
            if path.join("rustok-module.toml").exists() {
                roots.insert(path);
            }
        }
    }

    for spec in manifest.modules.values() {
        if let Some(path) = spec.path.as_ref() {
            let module_root = workspace_root.join(path);
            if module_root.join("rustok-module.toml").exists() {
                roots.insert(module_root);
            }
        }
    }

    Ok(roots.into_iter().collect())
}

#[cfg(feature = "ssr")]
fn load_runtime_marketplace_modules(
    registry: &rustok_core::ModuleRegistry,
) -> Result<Vec<MarketplaceModule>, ServerFnError> {
    let manifest = load_runtime_modules_manifest()?;
    let module_roots = runtime_module_roots(&manifest)?;
    let mut installed_by_slug = manifest.modules;
    let mut modules = Vec::new();

    for module_root in module_roots {
        let package_manifest: RuntimeModulePackageManifest =
            load_toml_file(&module_root.join("rustok-module.toml"))?;
        let cargo_manifest: RuntimeCargoManifest = load_toml_file(&module_root.join("Cargo.toml"))?;
        let slug = package_manifest.module.slug.clone();
        let installed_entry = installed_by_slug.remove(&slug);
        let runtime_module = registry.get(&slug);
        let latest_version = runtime_module
            .map(|module| module.version().to_string())
            .unwrap_or_else(|| package_manifest.module.version.clone());
        let installed_version = installed_entry
            .as_ref()
            .and_then(|entry| entry.version.clone());
        let dependencies = runtime_module
            .map(|module| {
                module
                    .dependencies()
                    .iter()
                    .map(|dependency| dependency.to_string())
                    .collect::<Vec<_>>()
            })
            .unwrap_or_else(|| {
                if package_manifest.dependencies.is_empty() {
                    installed_entry
                        .as_ref()
                        .map(|entry| entry.depends_on.clone())
                        .unwrap_or_default()
                } else {
                    package_manifest.dependencies.keys().cloned().collect()
                }
            });

        modules.push(MarketplaceModule {
            slug: slug.clone(),
            name: runtime_module
                .map(|module| module.name().to_string())
                .unwrap_or_else(|| package_manifest.module.name.clone()),
            latest_version: latest_version.clone(),
            description: runtime_module
                .map(|module| module.description().to_string())
                .filter(|value| !value.is_empty())
                .unwrap_or_else(|| package_manifest.module.description.clone()),
            source: installed_entry
                .as_ref()
                .map(|entry| entry.source.clone())
                .unwrap_or_else(|| "path".to_string()),
            kind: if registry.is_core(&slug)
                || installed_entry.as_ref().is_some_and(|entry| entry.required)
            {
                "core".to_string()
            } else {
                "optional".to_string()
            },
            category: package_manifest
                .marketplace
                .category
                .clone()
                .unwrap_or_else(|| fallback_module_category(&slug).to_string()),
            tags: package_manifest.marketplace.tags.clone(),
            icon_url: package_manifest.marketplace.icon_url.clone(),
            banner_url: package_manifest.marketplace.banner_url.clone(),
            screenshots: package_manifest.marketplace.screenshots.clone(),
            crate_name: installed_entry
                .as_ref()
                .map(|entry| entry.crate_name.clone())
                .filter(|value| !value.is_empty())
                .unwrap_or_else(|| cargo_manifest.package.name.clone()),
            dependencies,
            ownership: package_manifest.module.ownership.clone(),
            trust_level: package_manifest.module.trust_level.clone(),
            rustok_min_version: package_manifest.module.rustok_min_version.clone(),
            rustok_max_version: package_manifest.module.rustok_max_version.clone(),
            publisher: package_manifest.marketplace.publisher.clone(),
            checksum_sha256: package_manifest.marketplace.checksum_sha256.clone(),
            signature_present: package_manifest.marketplace.signature.is_some(),
            versions: vec![crate::entities::module::model::MarketplaceModuleVersion {
                version: latest_version.clone(),
                changelog: None,
                yanked: false,
                published_at: None,
                checksum_sha256: package_manifest.marketplace.checksum_sha256.clone(),
                signature_present: package_manifest.marketplace.signature.is_some(),
            }],
            registry_lifecycle: None,
            compatible: true,
            recommended_admin_surfaces: package_manifest.module.recommended_admin_surfaces.clone(),
            showcase_admin_surfaces: package_manifest.module.showcase_admin_surfaces.clone(),
            settings_schema: runtime_setting_fields(&package_manifest.settings),
            installed: installed_entry.is_some(),
            installed_version: installed_version.clone(),
            update_available: installed_version
                .as_ref()
                .is_some_and(|version| version != &latest_version),
        });
    }

    for (slug, entry) in installed_by_slug {
        let latest_version = registry
            .get(&slug)
            .map(|module| module.version().to_string())
            .or(entry.version.clone())
            .unwrap_or_else(|| "workspace".to_string());
        modules.push(MarketplaceModule {
            slug: slug.clone(),
            name: registry
                .get(&slug)
                .map(|module| module.name().to_string())
                .unwrap_or_else(|| humanize_module_slug(&slug)),
            latest_version: latest_version.clone(),
            description: registry
                .get(&slug)
                .map(|module| module.description().to_string())
                .unwrap_or_else(|| format!("{} module", humanize_module_slug(&slug))),
            source: entry.source,
            kind: if registry.is_core(&slug) || entry.required {
                "core".to_string()
            } else {
                "optional".to_string()
            },
            category: fallback_module_category(&slug).to_string(),
            tags: Vec::new(),
            icon_url: None,
            banner_url: None,
            screenshots: Vec::new(),
            crate_name: if entry.crate_name.is_empty() {
                format!("rustok-{slug}")
            } else {
                entry.crate_name
            },
            dependencies: if entry.depends_on.is_empty() {
                registry
                    .get(&slug)
                    .map(|module| {
                        module
                            .dependencies()
                            .iter()
                            .map(|dependency| dependency.to_string())
                            .collect()
                    })
                    .unwrap_or_default()
            } else {
                entry.depends_on
            },
            ownership: "third_party".to_string(),
            trust_level: "unverified".to_string(),
            rustok_min_version: None,
            rustok_max_version: None,
            publisher: None,
            checksum_sha256: None,
            signature_present: false,
            versions: vec![crate::entities::module::model::MarketplaceModuleVersion {
                version: latest_version.clone(),
                changelog: None,
                yanked: false,
                published_at: None,
                checksum_sha256: None,
                signature_present: false,
            }],
            registry_lifecycle: None,
            compatible: true,
            recommended_admin_surfaces: Vec::new(),
            showcase_admin_surfaces: Vec::new(),
            settings_schema: Vec::new(),
            installed: true,
            installed_version: entry.version.clone(),
            update_available: entry
                .version
                .as_ref()
                .is_some_and(|version| version != &latest_version),
        });
    }

    modules.sort_by(|left, right| left.slug.cmp(&right.slug));
    Ok(modules)
}

#[cfg(feature = "ssr")]
struct RuntimeModuleDescriptor {
    root: std::path::PathBuf,
    package_manifest: RuntimeModulePackageManifest,
    cargo_manifest: RuntimeCargoManifest,
}

#[cfg(feature = "ssr")]
fn load_runtime_module_package_manifest_by_slug(
    module_slug: &str,
) -> Result<Option<RuntimeModulePackageManifest>, ServerFnError> {
    let manifest = load_runtime_modules_manifest()?;
    for module_root in runtime_module_roots(&manifest)? {
        let package_manifest: RuntimeModulePackageManifest =
            load_toml_file(&module_root.join("rustok-module.toml"))?;
        if package_manifest.module.slug == module_slug {
            return Ok(Some(package_manifest));
        }
    }

    Ok(None)
}

#[cfg(feature = "ssr")]
fn load_runtime_module_descriptor_by_slug(
    module_slug: &str,
) -> Result<Option<RuntimeModuleDescriptor>, ServerFnError> {
    let manifest = load_runtime_modules_manifest()?;
    for module_root in runtime_module_roots(&manifest)? {
        let package_manifest: RuntimeModulePackageManifest =
            load_toml_file(&module_root.join("rustok-module.toml"))?;
        if package_manifest.module.slug == module_slug {
            let cargo_manifest: RuntimeCargoManifest =
                load_toml_file(&module_root.join("Cargo.toml"))?;
            return Ok(Some(RuntimeModuleDescriptor {
                root: module_root,
                package_manifest,
                cargo_manifest,
            }));
        }
    }

    Ok(None)
}

#[cfg(feature = "ssr")]
async fn save_manifest_and_enqueue_build(
    app_ctx: &loco_rs::app::AppContext,
    original_manifest: &RuntimeModulesManifest,
    manifest: &RuntimeModulesManifest,
    requested_by: &str,
    reason: String,
    summary: String,
) -> Result<BuildJob, ServerFnError> {
    use sea_orm::{ConnectionTrait, Statement};

    save_runtime_modules_manifest(manifest)?;

    let backend = app_ctx.db.get_database_backend();
    let build_id = rustok_core::generate_id();
    let now = chrono::Utc::now();
    let insert = Statement::from_sql_and_values(
        backend,
        runtime_build_job_insert_sql(backend),
        vec![
            build_id.into(),
            "queued".into(),
            "pending".into(),
            0.into(),
            runtime_deployment_profile(manifest).into(),
            runtime_modules_manifest_path().display().to_string().into(),
            runtime_manifest_hash(manifest).into(),
            runtime_modules_delta_json(manifest, summary).into(),
            requested_by.to_string().into(),
            reason.into(),
            now.into(),
            now.into(),
        ],
    );

    if let Err(err) = app_ctx.db.execute(insert).await {
        let _ = save_runtime_modules_manifest(original_manifest);
        return Err(server_error(format!(
            "failed to enqueue build after manifest update: {err}"
        )));
    }

    let select = Statement::from_sql_and_values(
        backend,
        runtime_build_job_select_sql(backend),
        vec![build_id.into()],
    );

    app_ctx
        .db
        .query_one(select)
        .await
        .map_err(|err| server_error(err.to_string()))?
        .map(map_build_job_row)
        .transpose()?
        .ok_or_else(|| server_error("build record missing after enqueue"))
}

#[cfg(feature = "ssr")]
fn runtime_setting_value_matches_type(value_type: &str, value: &serde_json::Value) -> bool {
    match value_type {
        "string" => value.is_string(),
        "integer" => {
            value.as_i64().is_some()
                || value.as_u64().is_some()
                || value
                    .as_f64()
                    .is_some_and(|number| number.fract().abs() < f64::EPSILON)
        }
        "number" => value.is_number(),
        "boolean" => value.is_boolean(),
        "object" => value.is_object(),
        "array" => value.is_array(),
        "json" | "any" => true,
        _ => false,
    }
}

#[cfg(feature = "ssr")]
fn validate_runtime_setting_value(
    module_slug: &str,
    key: &str,
    spec: &RuntimeModuleSettingSpec,
    value: &serde_json::Value,
) -> Result<(), ServerFnError> {
    let value_type = spec.value_type.trim();
    if !runtime_setting_value_matches_type(value_type, value) {
        return Err(server_error(format!(
            "Module '{module_slug}' setting '{key}' is invalid: expected {value_type}"
        )));
    }

    if !spec.options.is_empty() && !spec.options.iter().any(|option| option == value) {
        let allowed = spec
            .options
            .iter()
            .map(serde_json::Value::to_string)
            .collect::<Vec<_>>()
            .join(", ");
        return Err(server_error(format!(
            "Module '{module_slug}' setting '{key}' is invalid: must be one of: {allowed}"
        )));
    }

    match value_type {
        "integer" | "number" => {
            let numeric_value = value.as_f64().ok_or_else(|| {
                server_error(format!(
                    "Module '{module_slug}' setting '{key}' is invalid: expected {value_type}"
                ))
            })?;

            if let Some(min) = spec.min {
                if numeric_value < min {
                    return Err(server_error(format!(
                        "Module '{module_slug}' setting '{key}' is invalid: must be >= {min}"
                    )));
                }
            }
            if let Some(max) = spec.max {
                if numeric_value > max {
                    return Err(server_error(format!(
                        "Module '{module_slug}' setting '{key}' is invalid: must be <= {max}"
                    )));
                }
            }
        }
        "string" => {
            let length = value
                .as_str()
                .map(|item| item.chars().count())
                .unwrap_or_default() as f64;
            if let Some(min) = spec.min {
                if length < min {
                    return Err(server_error(format!(
                        "Module '{module_slug}' setting '{key}' is invalid: length must be >= {min}"
                    )));
                }
            }
            if let Some(max) = spec.max {
                if length > max {
                    return Err(server_error(format!(
                        "Module '{module_slug}' setting '{key}' is invalid: length must be <= {max}"
                    )));
                }
            }
        }
        "array" => {
            let length = value
                .as_array()
                .map(|items| items.len())
                .unwrap_or_default() as f64;
            if let Some(min) = spec.min {
                if length < min {
                    return Err(server_error(format!(
                        "Module '{module_slug}' setting '{key}' is invalid: length must be >= {min}"
                    )));
                }
            }
            if let Some(max) = spec.max {
                if length > max {
                    return Err(server_error(format!(
                        "Module '{module_slug}' setting '{key}' is invalid: length must be <= {max}"
                    )));
                }
            }
        }
        _ => {}
    }

    Ok(())
}

#[cfg(feature = "ssr")]
fn normalize_runtime_module_settings(
    module_slug: &str,
    schema: &std::collections::BTreeMap<String, RuntimeModuleSettingSpec>,
    settings: serde_json::Value,
) -> Result<serde_json::Value, ServerFnError> {
    let mut settings_object = settings.as_object().cloned().ok_or_else(|| {
        server_error(format!(
            "Module '{module_slug}' setting '$root' is invalid: module settings must be a JSON object"
        ))
    })?;

    if schema.is_empty() {
        return Ok(serde_json::Value::Object(settings_object));
    }

    let allowed_keys = schema.keys().cloned().collect::<Vec<_>>();
    let mut unknown_keys = settings_object
        .keys()
        .filter(|key| !schema.contains_key(*key))
        .cloned()
        .collect::<Vec<_>>();
    unknown_keys.sort();
    if let Some(key) = unknown_keys.first() {
        return Err(server_error(format!(
            "Module '{module_slug}' setting '{key}' is invalid: unknown setting; allowed keys: {}",
            allowed_keys.join(", ")
        )));
    }

    let mut normalized = serde_json::Map::new();
    for key in allowed_keys {
        let spec = schema
            .get(&key)
            .expect("runtime settings key must exist in schema");

        match settings_object.remove(&key) {
            Some(value) => {
                validate_runtime_setting_value(module_slug, &key, spec, &value)?;
                normalized.insert(key, value);
            }
            None if spec.required && spec.default.is_none() => {
                return Err(server_error(format!(
                    "Module '{module_slug}' setting '{key}' is invalid: required setting is missing"
                )));
            }
            None => {
                if let Some(default) = spec.default.clone() {
                    if !runtime_setting_value_matches_type(spec.value_type.trim(), &default) {
                        return Err(server_error(format!(
                            "Module '{module_slug}' setting '{key}' has invalid schema: default does not match declared type"
                        )));
                    }
                    if !spec.options.is_empty()
                        && !spec.options.iter().any(|option| option == &default)
                    {
                        return Err(server_error(format!(
                            "Module '{module_slug}' setting '{key}' has invalid schema: default must be one of the declared options"
                        )));
                    }
                    normalized.insert(key, default);
                }
            }
        }
    }

    Ok(serde_json::Value::Object(normalized))
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

#[cfg(feature = "ssr")]
fn json_message_list(value: Option<serde_json::Value>) -> Vec<String> {
    value
        .and_then(|value| value.as_array().cloned())
        .unwrap_or_default()
        .into_iter()
        .filter_map(|item| item.as_str().map(ToString::to_string))
        .collect()
}

#[cfg(feature = "ssr")]
fn map_registry_publish_request_row(
    row: sea_orm::QueryResult,
) -> Result<RegistryPublishRequestLifecycle, ServerFnError> {
    Ok(RegistryPublishRequestLifecycle {
        id: row
            .try_get("", "id")
            .map_err(|err| server_error(err.to_string()))?,
        status: upper_snake(
            &row.try_get::<String>("", "status")
                .map_err(|err| server_error(err.to_string()))?,
        ),
        requested_by: row
            .try_get("", "requested_by")
            .map_err(|err| server_error(err.to_string()))?,
        approved_by: row
            .try_get("", "approved_by")
            .map_err(|err| server_error(err.to_string()))?,
        rejected_by: row
            .try_get("", "rejected_by")
            .map_err(|err| server_error(err.to_string()))?,
        rejection_reason: row
            .try_get("", "rejection_reason")
            .map_err(|err| server_error(err.to_string()))?,
        warnings: json_message_list(
            row.try_get::<Option<serde_json::Value>>("", "validation_warnings")
                .map_err(|err| server_error(err.to_string()))?,
        ),
        errors: json_message_list(
            row.try_get::<Option<serde_json::Value>>("", "validation_errors")
                .map_err(|err| server_error(err.to_string()))?,
        ),
        created_at: row
            .try_get::<chrono::DateTime<chrono::Utc>>("", "created_at")
            .map(|value| value.to_rfc3339())
            .map_err(|err| server_error(err.to_string()))?,
        updated_at: row
            .try_get::<chrono::DateTime<chrono::Utc>>("", "updated_at")
            .map(|value| value.to_rfc3339())
            .map_err(|err| server_error(err.to_string()))?,
        published_at: row
            .try_get::<Option<chrono::DateTime<chrono::Utc>>>("", "published_at")
            .map(|value| value.map(|value| value.to_rfc3339()))
            .map_err(|err| server_error(err.to_string()))?,
    })
}

#[cfg(feature = "ssr")]
fn map_registry_release_row(
    row: sea_orm::QueryResult,
) -> Result<RegistryReleaseLifecycle, ServerFnError> {
    Ok(RegistryReleaseLifecycle {
        version: row
            .try_get("", "version")
            .map_err(|err| server_error(err.to_string()))?,
        status: upper_snake(
            &row.try_get::<String>("", "status")
                .map_err(|err| server_error(err.to_string()))?,
        ),
        publisher: row
            .try_get("", "publisher")
            .map_err(|err| server_error(err.to_string()))?,
        checksum_sha256: row
            .try_get("", "checksum_sha256")
            .map_err(|err| server_error(err.to_string()))?,
        published_at: row
            .try_get::<chrono::DateTime<chrono::Utc>>("", "published_at")
            .map(|value| value.to_rfc3339())
            .map_err(|err| server_error(err.to_string()))?,
        yanked_reason: row
            .try_get("", "yanked_reason")
            .map_err(|err| server_error(err.to_string()))?,
        yanked_by: row
            .try_get("", "yanked_by")
            .map_err(|err| server_error(err.to_string()))?,
        yanked_at: row
            .try_get::<Option<chrono::DateTime<chrono::Utc>>>("", "yanked_at")
            .map(|value| value.map(|value| value.to_rfc3339()))
            .map_err(|err| server_error(err.to_string()))?,
    })
}

#[cfg(feature = "ssr")]
async fn load_registry_module_lifecycle(
    app_ctx: &loco_rs::app::AppContext,
    slug: &str,
) -> Result<Option<RegistryModuleLifecycle>, ServerFnError> {
    use sea_orm::{ConnectionTrait, DbBackend, Statement};

    let backend = app_ctx.db.get_database_backend();
    let request_statement = match backend {
        DbBackend::Sqlite => Statement::from_sql_and_values(
            DbBackend::Sqlite,
            r#"
            SELECT
                id,
                status,
                requested_by,
                approved_by,
                rejected_by,
                rejection_reason,
                validation_warnings,
                validation_errors,
                created_at,
                updated_at,
                published_at
            FROM registry_publish_requests
            WHERE slug = ?
            ORDER BY created_at DESC
            LIMIT 1
            "#,
            [slug.into()],
        ),
        _ => Statement::from_sql_and_values(
            DbBackend::Postgres,
            r#"
            SELECT
                id,
                status,
                requested_by,
                approved_by,
                rejected_by,
                rejection_reason,
                validation_warnings,
                validation_errors,
                created_at,
                updated_at,
                published_at
            FROM registry_publish_requests
            WHERE slug = $1
            ORDER BY created_at DESC
            LIMIT 1
            "#,
            [slug.into()],
        ),
    };
    let release_statement = match backend {
        DbBackend::Sqlite => Statement::from_sql_and_values(
            DbBackend::Sqlite,
            r#"
            SELECT
                version,
                status,
                publisher,
                checksum_sha256,
                published_at,
                yanked_reason,
                yanked_by,
                yanked_at
            FROM registry_module_releases
            WHERE slug = ?
            ORDER BY published_at DESC
            LIMIT 1
            "#,
            [slug.into()],
        ),
        _ => Statement::from_sql_and_values(
            DbBackend::Postgres,
            r#"
            SELECT
                version,
                status,
                publisher,
                checksum_sha256,
                published_at,
                yanked_reason,
                yanked_by,
                yanked_at
            FROM registry_module_releases
            WHERE slug = $1
            ORDER BY published_at DESC
            LIMIT 1
            "#,
            [slug.into()],
        ),
    };

    let latest_request = app_ctx
        .db
        .query_one(request_statement)
        .await
        .map_err(|err| server_error(err.to_string()))?
        .map(map_registry_publish_request_row)
        .transpose()?;
    let latest_release = app_ctx
        .db
        .query_one(release_statement)
        .await
        .map_err(|err| server_error(err.to_string()))?
        .map(map_registry_release_row)
        .transpose()?;

    if latest_request.is_none() && latest_release.is_none() {
        return Ok(None);
    }

    Ok(Some(RegistryModuleLifecycle {
        latest_request,
        latest_release,
    }))
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
    match list_marketplace_modules_native(
        variables.search.clone(),
        variables.category.clone(),
        variables.tag.clone(),
        variables.source.clone(),
        variables.trust_level.clone(),
        variables.only_compatible,
        variables.installed_only,
    )
    .await
    {
        Ok(modules) => Ok(modules),
        Err(server_err) => {
            let response: MarketplaceResponse =
                request(MARKETPLACE_QUERY, variables, token, tenant_slug)
                    .await
                    .map_err(|graphql_err| {
                        combine_native_and_graphql_error(server_err, graphql_err)
                    })?;
            Ok(response.marketplace)
        }
    }
}

pub async fn fetch_marketplace_module(
    slug: String,
    token: Option<String>,
    tenant_slug: Option<String>,
) -> Result<Option<MarketplaceModule>, ApiError> {
    match marketplace_module_native(slug.clone()).await {
        Ok(module) => Ok(module),
        Err(server_err) => {
            let response: MarketplaceModuleResponse = request(
                MARKETPLACE_MODULE_QUERY,
                MarketplaceModuleVariables { slug },
                token,
                tenant_slug,
            )
            .await
            .map_err(|graphql_err| combine_native_and_graphql_error(server_err, graphql_err))?;
            Ok(response.marketplace_module)
        }
    }
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

#[server(prefix = "/api/fn", endpoint = "admin/marketplace")]
async fn list_marketplace_modules_native(
    search: Option<String>,
    category: Option<String>,
    tag: Option<String>,
    source: Option<String>,
    trust_level: Option<String>,
    only_compatible: Option<bool>,
    installed_only: Option<bool>,
) -> Result<Vec<MarketplaceModule>, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use leptos::prelude::expect_context;
        use rustok_core::ModuleRegistry;

        let (_app_ctx, _auth, _tenant) = modules_server_context().await?;
        let registry = expect_context::<ModuleRegistry>();
        let search = search
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_lowercase);
        let category = category
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_lowercase);
        let tag = tag
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_lowercase);
        let source = source
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_lowercase);
        let trust_level = trust_level
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_lowercase);
        let only_compatible = only_compatible.unwrap_or(true);
        let installed_only = installed_only.unwrap_or(false);

        Ok(load_runtime_marketplace_modules(&registry)?
            .into_iter()
            .filter(|module| module.kind == "optional")
            .filter(|module| !only_compatible || module.compatible || module.installed)
            .filter(|module| !installed_only || module.installed)
            .filter(|module| {
                trust_level
                    .as_ref()
                    .is_none_or(|value| module.trust_level.eq_ignore_ascii_case(value))
            })
            .filter(|module| {
                source
                    .as_ref()
                    .is_none_or(|value| module.source.eq_ignore_ascii_case(value))
            })
            .filter(|module| {
                category
                    .as_ref()
                    .is_none_or(|value| module.category.eq_ignore_ascii_case(value))
            })
            .filter(|module| {
                tag.as_ref().is_none_or(|value| {
                    module
                        .tags
                        .iter()
                        .any(|module_tag| module_tag.eq_ignore_ascii_case(value))
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
            .collect())
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = (
            search,
            category,
            tag,
            source,
            trust_level,
            only_compatible,
            installed_only,
        );
        Err(ServerFnError::new(
            "admin/marketplace requires the `ssr` feature",
        ))
    }
}

#[server(prefix = "/api/fn", endpoint = "admin/marketplace-module")]
async fn marketplace_module_native(
    slug: String,
) -> Result<Option<MarketplaceModule>, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use leptos::prelude::expect_context;
        use rustok_core::ModuleRegistry;

        let (app_ctx, _auth, _tenant) = modules_server_context().await?;
        let registry = expect_context::<ModuleRegistry>();
        let slug = slug.trim().to_lowercase();
        let module = load_runtime_marketplace_modules(&registry)?
            .into_iter()
            .find(|module| module.slug.eq_ignore_ascii_case(&slug) && module.kind == "optional");

        match module {
            Some(mut module) => {
                module.registry_lifecycle =
                    load_registry_module_lifecycle(&app_ctx, &module.slug).await?;
                Ok(Some(module))
            }
            None => Ok(None),
        }
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = slug;
        Err(ServerFnError::new(
            "admin/marketplace-module requires the `ssr` feature",
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

#[cfg(feature = "ssr")]
async fn persist_tenant_module_state_native(
    db: &sea_orm::DatabaseConnection,
    tenant_id: uuid::Uuid,
    module_slug: &str,
    enabled: bool,
) -> Result<(rustok_tenant::entities::tenant_module::Model, bool, bool), ServerFnError> {
    use rustok_tenant::entities::tenant_module;
    use rustok_tenant::entities::tenant_module::Entity as TenantModuleEntity;
    use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, Set, TransactionTrait};

    let module_slug = module_slug.to_string();

    db.transaction::<_, (rustok_tenant::entities::tenant_module::Model, bool, bool), sea_orm::DbErr>(
        move |txn| {
            let module_slug = module_slug.clone();
            Box::pin(async move {
                let existing = TenantModuleEntity::find()
                    .filter(tenant_module::Column::TenantId.eq(tenant_id))
                    .filter(tenant_module::Column::ModuleSlug.eq(&module_slug))
                    .one(txn)
                    .await?;

                match existing {
                    Some(model) => {
                        if model.enabled == enabled {
                            return Ok((model.clone(), model.enabled, false));
                        }

                        let previous_enabled = model.enabled;
                        let mut active: tenant_module::ActiveModel = model.into();
                        active.enabled = Set(enabled);
                        let updated = active.update(txn).await?;
                        Ok((updated, previous_enabled, true))
                    }
                    None => {
                        let module = tenant_module::ActiveModel {
                            id: Set(rustok_core::generate_id()),
                            tenant_id: Set(tenant_id),
                            module_slug: Set(module_slug),
                            enabled: Set(enabled),
                            settings: Set(serde_json::json!({})),
                            created_at: sea_orm::ActiveValue::NotSet,
                            updated_at: sea_orm::ActiveValue::NotSet,
                        }
                        .insert(txn)
                        .await?;

                        Ok((module, !enabled, true))
                    }
                }
            })
        },
    )
    .await
    .map_err(|err| match err {
        sea_orm::TransactionError::Connection(db_err) => server_error(db_err.to_string()),
        sea_orm::TransactionError::Transaction(db_err) => server_error(db_err.to_string()),
    })
}

#[server(prefix = "/api/fn", endpoint = "admin/toggle-module")]
async fn toggle_module_native(
    module_slug: String,
    enabled: bool,
) -> Result<ToggleModuleResult, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use leptos::prelude::expect_context;
        use rustok_api::has_any_effective_permission;
        use rustok_core::{ModuleContext, ModuleRegistry, Permission};
        use rustok_tenant::entities::tenant_module;
        use rustok_tenant::entities::tenant_module::Entity as TenantModuleEntity;
        use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};

        let (app_ctx, auth, tenant) = modules_server_context().await?;

        if !has_any_effective_permission(&auth.permissions, &[Permission::MODULES_MANAGE]) {
            return Err(ServerFnError::new("modules:manage required"));
        }

        let registry = expect_context::<ModuleRegistry>();
        let Some(module_impl) = registry.get(&module_slug) else {
            return Err(server_error("Unknown module"));
        };

        if !enabled && registry.is_core(&module_slug) {
            return Err(server_error(format!(
                "Module '{module_slug}' is a core platform module and cannot be disabled"
            )));
        }

        let mut enabled_set = TenantModuleEntity::find()
            .filter(tenant_module::Column::TenantId.eq(tenant.id))
            .filter(tenant_module::Column::Enabled.eq(true))
            .all(&app_ctx.db)
            .await
            .map_err(|err| server_error(err.to_string()))?
            .into_iter()
            .map(|module| module.module_slug)
            .collect::<std::collections::HashSet<_>>();

        for core_slug in registry
            .list()
            .into_iter()
            .filter(|module| registry.is_core(module.slug()))
            .map(|module| module.slug().to_string())
        {
            enabled_set.insert(core_slug);
        }

        if enabled {
            let missing = module_impl
                .dependencies()
                .iter()
                .filter(|dependency| !enabled_set.contains(**dependency))
                .map(|dependency| (*dependency).to_string())
                .collect::<Vec<_>>();

            if !missing.is_empty() {
                return Err(server_error(format!(
                    "Missing module dependencies: {}",
                    missing.join(", ")
                )));
            }
        } else {
            let dependents = registry
                .list()
                .into_iter()
                .filter(|module| enabled_set.contains(module.slug()))
                .filter(|module| module.dependencies().contains(&module_slug.as_str()))
                .map(|module| module.slug().to_string())
                .collect::<Vec<_>>();

            if !dependents.is_empty() {
                return Err(server_error(format!(
                    "Module is required by: {}",
                    dependents.join(", ")
                )));
            }
        }

        let (module, previous_enabled, changed) =
            persist_tenant_module_state_native(&app_ctx.db, tenant.id, &module_slug, enabled)
                .await?;

        if changed {
            let module_ctx = ModuleContext {
                db: &app_ctx.db,
                tenant_id: tenant.id,
                config: &module.settings,
            };

            let hook_result = if enabled {
                module_impl.on_enable(module_ctx).await
            } else {
                module_impl.on_disable(module_ctx).await
            };

            if let Err(err) = hook_result {
                log::error!(
                    "Module hook failed for {} (enabled={}): {}. Reverting to {}",
                    module_slug,
                    enabled,
                    err,
                    previous_enabled
                );

                let _ = persist_tenant_module_state_native(
                    &app_ctx.db,
                    tenant.id,
                    &module_slug,
                    previous_enabled,
                )
                .await?;
                return Err(server_error(format!("Module hook failed: {err}")));
            }
        }

        Ok(ToggleModuleResult {
            module_slug: module.module_slug,
            enabled: module.enabled,
            settings: module.settings.to_string(),
        })
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = (module_slug, enabled);
        Err(ServerFnError::new(
            "admin/toggle-module requires the `ssr` feature",
        ))
    }
}

#[server(prefix = "/api/fn", endpoint = "admin/update-module-settings")]
async fn update_module_settings_native(
    module_slug: String,
    settings: String,
) -> Result<TenantModule, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use leptos::prelude::expect_context;
        use rustok_api::has_any_effective_permission;
        use rustok_core::{ModuleRegistry, Permission};
        use rustok_tenant::entities::tenant_module;
        use rustok_tenant::entities::tenant_module::Entity as TenantModuleEntity;
        use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, Set};

        let (app_ctx, auth, tenant) = modules_server_context().await?;

        if !has_any_effective_permission(&auth.permissions, &[Permission::MODULES_MANAGE]) {
            return Err(ServerFnError::new("modules:manage required"));
        }

        let registry = expect_context::<ModuleRegistry>();
        if registry.get(&module_slug).is_none() {
            return Err(server_error("Unknown module"));
        }

        let raw_settings: serde_json::Value = serde_json::from_str(&settings)
            .map_err(|err| server_error(format!("invalid module settings JSON: {err}")))?;
        let schema = load_runtime_module_package_manifest_by_slug(&module_slug)?
            .map(|manifest| manifest.settings)
            .unwrap_or_default();
        let normalized_settings =
            normalize_runtime_module_settings(&module_slug, &schema, raw_settings)?;

        let existing = TenantModuleEntity::find()
            .filter(tenant_module::Column::TenantId.eq(tenant.id))
            .filter(tenant_module::Column::ModuleSlug.eq(&module_slug))
            .one(&app_ctx.db)
            .await
            .map_err(|err| server_error(err.to_string()))?;

        let is_core = registry.is_core(&module_slug);
        let model = match existing {
            Some(model) => {
                if !is_core && !model.enabled {
                    return Err(server_error(format!(
                        "Module '{module_slug}' is not enabled for this tenant"
                    )));
                }

                let was_enabled = model.enabled;
                let mut active: tenant_module::ActiveModel = model.into();
                active.enabled = Set(is_core || was_enabled);
                active.settings = Set(normalized_settings);
                active
                    .update(&app_ctx.db)
                    .await
                    .map_err(|err| server_error(err.to_string()))?
            }
            None if is_core => tenant_module::ActiveModel {
                id: Set(rustok_core::generate_id()),
                tenant_id: Set(tenant.id),
                module_slug: Set(module_slug.clone()),
                enabled: Set(true),
                settings: Set(normalized_settings),
                created_at: sea_orm::ActiveValue::NotSet,
                updated_at: sea_orm::ActiveValue::NotSet,
            }
            .insert(&app_ctx.db)
            .await
            .map_err(|err| server_error(err.to_string()))?,
            None => {
                return Err(server_error(format!(
                    "Module '{module_slug}' is not enabled for this tenant"
                )));
            }
        };

        Ok(TenantModule {
            module_slug: model.module_slug,
            enabled: model.enabled,
            settings: model.settings.to_string(),
        })
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = (module_slug, settings);
        Err(ServerFnError::new(
            "admin/update-module-settings requires the `ssr` feature",
        ))
    }
}

#[server(prefix = "/api/fn", endpoint = "admin/rollback-build")]
async fn rollback_build_native(build_id: String) -> Result<BuildJob, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use rustok_api::has_any_effective_permission;
        use rustok_core::Permission;
        use sea_orm::{ConnectionTrait, DbBackend, Statement, TransactionTrait};

        let (app_ctx, auth, _tenant) = modules_server_context().await?;

        if !has_any_effective_permission(&auth.permissions, &[Permission::MODULES_MANAGE]) {
            return Err(ServerFnError::new("modules:manage required"));
        }

        let build_id = uuid::Uuid::parse_str(build_id.trim())
            .map_err(|err| server_error(format!("invalid build id: {err}")))?;
        let backend = app_ctx.db.get_database_backend();

        let active_build_statement = match backend {
            DbBackend::Sqlite => Statement::from_string(
                DbBackend::Sqlite,
                r#"
                SELECT id
                FROM builds
                WHERE status IN ('queued', 'running')
                ORDER BY created_at DESC
                LIMIT 1
                "#,
            ),
            _ => Statement::from_string(
                DbBackend::Postgres,
                r#"
                SELECT id
                FROM builds
                WHERE status IN ('queued', 'running')
                ORDER BY created_at DESC
                LIMIT 1
                "#,
            ),
        };

        if app_ctx
            .db
            .query_one(active_build_statement)
            .await
            .map_err(|err| server_error(err.to_string()))?
            .is_some()
        {
            return Err(server_error(
                "Cannot rollback while another build is still queued or running",
            ));
        }

        let build_query = match backend {
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
                WHERE id = ?1
                LIMIT 1
                "#,
                vec![build_id.into()],
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
                WHERE id = $1
                LIMIT 1
                "#,
                vec![build_id.into()],
            ),
        };

        let build_row = app_ctx
            .db
            .query_one(build_query)
            .await
            .map_err(|err| server_error(err.to_string()))?
            .ok_or_else(|| server_error("Build not found"))?;

        let release_id = build_row
            .try_get::<Option<String>>("", "release_id")
            .map_err(|err| server_error(err.to_string()))?
            .ok_or_else(|| server_error("Build does not have a release to rollback"))?;

        let active_release_statement = match backend {
            DbBackend::Sqlite => Statement::from_string(
                DbBackend::Sqlite,
                r#"
                SELECT id, build_id, previous_release_id
                FROM releases
                WHERE status = 'active'
                LIMIT 1
                "#,
            ),
            _ => Statement::from_string(
                DbBackend::Postgres,
                r#"
                SELECT id, build_id, previous_release_id
                FROM releases
                WHERE status = 'active'
                LIMIT 1
                "#,
            ),
        };

        let active_release = app_ctx
            .db
            .query_one(active_release_statement)
            .await
            .map_err(|err| server_error(err.to_string()))?
            .ok_or_else(|| server_error("No active release available for rollback"))?;

        let active_release_id = active_release
            .try_get::<String>("", "id")
            .map_err(|err| server_error(err.to_string()))?;
        if active_release_id != release_id {
            return Err(server_error(
                "Only the build that backs the current active release can be rolled back",
            ));
        }

        let previous_release_id = active_release
            .try_get::<Option<String>>("", "previous_release_id")
            .map_err(|err| server_error(err.to_string()))?
            .ok_or_else(|| server_error("No previous release available for rollback"))?;

        let previous_release_statement = match backend {
            DbBackend::Sqlite => Statement::from_sql_and_values(
                DbBackend::Sqlite,
                r#"
                SELECT id, build_id
                FROM releases
                WHERE id = ?1
                LIMIT 1
                "#,
                vec![previous_release_id.clone().into()],
            ),
            _ => Statement::from_sql_and_values(
                DbBackend::Postgres,
                r#"
                SELECT id, build_id
                FROM releases
                WHERE id = $1
                LIMIT 1
                "#,
                vec![previous_release_id.clone().into()],
            ),
        };

        let previous_release = app_ctx
            .db
            .query_one(previous_release_statement)
            .await
            .map_err(|err| server_error(err.to_string()))?
            .ok_or_else(|| server_error("Previous release not found"))?;
        let restored_build_id = previous_release
            .try_get::<uuid::Uuid>("", "build_id")
            .map_err(|err| server_error(err.to_string()))?;

        let now = chrono::Utc::now();
        app_ctx
            .db
            .transaction::<_, (), sea_orm::DbErr>(move |txn| {
                let active_release_id = active_release_id.clone();
                let previous_release_id = previous_release_id.clone();
                Box::pin(async move {
                    let rollback_current = match backend {
                        DbBackend::Sqlite => Statement::from_sql_and_values(
                            DbBackend::Sqlite,
                            r#"
                            UPDATE releases
                            SET status = 'rolled_back', rolled_back_at = ?2, updated_at = ?2
                            WHERE id = ?1
                            "#,
                            vec![active_release_id.into(), now.into()],
                        ),
                        _ => Statement::from_sql_and_values(
                            DbBackend::Postgres,
                            r#"
                            UPDATE releases
                            SET status = 'rolled_back', rolled_back_at = $2, updated_at = $2
                            WHERE id = $1
                            "#,
                            vec![active_release_id.into(), now.into()],
                        ),
                    };
                    txn.execute(rollback_current).await?;

                    let activate_previous = match backend {
                        DbBackend::Sqlite => Statement::from_sql_and_values(
                            DbBackend::Sqlite,
                            r#"
                            UPDATE releases
                            SET status = 'active', deployed_at = ?2, updated_at = ?2
                            WHERE id = ?1
                            "#,
                            vec![previous_release_id.into(), now.into()],
                        ),
                        _ => Statement::from_sql_and_values(
                            DbBackend::Postgres,
                            r#"
                            UPDATE releases
                            SET status = 'active', deployed_at = $2, updated_at = $2
                            WHERE id = $1
                            "#,
                            vec![previous_release_id.into(), now.into()],
                        ),
                    };
                    txn.execute(activate_previous).await?;
                    Ok(())
                })
            })
            .await
            .map_err(|err| match err {
                sea_orm::TransactionError::Connection(db_err) => server_error(db_err.to_string()),
                sea_orm::TransactionError::Transaction(db_err) => server_error(db_err.to_string()),
            })?;

        let restored_build_statement = match backend {
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
                WHERE id = ?1
                LIMIT 1
                "#,
                vec![restored_build_id.into()],
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
                WHERE id = $1
                LIMIT 1
                "#,
                vec![restored_build_id.into()],
            ),
        };

        let restored_build = app_ctx
            .db
            .query_one(restored_build_statement)
            .await
            .map_err(|err| server_error(err.to_string()))?
            .ok_or_else(|| server_error("restored release is missing its build record"))?;

        map_build_job_row(restored_build)
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = build_id;
        Err(ServerFnError::new(
            "admin/rollback-build requires the `ssr` feature",
        ))
    }
}

#[server(prefix = "/api/fn", endpoint = "admin/install-module")]
async fn install_module_native(slug: String, version: String) -> Result<BuildJob, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use rustok_api::has_any_effective_permission;
        use rustok_core::Permission;

        let (app_ctx, auth, _tenant) = modules_server_context().await?;

        if !has_any_effective_permission(&auth.permissions, &[Permission::MODULES_MANAGE]) {
            return Err(ServerFnError::new("modules:manage required"));
        }

        let version = version.trim();
        if version.is_empty() {
            return Err(server_error("Version must not be empty"));
        }

        let descriptor = load_runtime_module_descriptor_by_slug(&slug)?
            .ok_or_else(|| server_error(format!("Unknown module '{slug}'")))?;
        let mut manifest = load_runtime_modules_manifest()?;
        let original_manifest = manifest.clone();

        if manifest.modules.contains_key(&slug) {
            return Err(server_error(format!(
                "Module '{slug}' is already installed in modules.toml"
            )));
        }

        let workspace_root = runtime_workspace_root();
        let relative_root = descriptor
            .root
            .strip_prefix(&workspace_root)
            .map_err(|err| server_error(err.to_string()))?
            .to_string_lossy()
            .replace('\\', "/");

        let dependencies = if descriptor.package_manifest.dependencies.is_empty() {
            Vec::new()
        } else {
            descriptor
                .package_manifest
                .dependencies
                .keys()
                .cloned()
                .collect::<Vec<_>>()
        };

        let missing_dependencies = dependencies
            .iter()
            .filter(|dependency| !manifest.modules.contains_key(*dependency))
            .cloned()
            .collect::<Vec<_>>();
        if !missing_dependencies.is_empty() {
            return Err(server_error(format!(
                "Module '{slug}' depends on missing modules: {}",
                missing_dependencies.join(", ")
            )));
        }

        manifest.modules.insert(
            slug.clone(),
            RuntimeManifestModuleSpec {
                source: "path".to_string(),
                crate_name: descriptor.cargo_manifest.package.name,
                path: Some(relative_root),
                version: Some(version.to_string()),
                git: None,
                rev: None,
                required: false,
                depends_on: dependencies,
            },
        );

        if runtime_builtin_default_enabled().contains(slug.as_str())
            && !manifest
                .settings
                .default_enabled
                .iter()
                .any(|item| item == &slug)
        {
            manifest.settings.default_enabled.push(slug.clone());
            manifest.settings.default_enabled.sort();
        }

        save_manifest_and_enqueue_build(
            &app_ctx,
            &original_manifest,
            &manifest,
            &auth.user_id.to_string(),
            format!("install module {slug}"),
            format!("+{slug}@{version}"),
        )
        .await
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = (slug, version);
        Err(ServerFnError::new(
            "admin/install-module requires the `ssr` feature",
        ))
    }
}

#[server(prefix = "/api/fn", endpoint = "admin/uninstall-module")]
async fn uninstall_module_native(slug: String) -> Result<BuildJob, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use rustok_api::has_any_effective_permission;
        use rustok_core::Permission;

        let (app_ctx, auth, _tenant) = modules_server_context().await?;

        if !has_any_effective_permission(&auth.permissions, &[Permission::MODULES_MANAGE]) {
            return Err(ServerFnError::new("modules:manage required"));
        }

        let mut manifest = load_runtime_modules_manifest()?;
        let original_manifest = manifest.clone();
        let spec = manifest.modules.get(&slug).cloned().ok_or_else(|| {
            server_error(format!("Module '{slug}' is not installed in modules.toml"))
        })?;

        if spec.required {
            return Err(server_error(format!(
                "Module '{slug}' is required and cannot be removed from modules.toml"
            )));
        }

        let dependents = manifest
            .modules
            .iter()
            .filter(|(candidate_slug, _)| candidate_slug.as_str() != slug)
            .filter(|(_, candidate_spec)| candidate_spec.depends_on.iter().any(|dep| dep == &slug))
            .map(|(candidate_slug, _)| candidate_slug.clone())
            .collect::<Vec<_>>();

        if !dependents.is_empty() {
            return Err(server_error(format!(
                "Module '{slug}' is required by: {}",
                dependents.join(", ")
            )));
        }

        manifest.modules.remove(&slug);
        manifest
            .settings
            .default_enabled
            .retain(|item| item != &slug);

        save_manifest_and_enqueue_build(
            &app_ctx,
            &original_manifest,
            &manifest,
            &auth.user_id.to_string(),
            format!("uninstall module {slug}"),
            format!("-{slug}"),
        )
        .await
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = slug;
        Err(ServerFnError::new(
            "admin/uninstall-module requires the `ssr` feature",
        ))
    }
}

#[server(prefix = "/api/fn", endpoint = "admin/upgrade-module")]
async fn upgrade_module_native(slug: String, version: String) -> Result<BuildJob, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use rustok_api::has_any_effective_permission;
        use rustok_core::Permission;

        let (app_ctx, auth, _tenant) = modules_server_context().await?;

        if !has_any_effective_permission(&auth.permissions, &[Permission::MODULES_MANAGE]) {
            return Err(ServerFnError::new("modules:manage required"));
        }

        let version = version.trim();
        if version.is_empty() {
            return Err(server_error("Version must not be empty"));
        }

        let mut manifest = load_runtime_modules_manifest()?;
        let original_manifest = manifest.clone();
        let spec = manifest.modules.get_mut(&slug).ok_or_else(|| {
            server_error(format!("Module '{slug}' is not installed in modules.toml"))
        })?;

        if spec.version.as_deref() == Some(version) {
            return Err(server_error(format!(
                "Module '{slug}' is already pinned to version '{version}'"
            )));
        }

        spec.version = Some(version.to_string());

        save_manifest_and_enqueue_build(
            &app_ctx,
            &original_manifest,
            &manifest,
            &auth.user_id.to_string(),
            format!("upgrade module {slug}"),
            format!("~{slug}@{version}"),
        )
        .await
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = (slug, version);
        Err(ServerFnError::new(
            "admin/upgrade-module requires the `ssr` feature",
        ))
    }
}

pub async fn toggle_module(
    module_slug: String,
    enabled: bool,
    token: Option<String>,
    tenant_slug: Option<String>,
) -> Result<ToggleModuleResult, ApiError> {
    match toggle_module_native(module_slug.clone(), enabled).await {
        Ok(result) => Ok(result),
        Err(server_err) => {
            let response: ToggleModuleResponse = request(
                TOGGLE_MODULE_MUTATION,
                ToggleModuleVariables {
                    module_slug,
                    enabled,
                },
                token,
                tenant_slug,
            )
            .await
            .map_err(|graphql_err| combine_native_and_graphql_error(server_err, graphql_err))?;
            Ok(response.toggle_module)
        }
    }
}

pub async fn update_module_settings(
    module_slug: String,
    settings: String,
    token: Option<String>,
    tenant_slug: Option<String>,
) -> Result<TenantModule, ApiError> {
    match update_module_settings_native(module_slug.clone(), settings.clone()).await {
        Ok(module) => Ok(module),
        Err(server_err) => {
            let response: UpdateModuleSettingsResponse = request(
                UPDATE_MODULE_SETTINGS_MUTATION,
                UpdateModuleSettingsVariables {
                    module_slug,
                    settings,
                },
                token,
                tenant_slug,
            )
            .await
            .map_err(|graphql_err| combine_native_and_graphql_error(server_err, graphql_err))?;
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
    match install_module_native(slug.clone(), version.clone()).await {
        Ok(build) => Ok(build),
        Err(server_err) => {
            let response: InstallModuleResponse = request(
                INSTALL_MODULE_MUTATION,
                InstallModuleVariables { slug, version },
                token,
                tenant_slug,
            )
            .await
            .map_err(|graphql_err| combine_native_and_graphql_error(server_err, graphql_err))?;
            Ok(response.install_module)
        }
    }
}

pub async fn uninstall_module(
    slug: String,
    token: Option<String>,
    tenant_slug: Option<String>,
) -> Result<BuildJob, ApiError> {
    match uninstall_module_native(slug.clone()).await {
        Ok(build) => Ok(build),
        Err(server_err) => {
            let response: UninstallModuleResponse = request(
                UNINSTALL_MODULE_MUTATION,
                UninstallModuleVariables { slug },
                token,
                tenant_slug,
            )
            .await
            .map_err(|graphql_err| combine_native_and_graphql_error(server_err, graphql_err))?;
            Ok(response.uninstall_module)
        }
    }
}

pub async fn upgrade_module(
    slug: String,
    version: String,
    token: Option<String>,
    tenant_slug: Option<String>,
) -> Result<BuildJob, ApiError> {
    match upgrade_module_native(slug.clone(), version.clone()).await {
        Ok(build) => Ok(build),
        Err(server_err) => {
            let response: UpgradeModuleResponse = request(
                UPGRADE_MODULE_MUTATION,
                UpgradeModuleVariables { slug, version },
                token,
                tenant_slug,
            )
            .await
            .map_err(|graphql_err| combine_native_and_graphql_error(server_err, graphql_err))?;
            Ok(response.upgrade_module)
        }
    }
}

pub async fn rollback_build(
    build_id: String,
    token: Option<String>,
    tenant_slug: Option<String>,
) -> Result<BuildJob, ApiError> {
    match rollback_build_native(build_id.clone()).await {
        Ok(build) => Ok(build),
        Err(server_err) => {
            let response: RollbackBuildResponse = request(
                ROLLBACK_BUILD_MUTATION,
                RollbackBuildVariables { build_id },
                token,
                tenant_slug,
            )
            .await
            .map_err(|graphql_err| combine_native_and_graphql_error(server_err, graphql_err))?;
            Ok(response.rollback_build)
        }
    }
}
