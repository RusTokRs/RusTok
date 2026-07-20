use rustok_build::DeploymentProfile;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use thiserror::Error;
use utoipa::ToSchema;

#[derive(Debug, Clone, Copy, Default)]
pub struct ModuleUiSurfaceFlags {
    pub has_admin_ui: bool,
    pub has_storefront_ui: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ModulesManifest {
    #[serde(default)]
    pub schema: u32,
    #[serde(default)]
    pub app: String,
    #[serde(default)]
    pub build: BuildConfig,
    #[serde(default)]
    pub modules: HashMap<String, ManifestModuleSpec>,
    #[serde(default)]
    pub settings: SettingsManifest,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct BuildConfig {
    #[serde(default)]
    pub target: String,
    #[serde(default)]
    pub profile: String,
    #[serde(default)]
    pub server: ServerBuildConfig,
    #[serde(default)]
    pub admin: AdminBuildConfig,
    #[serde(default)]
    pub storefront: Vec<StorefrontBuildConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ServerBuildConfig {
    #[serde(default)]
    pub embed_admin: bool,
    #[serde(default)]
    pub embed_storefront: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AdminBuildConfig {
    #[serde(default)]
    pub stack: String,
    #[serde(default)]
    pub public_url: String,
    #[serde(default)]
    pub redirect_uris: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct StorefrontBuildConfig {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub stack: String,
    #[serde(default)]
    pub public_url: String,
    #[serde(default)]
    pub redirect_uris: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeploymentSurfaceContract {
    pub profile: DeploymentProfile,
    pub embed_admin: bool,
    pub embed_storefront: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SettingsManifest {
    #[serde(default)]
    pub default_enabled: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ManifestModuleSpec {
    pub source: String,
    #[serde(rename = "crate")]
    pub crate_name: String,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub category: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub icon_url: Option<String>,
    #[serde(default)]
    pub banner_url: Option<String>,
    #[serde(default)]
    pub screenshots: Vec<String>,
    #[serde(default)]
    pub version: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub git: Option<String>,
    #[serde(default)]
    pub rev: Option<String>,
    #[serde(default)]
    pub path: Option<String>,
    #[serde(default)]
    pub required: bool,
    #[serde(default)]
    pub depends_on: Vec<String>,
    #[serde(default)]
    pub dependency_version_reqs: HashMap<String, String>,
    #[serde(default)]
    pub conflicts_with: Vec<String>,
    #[serde(default)]
    pub ownership: String,
    #[serde(default)]
    pub trust_level: String,
    #[serde(default)]
    pub rustok_min_version: Option<String>,
    #[serde(default)]
    pub rustok_max_version: Option<String>,
    #[serde(default)]
    pub ui_classification: Option<String>,
    #[serde(default)]
    pub entry_type: Option<String>,
    #[serde(default)]
    pub graphql_query_type: Option<String>,
    #[serde(default)]
    pub graphql_mutation_type: Option<String>,
    #[serde(default)]
    pub http_routes_fn: Option<String>,
    #[serde(default)]
    pub http_axum_router_fn: Option<String>,
    #[serde(default)]
    pub http_axum_webhook_router_fn: Option<String>,
    #[serde(default)]
    pub http_webhook_routes_fn: Option<String>,
    #[serde(default)]
    pub recommended_admin_surfaces: Vec<String>,
    #[serde(default)]
    pub showcase_admin_surfaces: Vec<String>,
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub settings_schema: HashMap<String, ModuleSettingSpec>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, ToSchema)]
pub struct ModuleSettingSpec {
    #[serde(rename = "type", default)]
    pub value_type: String,
    #[serde(default)]
    pub required: bool,
    #[serde(default)]
    pub default: Option<serde_json::Value>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub min: Option<f64>,
    #[serde(default)]
    pub max: Option<f64>,
    #[serde(default)]
    pub options: Vec<serde_json::Value>,
    #[serde(default)]
    pub object_keys: Vec<String>,
    #[serde(default)]
    pub item_type: Option<String>,
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    #[schema(no_recursion)]
    pub properties: HashMap<String, ModuleSettingSpec>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schema(no_recursion)]
    pub items: Option<Box<ModuleSettingSpec>>,
}

#[derive(Debug, Clone)]
pub struct InstalledManifestModule {
    pub slug: String,
    pub source: String,
    pub crate_name: String,
    pub version: Option<String>,
    pub git: Option<String>,
    pub rev: Option<String>,
    pub path: Option<String>,
    pub required: bool,
    pub depends_on: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct CatalogManifestModule {
    pub slug: String,
    pub source: String,
    pub crate_name: String,
    pub name: Option<String>,
    pub category: Option<String>,
    pub tags: Vec<String>,
    pub icon_url: Option<String>,
    pub banner_url: Option<String>,
    pub screenshots: Vec<String>,
    pub version: Option<String>,
    pub description: Option<String>,
    pub git: Option<String>,
    pub rev: Option<String>,
    pub path: Option<String>,
    pub required: bool,
    pub depends_on: Vec<String>,
    pub ownership: String,
    pub trust_level: String,
    pub rustok_min_version: Option<String>,
    pub rustok_max_version: Option<String>,
    pub publisher: Option<String>,
    pub checksum_sha256: Option<String>,
    pub signature: Option<String>,
    pub versions: Vec<CatalogModuleVersion>,
    pub has_admin_ui: bool,
    pub has_storefront_ui: bool,
    pub ui_classification: String,
    pub recommended_admin_surfaces: Vec<String>,
    pub showcase_admin_surfaces: Vec<String>,
    pub settings_schema: HashMap<String, ModuleSettingSpec>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CatalogModuleVersion {
    pub version: String,
    #[serde(default)]
    pub changelog: Option<String>,
    #[serde(default)]
    pub yanked: bool,
    #[serde(default)]
    pub published_at: Option<String>,
    #[serde(default)]
    pub checksum_sha256: Option<String>,
    #[serde(default)]
    pub signature: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ModulePackageManifest {
    #[serde(default)]
    pub module: ModulePackageMetadata,
    #[serde(default)]
    pub marketplace: ModulePackageMarketplaceMetadata,
    #[serde(rename = "crate", default)]
    pub crate_contract: ModulePackageCrateContract,
    #[serde(default)]
    pub dependencies: HashMap<String, ModulePackageDependencySpec>,
    #[serde(default)]
    pub conflicts: ModulePackageConflicts,
    #[serde(default)]
    pub provides: ModulePackageProvides,
    #[serde(default)]
    pub settings: HashMap<String, ModuleSettingSpec>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ModulePackageMetadata {
    #[serde(default)]
    pub slug: Option<String>,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub category: Option<String>,
    #[serde(default)]
    pub version: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub ownership: String,
    #[serde(default)]
    pub trust_level: String,
    #[serde(default)]
    pub rustok_min_version: Option<String>,
    #[serde(default)]
    pub rustok_max_version: Option<String>,
    #[serde(default)]
    pub ui_classification: Option<String>,
    #[serde(default)]
    pub recommended_admin_surfaces: Vec<String>,
    #[serde(default)]
    pub showcase_admin_surfaces: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ModulePackageMarketplaceMetadata {
    #[serde(default)]
    pub category: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub icon: Option<String>,
    #[serde(default)]
    pub banner: Option<String>,
    #[serde(default)]
    pub screenshots: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ModulePackageCrateContract {
    #[serde(default)]
    pub entry_type: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ModulePackageProvides {
    #[serde(default)]
    pub graphql: Option<ModulePackageGraphqlProvides>,
    #[serde(default)]
    pub http: Option<ModulePackageHttpProvides>,
    #[serde(default)]
    pub admin_ui: Option<ModulePackageUiProvides>,
    #[serde(default)]
    pub storefront_ui: Option<ModulePackageUiProvides>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ModulePackageGraphqlProvides {
    #[serde(default)]
    pub query: Option<String>,
    #[serde(default)]
    pub mutation: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ModulePackageHttpProvides {
    #[serde(default)]
    pub routes: Option<String>,
    #[serde(default)]
    pub axum_router: Option<String>,
    #[serde(default)]
    pub axum_webhook_router: Option<String>,
    #[serde(default)]
    pub webhook_routes: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ModulePackageUiProvides {
    #[serde(default)]
    pub leptos_crate: Option<String>,
    #[serde(default)]
    pub next_package: Option<String>,
    #[serde(default)]
    pub route_segment: Option<String>,
    #[serde(default)]
    pub nav_label: Option<String>,
    #[serde(default)]
    pub slot: Option<String>,
    #[serde(default)]
    pub page_title: Option<String>,
    #[serde(default)]
    pub pages: Vec<ModulePackageUiPage>,
    #[serde(default)]
    pub i18n: Option<ModulePackageUiI18nContract>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ModulePackageUiPage {
    #[serde(default)]
    pub subpath: String,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub nav_label: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ModulePackageUiI18nContract {
    #[serde(default)]
    pub default_locale: Option<String>,
    #[serde(default)]
    pub supported_locales: Vec<String>,
    #[serde(default)]
    pub leptos_locales_path: Option<String>,
    #[serde(default)]
    pub next_messages_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ModulePackageDependencySpec {
    #[serde(default)]
    pub version_req: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ModulePackageConflicts {
    #[serde(default)]
    pub modules: Vec<String>,
}

#[derive(Debug, Clone, Default)]

pub struct ManifestDiff {
    pub changes: Vec<String>,
}

impl ManifestDiff {
    pub fn added(slug: &str, version: Option<&str>) -> Self {
        Self {
            changes: vec![format_change('+', slug, version)],
        }
    }

    pub fn removed(slug: &str) -> Self {
        Self {
            changes: vec![format!("-{slug}")],
        }
    }

    pub fn upgraded(slug: &str, version: &str) -> Self {
        Self {
            changes: vec![format_change('~', slug, Some(version))],
        }
    }

    pub fn summary(&self) -> String {
        self.changes.join(",")
    }
}

fn format_change(prefix: char, slug: &str, version: Option<&str>) -> String {
    match version {
        Some(version) if !version.trim().is_empty() => format!("{prefix}{slug}@{version}"),
        _ => format!("{prefix}{slug}"),
    }
}

#[derive(Debug, Error)]
pub enum ManifestError {
    #[error("Failed to read modules manifest {path}: {error}")]
    Read { path: String, error: String },
    #[error("Failed to parse modules manifest {path}: {error}")]
    Parse { path: String, error: String },
    #[error("Failed to write modules manifest {path}: {error}")]
    Write { path: String, error: String },
    #[error("Unknown module '{0}'")]
    UnknownModule(String),
    #[error("Module '{0}' is already installed in modules.toml")]
    ModuleAlreadyInstalled(String),
    #[error("Module '{0}' is not installed in modules.toml")]
    ModuleNotInstalled(String),
    #[error("Module '{0}' is required and cannot be removed from modules.toml")]
    RequiredModule(String),
    #[error("Module '{slug}' is required by: {dependents}")]
    HasDependents { slug: String, dependents: String },
    #[error("Module '{slug}' depends on missing modules: {missing}")]
    MissingDependencies { slug: String, missing: String },
    #[error("Default-enabled modules are not installed: {0}")]
    UnknownDefaultEnabled(String),
    #[error("Module '{0}' is already pinned to version '{1}'")]
    VersionUnchanged(String, String),
    #[error("Version must not be empty")]
    InvalidVersion,
    #[error("modules.toml entries are not available in ModuleRegistry: {0}")]
    MissingInRegistry(String),
    #[error("modules.toml required flags conflict with ModuleRegistry kinds: {0}")]
    RequiredMismatch(String),
    #[error("modules.toml depends_on conflict with ModuleRegistry dependencies: {0}")]
    DependencyMismatch(String),
    #[error("Invalid build surface configuration: {0}")]
    InvalidBuildSurface(String),
    #[error("Failed to read module package manifest {path}: {error}")]
    ModulePackageRead { path: String, error: String },
    #[error("Failed to parse module package manifest {path}: {error}")]
    ModulePackageParse { path: String, error: String },
    #[error("Module '{slug}' requires rustok-module.toml at {path}")]
    MissingModulePackageManifest { slug: String, path: String },
    #[error("Module '{slug}' has invalid ownership '{value}'")]
    InvalidModuleOwnership { slug: String, value: String },
    #[error("Module '{slug}' has invalid trust level '{value}'")]
    InvalidModuleTrustLevel { slug: String, value: String },
    #[error("Module '{slug}' has invalid ui_classification '{value}'")]
    InvalidModuleUiClassification { slug: String, value: String },
    #[error("Module package manifest for '{slug}' declares slug '{found}', expected '{slug}'")]
    ModulePackageSlugMismatch { slug: String, found: String },
    #[error("Module '{slug}' has invalid version '{value}'")]
    InvalidModuleVersion { slug: String, value: String },
    #[error("Module '{slug}' declares invalid dependency '{dependency}'")]
    InvalidModuleDependency { slug: String, dependency: String },
    #[error("Module '{slug}' declares invalid conflict '{conflict}'")]
    InvalidModuleConflict { slug: String, conflict: String },
    #[error("Module '{slug}' dependency '{dependency}' has invalid version requirement '{value}'")]
    InvalidDependencyVersionReq {
        slug: String,
        dependency: String,
        value: String,
    },
    #[error("Module '{slug}' requires a version for dependency '{dependency}'")]
    MissingDependencyVersion { slug: String, dependency: String },
    #[error(
        "Module '{slug}' requires '{dependency}' version '{required}', but installed '{installed}'"
    )]
    IncompatibleDependencyVersion {
        slug: String,
        dependency: String,
        required: String,
        installed: String,
    },
    #[error("Module '{slug}' conflicts with installed module '{conflicts_with}'")]
    ConflictingModule {
        slug: String,
        conflicts_with: String,
    },
    #[error(
        "Module '{slug}' is incompatible with RusToK {current_version} (min={minimum:?}, max={maximum:?})"
    )]
    IncompatibleRustokVersion {
        slug: String,
        current_version: String,
        minimum: Option<String>,
        maximum: Option<String>,
    },
    #[error("Module '{slug}' has invalid admin surface '{value}' in {field}")]
    InvalidModuleAdminSurface {
        slug: String,
        field: String,
        value: String,
    },
    #[error("Module '{slug}' lists admin surface '{surface}' as both recommended and showcase")]
    ConflictingModuleAdminSurface { slug: String, surface: String },
    #[error("Module '{slug}' has invalid setting key '{key}'")]
    InvalidModuleSettingKey { slug: String, key: String },
    #[error("Module '{slug}' setting '{key}' has invalid schema: {reason}")]
    InvalidModuleSettingSchema {
        slug: String,
        key: String,
        reason: String,
    },
    #[error("Module '{slug}' setting '{key}' is invalid: {reason}")]
    InvalidModuleSettingValue {
        slug: String,
        key: String,
        reason: String,
    },
    #[error("Module '{slug}' has invalid marketplace metadata '{field}': {reason}")]
    InvalidModuleMarketplaceMetadata {
        slug: String,
        field: String,
        reason: String,
    },
    #[error("Module '{slug}' has invalid {surface} UI wiring: {reason}")]
    InvalidModuleUiWiring {
        slug: String,
        surface: String,
        reason: String,
    },
    #[error("Module '{slug}' has invalid HTTP wiring: {reason}")]
    InvalidModuleHttpWiring { slug: String, reason: String },
}
