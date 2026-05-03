use crate::models::build::DeploymentProfile;
use crate::services::build_service::ModuleSpec as BuildModuleSpec;
use rustok_core::{normalize_locale_tag, ModuleRegistry};
use semver::{Version, VersionReq};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use thiserror::Error;
use utoipa::ToSchema;

use crate::error::{Error as ServerError, Result as ServerResult};

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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BuildExecutionPlan {
    pub cargo_package: String,
    pub cargo_profile: String,
    pub cargo_target: Option<String>,
    pub cargo_features: Vec<String>,
    pub cargo_command: String,
    #[serde(default)]
    pub admin_build: Option<FrontendBuildPlan>,
    #[serde(default)]
    pub storefront_build: Option<FrontendBuildPlan>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum FrontendBuildTool {
    Cargo,
    Trunk,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum FrontendArtifactKind {
    File,
    Directory,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FrontendBuildPlan {
    pub surface: String,
    pub tool: FrontendBuildTool,
    pub package: String,
    pub workspace_path: String,
    pub profile: String,
    pub target: Option<String>,
    pub artifact_path: String,
    pub artifact_kind: FrontendArtifactKind,
    pub command: String,
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
    pub properties: HashMap<String, ModuleSettingSpec>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
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

fn admin_frontend_build_plan(
    manifest: &ModulesManifest,
    cargo_profile: &str,
) -> Option<FrontendBuildPlan> {
    let admin_stack = manifest.build.admin.stack.trim().to_ascii_lowercase();
    let requires_leptos_admin = manifest.build.server.embed_admin || admin_stack == "leptos";

    requires_leptos_admin.then(|| {
        let mut command_parts = vec!["trunk".to_string(), "build".to_string()];
        if cargo_profile == "release" {
            command_parts.push("--release".to_string());
        }

        FrontendBuildPlan {
            surface: "admin".to_string(),
            tool: FrontendBuildTool::Trunk,
            package: "rustok-admin".to_string(),
            workspace_path: "apps/admin".to_string(),
            profile: cargo_profile.to_string(),
            target: None,
            artifact_path: "apps/admin/dist".to_string(),
            artifact_kind: FrontendArtifactKind::Directory,
            command: command_parts.join(" "),
        }
    })
}

fn storefront_frontend_build_plan(
    manifest: &ModulesManifest,
    cargo_profile: &str,
    cargo_target: Option<&str>,
) -> Option<FrontendBuildPlan> {
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
        artifact_path.push_str(binary_output_dir_name(cargo_profile));
        artifact_path.push('/');
        artifact_path.push_str(&binary_file_name("rustok-storefront", cargo_target));

        FrontendBuildPlan {
            surface: "storefront".to_string(),
            tool: FrontendBuildTool::Cargo,
            package: "rustok-storefront".to_string(),
            workspace_path: ".".to_string(),
            profile: cargo_profile.to_string(),
            target: cargo_target.map(ToString::to_string),
            artifact_path,
            artifact_kind: FrontendArtifactKind::File,
            command: command_parts.join(" "),
        }
    })
}

fn binary_output_dir_name(profile: &str) -> &str {
    if profile == "release" {
        "release"
    } else {
        profile
    }
}

fn binary_file_name(package: &str, cargo_target: Option<&str>) -> String {
    let exe_suffix = executable_suffix(cargo_target);
    if exe_suffix.is_empty() {
        package.to_string()
    } else {
        format!("{package}.{exe_suffix}")
    }
}

fn executable_suffix(cargo_target: Option<&str>) -> &'static str {
    match cargo_target {
        Some(target) if target.contains("windows") => "exe",
        Some(_) => "",
        None => std::env::consts::EXE_EXTENSION,
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct ModulePackageManifest {
    #[serde(default)]
    module: ModulePackageMetadata,
    #[serde(default)]
    marketplace: ModulePackageMarketplaceMetadata,
    #[serde(rename = "crate", default)]
    crate_contract: ModulePackageCrateContract,
    #[serde(default)]
    dependencies: HashMap<String, ModulePackageDependencySpec>,
    #[serde(default)]
    conflicts: ModulePackageConflicts,
    #[serde(default)]
    provides: ModulePackageProvides,
    #[serde(default)]
    settings: HashMap<String, ModuleSettingSpec>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct ModulePackageMetadata {
    #[serde(default)]
    slug: Option<String>,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    category: Option<String>,
    #[serde(default)]
    version: Option<String>,
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    ownership: String,
    #[serde(default)]
    trust_level: String,
    #[serde(default)]
    rustok_min_version: Option<String>,
    #[serde(default)]
    rustok_max_version: Option<String>,
    #[serde(default)]
    ui_classification: Option<String>,
    #[serde(default)]
    recommended_admin_surfaces: Vec<String>,
    #[serde(default)]
    showcase_admin_surfaces: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct ModulePackageMarketplaceMetadata {
    #[serde(default)]
    category: Option<String>,
    #[serde(default)]
    tags: Vec<String>,
    #[serde(default)]
    icon: Option<String>,
    #[serde(default)]
    banner: Option<String>,
    #[serde(default)]
    screenshots: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct ModulePackageCrateContract {
    #[serde(default)]
    entry_type: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct ModulePackageProvides {
    #[serde(default)]
    graphql: Option<ModulePackageGraphqlProvides>,
    #[serde(default)]
    http: Option<ModulePackageHttpProvides>,
    #[serde(default)]
    admin_ui: Option<ModulePackageUiProvides>,
    #[serde(default)]
    storefront_ui: Option<ModulePackageUiProvides>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct ModulePackageGraphqlProvides {
    #[serde(default)]
    query: Option<String>,
    #[serde(default)]
    mutation: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct ModulePackageHttpProvides {
    #[serde(default)]
    routes: Option<String>,
    #[serde(default)]
    webhook_routes: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct ModulePackageUiProvides {
    #[serde(default)]
    leptos_crate: Option<String>,
    #[serde(default)]
    next_package: Option<String>,
    #[serde(default)]
    route_segment: Option<String>,
    #[serde(default)]
    nav_label: Option<String>,
    #[serde(default)]
    slot: Option<String>,
    #[serde(default)]
    page_title: Option<String>,
    #[serde(default)]
    pages: Vec<ModulePackageUiPage>,
    #[serde(default)]
    i18n: Option<ModulePackageUiI18nContract>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct ModulePackageUiPage {
    #[serde(default)]
    subpath: String,
    #[serde(default)]
    title: Option<String>,
    #[serde(default)]
    nav_label: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct ModulePackageUiI18nContract {
    #[serde(default)]
    default_locale: Option<String>,
    #[serde(default)]
    supported_locales: Vec<String>,
    #[serde(default)]
    leptos_locales_path: Option<String>,
    #[serde(default)]
    next_messages_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct ModulePackageDependencySpec {
    #[serde(default)]
    version_req: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct ModulePackageConflicts {
    #[serde(default)]
    modules: Vec<String>,
}

#[derive(Debug, Clone, Default)]
pub struct ManifestDiff {
    changes: Vec<String>,
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
    #[error("Module '{slug}' is incompatible with RusToK {current_version} (min={minimum:?}, max={maximum:?})")]
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
}

fn normalize_deps(deps: &[String]) -> HashSet<String> {
    deps.iter().cloned().collect()
}

fn default_manifest_path() -> PathBuf {
    if let Ok(path) = std::env::var("RUSTOK_MODULES_MANIFEST") {
        return PathBuf::from(path);
    }

    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../modules.toml")
}

fn module_package_manifest_path(spec: &ManifestModuleSpec) -> Option<PathBuf> {
    if spec.source != "path" {
        return None;
    }

    let module_path = spec.path.as_ref()?;
    Some(
        default_manifest_path()
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .join(module_path)
            .join("rustok-module.toml"),
    )
}

fn module_root_path(spec: &ManifestModuleSpec) -> Option<PathBuf> {
    if spec.source != "path" {
        return None;
    }

    let module_path = spec.path.as_ref()?;
    Some(
        default_manifest_path()
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .join(module_path),
    )
}

fn workspace_root_path() -> PathBuf {
    default_manifest_path()
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .to_path_buf()
}

fn resolve_module_contract_path(
    module_root: &Path,
    raw_path: &str,
) -> std::result::Result<PathBuf, String> {
    let raw_path = raw_path.trim();
    if raw_path.is_empty() {
        return Err("path must not be empty".to_string());
    }

    let candidate = PathBuf::from(raw_path);
    let resolved = if candidate.is_absolute() {
        candidate
    } else {
        module_root.join(candidate)
    };

    let canonical = std::fs::canonicalize(&resolved)
        .map_err(|_| format!("{} is missing", resolved.display()))?;
    let workspace_root = std::fs::canonicalize(workspace_root_path())
        .map_err(|error| format!("failed to resolve workspace root: {error}"))?;

    if !canonical.starts_with(&workspace_root) {
        return Err(format!(
            "{} resolves outside workspace root {}",
            resolved.display(),
            workspace_root.display()
        ));
    }

    Ok(canonical)
}

fn validate_ui_i18n_bundle_dir(
    slug: &str,
    surface: &str,
    field: &str,
    dir: &Path,
    supported_locales: &[String],
) -> Result<(), ManifestError> {
    if !dir.is_dir() {
        return Err(ManifestError::InvalidModuleUiWiring {
            slug: slug.to_string(),
            surface: surface.to_string(),
            reason: format!("{field} must point to a directory, got {}", dir.display()),
        });
    }

    for locale in supported_locales {
        let locale_file = dir.join(format!("{locale}.json"));
        if !locale_file.is_file() {
            return Err(ManifestError::InvalidModuleUiWiring {
                slug: slug.to_string(),
                surface: surface.to_string(),
                reason: format!("{field} is missing locale bundle {}", locale_file.display()),
            });
        }
    }

    Ok(())
}

fn validate_module_ui_i18n_contract(
    slug: &str,
    surface: &str,
    module_root: &Path,
    ui: &ModulePackageUiProvides,
) -> Result<(), ManifestError> {
    let Some(i18n) = ui.i18n.as_ref() else {
        return Ok(());
    };

    let supported_locales = i18n
        .supported_locales
        .iter()
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
        .map(|locale| {
            normalize_locale_tag(locale).ok_or_else(|| ManifestError::InvalidModuleUiWiring {
                slug: slug.to_string(),
                surface: surface.to_string(),
                reason: format!("i18n.supported_locales contains invalid locale '{locale}'"),
            })
        })
        .collect::<Result<std::collections::BTreeSet<_>, _>>()?
        .into_iter()
        .collect::<Vec<_>>();

    if supported_locales.is_empty() {
        return Err(ManifestError::InvalidModuleUiWiring {
            slug: slug.to_string(),
            surface: surface.to_string(),
            reason: "i18n.supported_locales must list at least one locale".to_string(),
        });
    }

    if let Some(default_locale) = i18n
        .default_locale
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        let default_locale = normalize_locale_tag(default_locale).ok_or_else(|| {
            ManifestError::InvalidModuleUiWiring {
                slug: slug.to_string(),
                surface: surface.to_string(),
                reason: format!("i18n.default_locale '{default_locale}' is invalid"),
            }
        })?;
        if !supported_locales
            .iter()
            .any(|locale| locale == &default_locale)
        {
            return Err(ManifestError::InvalidModuleUiWiring {
                slug: slug.to_string(),
                surface: surface.to_string(),
                reason: format!(
                    "i18n.default_locale '{default_locale}' must be present in i18n.supported_locales"
                ),
            });
        }
    }

    let leptos_locales_path = i18n
        .leptos_locales_path
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let next_messages_path = i18n
        .next_messages_path
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty());

    if leptos_locales_path.is_none() && next_messages_path.is_none() {
        return Err(ManifestError::InvalidModuleUiWiring {
            slug: slug.to_string(),
            surface: surface.to_string(),
            reason: "i18n contract must declare leptos_locales_path and/or next_messages_path"
                .to_string(),
        });
    }

    if leptos_locales_path.is_some()
        && ui
            .leptos_crate
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .is_none()
    {
        return Err(ManifestError::InvalidModuleUiWiring {
            slug: slug.to_string(),
            surface: surface.to_string(),
            reason: "i18n.leptos_locales_path requires [provides.*_ui].leptos_crate".to_string(),
        });
    }

    if next_messages_path.is_some()
        && ui
            .next_package
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .is_none()
    {
        return Err(ManifestError::InvalidModuleUiWiring {
            slug: slug.to_string(),
            surface: surface.to_string(),
            reason: "i18n.next_messages_path requires [provides.*_ui].next_package".to_string(),
        });
    }

    if let Some(path) = leptos_locales_path {
        let resolved = resolve_module_contract_path(module_root, path).map_err(|reason| {
            ManifestError::InvalidModuleUiWiring {
                slug: slug.to_string(),
                surface: surface.to_string(),
                reason: format!("i18n.leptos_locales_path {reason}"),
            }
        })?;
        validate_ui_i18n_bundle_dir(
            slug,
            surface,
            "i18n.leptos_locales_path",
            &resolved,
            &supported_locales,
        )?;
    }

    if let Some(path) = next_messages_path {
        let resolved = resolve_module_contract_path(module_root, path).map_err(|reason| {
            ManifestError::InvalidModuleUiWiring {
                slug: slug.to_string(),
                surface: surface.to_string(),
                reason: format!("i18n.next_messages_path {reason}"),
            }
        })?;
        validate_ui_i18n_bundle_dir(
            slug,
            surface,
            "i18n.next_messages_path",
            &resolved,
            &supported_locales,
        )?;
    }

    Ok(())
}

fn merge_module_package_manifest(
    mut spec: ManifestModuleSpec,
    package_manifest: ModulePackageManifest,
) -> ManifestModuleSpec {
    let crate_name = spec.crate_name.clone();
    let metadata = package_manifest.module;

    if let Some(version) = metadata
        .version
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        spec.version = Some(version.to_string());
    }
    if let Some(name) = metadata
        .name
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        spec.name = Some(name.to_string());
    }
    if let Some(description) = metadata
        .description
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        spec.description = Some(description.to_string());
    }
    if let Some(category) = package_manifest
        .marketplace
        .category
        .as_deref()
        .or(metadata.category.as_deref())
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        spec.category = Some(category.to_string());
    }
    if !package_manifest.marketplace.tags.is_empty() {
        spec.tags = package_manifest
            .marketplace
            .tags
            .into_iter()
            .map(|tag| tag.trim().to_string())
            .filter(|tag| !tag.is_empty())
            .collect::<Vec<_>>();
        spec.tags.sort();
        spec.tags.dedup();
    }
    if let Some(icon_url) = package_manifest
        .marketplace
        .icon
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        spec.icon_url = Some(icon_url.to_string());
    }
    if let Some(banner_url) = package_manifest
        .marketplace
        .banner
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        spec.banner_url = Some(banner_url.to_string());
    }
    if !package_manifest.marketplace.screenshots.is_empty() {
        spec.screenshots = package_manifest
            .marketplace
            .screenshots
            .into_iter()
            .map(|item| item.trim().to_string())
            .filter(|item| !item.is_empty())
            .collect::<Vec<_>>();
        spec.screenshots.dedup();
    }

    if !metadata.ownership.trim().is_empty() {
        spec.ownership = metadata.ownership;
    }
    if !metadata.trust_level.trim().is_empty() {
        spec.trust_level = metadata.trust_level;
    }
    if metadata.rustok_min_version.is_some() {
        spec.rustok_min_version = metadata.rustok_min_version;
    }
    if metadata.rustok_max_version.is_some() {
        spec.rustok_max_version = metadata.rustok_max_version;
    }
    if let Some(ui_classification) = metadata
        .ui_classification
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        spec.ui_classification = Some(ui_classification.to_string());
    }
    if let Some(entry_type) = qualify_module_type_path(
        &crate_name,
        package_manifest.crate_contract.entry_type.as_deref(),
    ) {
        spec.entry_type = Some(entry_type);
    }
    if let Some(graphql) = package_manifest.provides.graphql {
        if let Some(query_type) = qualify_module_type_path(&crate_name, graphql.query.as_deref()) {
            spec.graphql_query_type = Some(query_type);
        }
        if let Some(mutation_type) =
            qualify_module_type_path(&crate_name, graphql.mutation.as_deref())
        {
            spec.graphql_mutation_type = Some(mutation_type);
        }
    }
    if let Some(http) = package_manifest.provides.http {
        if let Some(routes_fn) = qualify_module_member_path(&crate_name, http.routes.as_deref()) {
            spec.http_routes_fn = Some(routes_fn);
        }
        if let Some(webhook_routes_fn) =
            qualify_module_member_path(&crate_name, http.webhook_routes.as_deref())
        {
            spec.http_webhook_routes_fn = Some(webhook_routes_fn);
        }
    }
    if !metadata.recommended_admin_surfaces.is_empty() {
        spec.recommended_admin_surfaces = metadata.recommended_admin_surfaces;
    }
    if !metadata.showcase_admin_surfaces.is_empty() {
        spec.showcase_admin_surfaces = metadata.showcase_admin_surfaces;
    }
    if !package_manifest.settings.is_empty() {
        spec.settings_schema = package_manifest.settings;
    }

    for (dependency, dependency_spec) in package_manifest.dependencies {
        let dependency = dependency.trim().to_string();
        if !spec.depends_on.iter().any(|item| item == &dependency) {
            spec.depends_on.push(dependency.clone());
        }

        let version_req = dependency_spec.version_req.trim();
        if !version_req.is_empty() {
            spec.dependency_version_reqs
                .insert(dependency, version_req.to_string());
        }
    }

    spec.depends_on.sort();
    spec.depends_on.dedup();

    for conflict in package_manifest.conflicts.modules {
        let conflict = conflict.trim().to_string();
        if !conflict.is_empty() && !spec.conflicts_with.iter().any(|item| item == &conflict) {
            spec.conflicts_with.push(conflict);
        }
    }

    spec.conflicts_with.sort();
    spec.conflicts_with.dedup();

    spec
}

fn qualify_module_type_path(crate_name: &str, value: Option<&str>) -> Option<String> {
    let value = value?.trim();
    if value.is_empty() {
        return None;
    }

    let crate_ident = crate_name.replace('-', "_");
    let relative = value.strip_prefix("crate::").unwrap_or(value);
    Some(format!("{crate_ident}::{relative}"))
}

fn qualify_module_member_path(crate_name: &str, value: Option<&str>) -> Option<String> {
    qualify_module_type_path(crate_name, value)
}

fn is_valid_module_setting_key(value: &str) -> bool {
    !value.is_empty()
        && value
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '_' || ch == '-')
}

fn is_supported_setting_type(value_type: &str) -> bool {
    matches!(
        value_type,
        "string" | "integer" | "number" | "boolean" | "object" | "array" | "json" | "any"
    )
}

fn declared_object_keys(spec: &ModuleSettingSpec) -> Vec<String> {
    if !spec.properties.is_empty() {
        let mut keys = spec.properties.keys().cloned().collect::<Vec<_>>();
        keys.sort();
        keys
    } else {
        spec.object_keys.clone()
    }
}

fn declared_item_type(spec: &ModuleSettingSpec) -> Option<&str> {
    spec.items
        .as_deref()
        .map(|item| item.value_type.trim())
        .filter(|value| !value.is_empty())
        .or_else(|| {
            spec.item_type
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
        })
}

pub(crate) fn module_setting_shape_value(spec: &ModuleSettingSpec) -> Option<serde_json::Value> {
    let mut shape = serde_json::Map::new();

    if !spec.properties.is_empty() {
        let properties = spec
            .properties
            .iter()
            .map(|(key, property_spec)| {
                (
                    key.clone(),
                    serde_json::to_value(property_spec)
                        .expect("module setting schema should serialize to shape json"),
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
                .expect("module setting item schema should serialize to shape json"),
        );
    }

    (!shape.is_empty()).then_some(serde_json::Value::Object(shape))
}

fn setting_value_matches_type(value_type: &str, value: &serde_json::Value) -> bool {
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

fn validate_setting_spec(
    slug: &str,
    key: &str,
    spec: &ModuleSettingSpec,
) -> Result<(), ManifestError> {
    use std::collections::HashSet;

    if !is_valid_module_setting_key(key) {
        return Err(ManifestError::InvalidModuleSettingKey {
            slug: slug.to_string(),
            key: key.to_string(),
        });
    }

    let value_type = spec.value_type.trim();
    if !is_supported_setting_type(value_type) {
        return Err(ManifestError::InvalidModuleSettingSchema {
            slug: slug.to_string(),
            key: key.to_string(),
            reason: format!("unsupported type '{value_type}'"),
        });
    }

    if let Some(default) = &spec.default {
        if !setting_value_matches_type(value_type, default) {
            return Err(ManifestError::InvalidModuleSettingSchema {
                slug: slug.to_string(),
                key: key.to_string(),
                reason: "default does not match declared type".to_string(),
            });
        }
    }

    if let (Some(min), Some(max)) = (spec.min, spec.max) {
        if min > max {
            return Err(ManifestError::InvalidModuleSettingSchema {
                slug: slug.to_string(),
                key: key.to_string(),
                reason: format!("min ({min}) must not exceed max ({max})"),
            });
        }
    }

    if (spec.min.is_some() || spec.max.is_some())
        && !matches!(value_type, "integer" | "number" | "string" | "array")
    {
        return Err(ManifestError::InvalidModuleSettingSchema {
            slug: slug.to_string(),
            key: key.to_string(),
            reason: "min/max are only supported for string, array, integer, and number".to_string(),
        });
    }

    if !spec.options.is_empty() {
        if !matches!(value_type, "string" | "integer" | "number" | "boolean") {
            return Err(ManifestError::InvalidModuleSettingSchema {
                slug: slug.to_string(),
                key: key.to_string(),
                reason:
                    "options are only supported for scalar string/integer/number/boolean settings"
                        .to_string(),
            });
        }

        for option in &spec.options {
            if !setting_value_matches_type(value_type, option) {
                return Err(ManifestError::InvalidModuleSettingSchema {
                    slug: slug.to_string(),
                    key: key.to_string(),
                    reason: "all options must match the declared type".to_string(),
                });
            }
        }

        if let Some(default) = &spec.default {
            if !spec.options.iter().any(|option| option == default) {
                return Err(ManifestError::InvalidModuleSettingSchema {
                    slug: slug.to_string(),
                    key: key.to_string(),
                    reason: "default must be one of the declared options".to_string(),
                });
            }
        }
    }

    if !spec.object_keys.is_empty() {
        if value_type != "object" {
            return Err(ManifestError::InvalidModuleSettingSchema {
                slug: slug.to_string(),
                key: key.to_string(),
                reason: "object_keys are only supported for object settings".to_string(),
            });
        }

        let mut seen_keys = HashSet::new();
        for object_key in &spec.object_keys {
            if !is_valid_module_setting_key(object_key) {
                return Err(ManifestError::InvalidModuleSettingSchema {
                    slug: slug.to_string(),
                    key: key.to_string(),
                    reason: format!("invalid object key '{object_key}'"),
                });
            }

            if !seen_keys.insert(object_key.clone()) {
                return Err(ManifestError::InvalidModuleSettingSchema {
                    slug: slug.to_string(),
                    key: key.to_string(),
                    reason: format!("duplicate object key '{object_key}'"),
                });
            }
        }

        if let Some(default) = &spec.default {
            if let Some(object) = default.as_object() {
                if let Some(unknown_key) = object
                    .keys()
                    .find(|candidate| !spec.object_keys.iter().any(|allowed| allowed == *candidate))
                {
                    return Err(ManifestError::InvalidModuleSettingSchema {
                        slug: slug.to_string(),
                        key: key.to_string(),
                        reason: format!("default contains undeclared object key '{unknown_key}'"),
                    });
                }
            }
        }
    }

    if !spec.properties.is_empty() {
        if value_type != "object" {
            return Err(ManifestError::InvalidModuleSettingSchema {
                slug: slug.to_string(),
                key: key.to_string(),
                reason: "properties are only supported for object settings".to_string(),
            });
        }

        let mut property_keys = spec.properties.keys().cloned().collect::<Vec<_>>();
        property_keys.sort();
        let mut explicit_object_keys = spec.object_keys.clone();
        explicit_object_keys.sort();
        if !spec.object_keys.is_empty() && property_keys != explicit_object_keys {
            return Err(ManifestError::InvalidModuleSettingSchema {
                slug: slug.to_string(),
                key: key.to_string(),
                reason: "object_keys must match declared properties when both are provided"
                    .to_string(),
            });
        }

        for (property_key, property_spec) in &spec.properties {
            validate_setting_spec(slug, &format!("{key}.{property_key}"), property_spec)?;
        }

        if let Some(default) = &spec.default {
            if let Some(object) = default.as_object() {
                for (property_key, property_value) in object {
                    if let Some(property_spec) = spec.properties.get(property_key) {
                        validate_setting_value(
                            slug,
                            &format!("{key}.{property_key}"),
                            property_spec,
                            property_value,
                        )?;
                    }
                }
            }
        }
    }

    if let Some(item_type) = spec.item_type.as_deref() {
        let item_type = item_type.trim();
        if value_type != "array" {
            return Err(ManifestError::InvalidModuleSettingSchema {
                slug: slug.to_string(),
                key: key.to_string(),
                reason: "item_type is only supported for array settings".to_string(),
            });
        }

        if !is_supported_setting_type(item_type) {
            return Err(ManifestError::InvalidModuleSettingSchema {
                slug: slug.to_string(),
                key: key.to_string(),
                reason: format!("unsupported array item type '{item_type}'"),
            });
        }

        if let Some(default) = &spec.default {
            if let Some(items) = default.as_array() {
                if items
                    .iter()
                    .any(|item| !setting_value_matches_type(item_type, item))
                {
                    return Err(ManifestError::InvalidModuleSettingSchema {
                        slug: slug.to_string(),
                        key: key.to_string(),
                        reason: "default array items must match declared item_type".to_string(),
                    });
                }
            }
        }
    }

    if let Some(items) = &spec.items {
        if value_type != "array" {
            return Err(ManifestError::InvalidModuleSettingSchema {
                slug: slug.to_string(),
                key: key.to_string(),
                reason: "items are only supported for array settings".to_string(),
            });
        }

        validate_setting_spec(slug, &format!("{key}[]"), items)?;

        if let Some(item_type) = spec.item_type.as_deref() {
            if items.value_type.trim() != item_type.trim() {
                return Err(ManifestError::InvalidModuleSettingSchema {
                    slug: slug.to_string(),
                    key: key.to_string(),
                    reason: "item_type must match items.type when both are provided".to_string(),
                });
            }
        }

        if let Some(default) = &spec.default {
            if let Some(array) = default.as_array() {
                for (index, item) in array.iter().enumerate() {
                    validate_setting_value(slug, &format!("{key}[{index}]"), items, item)?;
                }
            }
        }
    }

    Ok(())
}

fn validate_setting_value(
    slug: &str,
    key: &str,
    spec: &ModuleSettingSpec,
    value: &serde_json::Value,
) -> Result<(), ManifestError> {
    let value_type = spec.value_type.trim();
    if !setting_value_matches_type(value_type, value) {
        return Err(ManifestError::InvalidModuleSettingValue {
            slug: slug.to_string(),
            key: key.to_string(),
            reason: format!("expected {value_type}"),
        });
    }

    if !spec.options.is_empty() && !spec.options.iter().any(|option| option == value) {
        let allowed = spec
            .options
            .iter()
            .map(serde_json::Value::to_string)
            .collect::<Vec<_>>()
            .join(", ");
        return Err(ManifestError::InvalidModuleSettingValue {
            slug: slug.to_string(),
            key: key.to_string(),
            reason: format!("must be one of: {allowed}"),
        });
    }

    if !declared_object_keys(spec).is_empty() {
        let object = value
            .as_object()
            .expect("object_keys validation only runs for object values");
        let allowed_keys = declared_object_keys(spec);
        let mut unknown_keys = object
            .keys()
            .filter(|candidate| !allowed_keys.iter().any(|allowed| allowed == *candidate))
            .cloned()
            .collect::<Vec<_>>();
        unknown_keys.sort();
        if let Some(unknown_key) = unknown_keys.first() {
            return Err(ManifestError::InvalidModuleSettingValue {
                slug: slug.to_string(),
                key: key.to_string(),
                reason: format!(
                    "unknown object key '{unknown_key}'; allowed keys: {}",
                    allowed_keys.join(", ")
                ),
            });
        }
    }

    if let Some(item_type) = declared_item_type(spec) {
        let array = value
            .as_array()
            .expect("item_type validation only runs for array values");
        if let Some((index, _)) = array
            .iter()
            .enumerate()
            .find(|(_, item)| !setting_value_matches_type(item_type, item))
        {
            return Err(ManifestError::InvalidModuleSettingValue {
                slug: slug.to_string(),
                key: key.to_string(),
                reason: format!("array item at index {index} must be {item_type}"),
            });
        }
    }

    if !spec.properties.is_empty() {
        let object = value
            .as_object()
            .expect("properties validation only runs for object values");
        for (property_key, property_value) in object {
            if let Some(property_spec) = spec.properties.get(property_key) {
                validate_setting_value(
                    slug,
                    &format!("{key}.{property_key}"),
                    property_spec,
                    property_value,
                )?;
            }
        }
    }

    if let Some(items) = &spec.items {
        let array = value
            .as_array()
            .expect("items validation only runs for array values");
        for (index, item) in array.iter().enumerate() {
            validate_setting_value(slug, &format!("{key}[{index}]"), items, item)?;
        }
    }

    match value_type {
        "integer" | "number" => {
            let numeric_value =
                value
                    .as_f64()
                    .ok_or_else(|| ManifestError::InvalidModuleSettingValue {
                        slug: slug.to_string(),
                        key: key.to_string(),
                        reason: format!("expected {value_type}"),
                    })?;
            if let Some(min) = spec.min {
                if numeric_value < min {
                    return Err(ManifestError::InvalidModuleSettingValue {
                        slug: slug.to_string(),
                        key: key.to_string(),
                        reason: format!("must be >= {min}"),
                    });
                }
            }
            if let Some(max) = spec.max {
                if numeric_value > max {
                    return Err(ManifestError::InvalidModuleSettingValue {
                        slug: slug.to_string(),
                        key: key.to_string(),
                        reason: format!("must be <= {max}"),
                    });
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
                    return Err(ManifestError::InvalidModuleSettingValue {
                        slug: slug.to_string(),
                        key: key.to_string(),
                        reason: format!("length must be >= {min}"),
                    });
                }
            }
            if let Some(max) = spec.max {
                if length > max {
                    return Err(ManifestError::InvalidModuleSettingValue {
                        slug: slug.to_string(),
                        key: key.to_string(),
                        reason: format!("length must be <= {max}"),
                    });
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
                    return Err(ManifestError::InvalidModuleSettingValue {
                        slug: slug.to_string(),
                        key: key.to_string(),
                        reason: format!("length must be >= {min}"),
                    });
                }
            }
            if let Some(max) = spec.max {
                if length > max {
                    return Err(ManifestError::InvalidModuleSettingValue {
                        slug: slug.to_string(),
                        key: key.to_string(),
                        reason: format!("length must be <= {max}"),
                    });
                }
            }
        }
        _ => {}
    }

    Ok(())
}

fn normalize_module_settings(
    slug: &str,
    schema: &HashMap<String, ModuleSettingSpec>,
    settings: serde_json::Value,
) -> Result<serde_json::Value, ManifestError> {
    let mut settings_object =
        settings
            .as_object()
            .cloned()
            .ok_or_else(|| ManifestError::InvalidModuleSettingValue {
                slug: slug.to_string(),
                key: "$root".to_string(),
                reason: "module settings must be a JSON object".to_string(),
            })?;

    if schema.is_empty() {
        return Ok(serde_json::Value::Object(settings_object));
    }

    let mut allowed_keys = schema.keys().cloned().collect::<Vec<_>>();
    allowed_keys.sort();

    let mut unknown_keys = settings_object
        .keys()
        .filter(|key| !schema.contains_key(*key))
        .cloned()
        .collect::<Vec<_>>();
    unknown_keys.sort();
    if let Some(key) = unknown_keys.first() {
        return Err(ManifestError::InvalidModuleSettingValue {
            slug: slug.to_string(),
            key: key.clone(),
            reason: format!("unknown setting; allowed keys: {}", allowed_keys.join(", ")),
        });
    }

    let mut normalized = serde_json::Map::new();
    for key in allowed_keys {
        let spec = schema
            .get(&key)
            .expect("allowed settings key must exist in schema");

        match settings_object.remove(&key) {
            Some(value) => {
                validate_setting_value(slug, &key, spec, &value)?;
                normalized.insert(key, value);
            }
            None if spec.required && spec.default.is_none() => {
                return Err(ManifestError::InvalidModuleSettingValue {
                    slug: slug.to_string(),
                    key,
                    reason: "required setting is missing".to_string(),
                });
            }
            None => {
                if let Some(default) = spec.default.clone() {
                    normalized.insert(key, default);
                }
            }
        }
    }

    Ok(serde_json::Value::Object(normalized))
}

fn validate_module_package_metadata(
    slug: &str,
    package_manifest: &ModulePackageManifest,
) -> Result<(), ManifestError> {
    let metadata = &package_manifest.module;

    if let Some(found_slug) = metadata
        .slug
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        if !is_valid_module_slug(found_slug) || found_slug != slug {
            return Err(ManifestError::ModulePackageSlugMismatch {
                slug: slug.to_string(),
                found: found_slug.to_string(),
            });
        }
    }

    if let Some(version) = metadata
        .version
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        Version::parse(version).map_err(|_| ManifestError::InvalidModuleVersion {
            slug: slug.to_string(),
            value: version.to_string(),
        })?;
    }

    let ownership = metadata.ownership.trim();
    if !ownership.is_empty() && !is_valid_module_ownership(ownership) {
        return Err(ManifestError::InvalidModuleOwnership {
            slug: slug.to_string(),
            value: ownership.to_string(),
        });
    }

    let trust_level = metadata.trust_level.trim();
    if !trust_level.is_empty() && !is_valid_trust_level(trust_level) {
        return Err(ManifestError::InvalidModuleTrustLevel {
            slug: slug.to_string(),
            value: trust_level.to_string(),
        });
    }

    let recommended = validate_admin_surfaces(
        slug,
        "recommended_admin_surfaces",
        &metadata.recommended_admin_surfaces,
    )?;
    let showcase = validate_admin_surfaces(
        slug,
        "showcase_admin_surfaces",
        &metadata.showcase_admin_surfaces,
    )?;

    if let Some(surface) = recommended.intersection(&showcase).next() {
        return Err(ManifestError::ConflictingModuleAdminSurface {
            slug: slug.to_string(),
            surface: surface.clone(),
        });
    }

    for (dependency, dependency_spec) in &package_manifest.dependencies {
        let dependency = dependency.trim();
        if !is_valid_module_slug(dependency) {
            return Err(ManifestError::InvalidModuleDependency {
                slug: slug.to_string(),
                dependency: dependency.to_string(),
            });
        }

        let version_req = dependency_spec.version_req.trim();
        if version_req.is_empty() {
            continue;
        }

        VersionReq::parse(version_req).map_err(|_| ManifestError::InvalidDependencyVersionReq {
            slug: slug.to_string(),
            dependency: dependency.to_string(),
            value: version_req.to_string(),
        })?;
    }

    for conflict in &package_manifest.conflicts.modules {
        let conflict = conflict.trim();
        if !is_valid_module_slug(conflict) || conflict == slug {
            return Err(ManifestError::InvalidModuleConflict {
                slug: slug.to_string(),
                conflict: conflict.to_string(),
            });
        }
    }

    for (key, spec) in &package_manifest.settings {
        validate_setting_spec(slug, key, spec)?;
    }

    Ok(())
}

fn validate_module_ui_wiring(
    slug: &str,
    module_root: &Path,
    package_manifest: &ModulePackageManifest,
) -> Result<(), ManifestError> {
    for (surface, declared_crate) in [
        (
            "admin",
            package_manifest
                .provides
                .admin_ui
                .as_ref()
                .and_then(|ui| ui.leptos_crate.as_deref()),
        ),
        (
            "storefront",
            package_manifest
                .provides
                .storefront_ui
                .as_ref()
                .and_then(|ui| ui.leptos_crate.as_deref()),
        ),
    ] {
        let manifest_path = module_root.join(surface).join("Cargo.toml");
        let has_subcrate = manifest_path.exists();
        let declared_crate = declared_crate
            .map(str::trim)
            .filter(|value| !value.is_empty());

        if has_subcrate && declared_crate.is_none() {
            return Err(ManifestError::InvalidModuleUiWiring {
                slug: slug.to_string(),
                surface: surface.to_string(),
                reason: format!(
                    "{} exists, but rustok-module.toml is missing [provides.{}_ui].leptos_crate",
                    manifest_path.display(),
                    surface
                ),
            });
        }

        if !has_subcrate && declared_crate.is_some() {
            return Err(ManifestError::InvalidModuleUiWiring {
                slug: slug.to_string(),
                surface: surface.to_string(),
                reason: format!(
                    "declares [provides.{}_ui].leptos_crate, but {} is missing",
                    surface,
                    manifest_path.display()
                ),
            });
        }
    }

    if let Some(admin_ui) = package_manifest.provides.admin_ui.as_ref() {
        validate_module_ui_i18n_contract(slug, "admin", module_root, admin_ui)?;
    }

    if let Some(storefront_ui) = package_manifest.provides.storefront_ui.as_ref() {
        validate_module_ui_i18n_contract(slug, "storefront", module_root, storefront_ui)?;
    }

    Ok(())
}

#[derive(Debug, Clone, Copy, Default)]
struct ModuleUiSurfaceFlags {
    has_admin_ui: bool,
    has_storefront_ui: bool,
}

fn module_package_ui_surface_flags(
    spec: &ManifestModuleSpec,
) -> Result<ModuleUiSurfaceFlags, ManifestError> {
    let Some(path) = module_package_manifest_path(spec) else {
        return Ok(ModuleUiSurfaceFlags::default());
    };

    if !path.exists() {
        return Ok(ModuleUiSurfaceFlags::default());
    }

    let raw = std::fs::read_to_string(&path).map_err(|error| ManifestError::ModulePackageRead {
        path: path.display().to_string(),
        error: error.to_string(),
    })?;
    let package_manifest: ModulePackageManifest =
        toml::from_str(&raw).map_err(|error| ManifestError::ModulePackageParse {
            path: path.display().to_string(),
            error: error.to_string(),
        })?;

    Ok(ModuleUiSurfaceFlags {
        has_admin_ui: package_manifest.provides.admin_ui.is_some(),
        has_storefront_ui: package_manifest.provides.storefront_ui.is_some(),
    })
}

pub fn catalog_module_ui_classification(
    has_admin_ui: bool,
    has_storefront_ui: bool,
) -> &'static str {
    match (has_admin_ui, has_storefront_ui) {
        (true, true) => "dual_surface",
        (true, false) => "admin_only",
        (false, true) => "storefront_only",
        (false, false) => "no_ui",
    }
}

fn normalize_module_ui_classification(value: &str) -> Option<String> {
    let normalized = value.trim().to_ascii_lowercase().replace('-', "_");
    match normalized.as_str() {
        "dual_surface" | "admin_only" | "storefront_only" | "no_ui" | "capability_only"
        | "future_ui" => Some(normalized),
        _ => None,
    }
}

fn resolved_catalog_module_ui_classification(
    slug: &str,
    explicit: Option<&str>,
    has_admin_ui: bool,
    has_storefront_ui: bool,
) -> Result<String, ManifestError> {
    let derived = catalog_module_ui_classification(has_admin_ui, has_storefront_ui);
    let Some(explicit) = explicit.map(str::trim).filter(|value| !value.is_empty()) else {
        return Ok(derived.to_string());
    };

    let normalized = normalize_module_ui_classification(explicit).ok_or_else(|| {
        ManifestError::InvalidModuleUiClassification {
            slug: slug.to_string(),
            value: explicit.to_string(),
        }
    })?;

    let matches_surface_contract = match normalized.as_str() {
        "dual_surface" => has_admin_ui && has_storefront_ui,
        "admin_only" => has_admin_ui && !has_storefront_ui,
        "storefront_only" => !has_admin_ui && has_storefront_ui,
        "no_ui" | "capability_only" | "future_ui" => !has_admin_ui && !has_storefront_ui,
        _ => false,
    };

    if !matches_surface_contract {
        return Err(ManifestError::InvalidModuleUiClassification {
            slug: slug.to_string(),
            value: explicit.to_string(),
        });
    }

    Ok(normalized)
}

fn apply_module_package_manifest(
    slug: &str,
    spec: &ManifestModuleSpec,
) -> Result<ManifestModuleSpec, ManifestError> {
    let Some(path) = module_package_manifest_path(spec) else {
        return Ok(spec.clone());
    };

    if !path.exists() {
        return Ok(spec.clone());
    }

    let raw = std::fs::read_to_string(&path).map_err(|error| ManifestError::ModulePackageRead {
        path: path.display().to_string(),
        error: error.to_string(),
    })?;
    let package_manifest: ModulePackageManifest =
        toml::from_str(&raw).map_err(|error| ManifestError::ModulePackageParse {
            path: path.display().to_string(),
            error: error.to_string(),
        })?;
    validate_module_package_metadata(slug, &package_manifest)?;
    if let Some(module_root) = module_root_path(spec) {
        validate_module_ui_wiring(slug, &module_root, &package_manifest)?;
    }

    Ok(merge_module_package_manifest(
        spec.clone(),
        package_manifest,
    ))
}

fn first_party_module(
    crate_name: &str,
    path: &str,
    required: bool,
    depends_on: &[&str],
    recommended_admin_surfaces: &[&str],
    showcase_admin_surfaces: &[&str],
) -> ManifestModuleSpec {
    ManifestModuleSpec {
        source: "path".to_string(),
        crate_name: crate_name.to_string(),
        path: Some(path.to_string()),
        required,
        depends_on: depends_on
            .iter()
            .map(|value| (*value).to_string())
            .collect(),
        ownership: "first_party".to_string(),
        trust_level: if required {
            "core".to_string()
        } else {
            "verified".to_string()
        },
        rustok_min_version: None,
        rustok_max_version: None,
        recommended_admin_surfaces: recommended_admin_surfaces
            .iter()
            .map(|value| (*value).to_string())
            .collect(),
        showcase_admin_surfaces: showcase_admin_surfaces
            .iter()
            .map(|value| (*value).to_string())
            .collect(),
        ..Default::default()
    }
}

fn builtin_module_catalog() -> HashMap<&'static str, ManifestModuleSpec> {
    HashMap::from([
        (
            "index",
            first_party_module(
                "rustok-index",
                "crates/rustok-index",
                true,
                &[],
                &["leptos-admin"],
                &[],
            ),
        ),
        (
            "outbox",
            first_party_module(
                "rustok-outbox",
                "crates/rustok-outbox",
                true,
                &[],
                &["leptos-admin"],
                &[],
            ),
        ),
        (
            "content",
            first_party_module(
                "rustok-content",
                "crates/rustok-content",
                false,
                &[],
                &["leptos-admin"],
                &[],
            ),
        ),
        (
            "cart",
            first_party_module(
                "rustok-cart",
                "crates/rustok-cart",
                false,
                &[],
                &["leptos-admin"],
                &[],
            ),
        ),
        (
            "customer",
            first_party_module(
                "rustok-customer",
                "crates/rustok-customer",
                false,
                &[],
                &["leptos-admin"],
                &[],
            ),
        ),
        (
            "product",
            first_party_module(
                "rustok-product",
                "crates/rustok-product",
                false,
                &[],
                &["leptos-admin"],
                &[],
            ),
        ),
        (
            "region",
            first_party_module(
                "rustok-region",
                "crates/rustok-region",
                false,
                &[],
                &["leptos-admin"],
                &[],
            ),
        ),
        (
            "pricing",
            first_party_module(
                "rustok-pricing",
                "crates/rustok-pricing",
                false,
                &["product"],
                &["leptos-admin"],
                &[],
            ),
        ),
        (
            "inventory",
            first_party_module(
                "rustok-inventory",
                "crates/rustok-inventory",
                false,
                &["product"],
                &["leptos-admin"],
                &[],
            ),
        ),
        (
            "order",
            first_party_module(
                "rustok-order",
                "crates/rustok-order",
                false,
                &[],
                &["leptos-admin"],
                &[],
            ),
        ),
        (
            "payment",
            first_party_module(
                "rustok-payment",
                "crates/rustok-payment",
                false,
                &[],
                &["leptos-admin"],
                &[],
            ),
        ),
        (
            "fulfillment",
            first_party_module(
                "rustok-fulfillment",
                "crates/rustok-fulfillment",
                false,
                &[],
                &["leptos-admin"],
                &[],
            ),
        ),
        (
            "commerce",
            first_party_module(
                "rustok-commerce",
                "crates/rustok-commerce",
                false,
                &[
                    "cart",
                    "customer",
                    "product",
                    "region",
                    "pricing",
                    "inventory",
                    "order",
                    "payment",
                    "fulfillment",
                ],
                &["leptos-admin"],
                &[],
            ),
        ),
        (
            "comments",
            first_party_module(
                "rustok-comments",
                "crates/rustok-comments",
                false,
                &[],
                &["leptos-admin"],
                &[],
            ),
        ),
        (
            "blog",
            first_party_module(
                "rustok-blog",
                "crates/rustok-blog",
                false,
                &["content", "comments"],
                &["leptos-admin"],
                &["next-admin"],
            ),
        ),
        (
            "forum",
            first_party_module(
                "rustok-forum",
                "crates/rustok-forum",
                false,
                &["content"],
                &["leptos-admin"],
                &[],
            ),
        ),
        (
            "pages",
            first_party_module(
                "rustok-pages",
                "crates/rustok-pages",
                false,
                &["content"],
                &["leptos-admin"],
                &[],
            ),
        ),
        (
            "tenant",
            first_party_module(
                "rustok-tenant",
                "crates/rustok-tenant",
                true,
                &[],
                &["leptos-admin"],
                &[],
            ),
        ),
        (
            "rbac",
            first_party_module(
                "rustok-rbac",
                "crates/rustok-rbac",
                true,
                &[],
                &["leptos-admin"],
                &[],
            ),
        ),
    ])
}

fn builtin_default_enabled() -> HashSet<&'static str> {
    HashSet::from([
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

fn is_valid_module_ownership(value: &str) -> bool {
    matches!(value, "first_party" | "third_party")
}

fn is_valid_trust_level(value: &str) -> bool {
    matches!(value, "core" | "verified" | "unverified" | "private")
}

fn is_valid_module_slug(value: &str) -> bool {
    !value.is_empty()
        && value
            .chars()
            .all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '-')
}

fn is_valid_admin_surface(value: &str) -> bool {
    !value.is_empty()
        && value
            .chars()
            .all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '-')
}

fn normalize_version_req(raw: &str, is_max: bool) -> String {
    let trimmed = raw.trim();
    let wildcard = trimmed.replace(".x", ".*").replace(".X", ".*");
    let has_operator = wildcard.contains('<')
        || wildcard.contains('>')
        || wildcard.contains('=')
        || wildcard.contains('~')
        || wildcard.contains('^')
        || wildcard.contains('*')
        || wildcard.contains(',');

    if has_operator {
        return wildcard;
    }

    if is_max {
        format!("<= {wildcard}")
    } else {
        format!(">= {wildcard}")
    }
}

fn current_platform_version() -> Option<Version> {
    Version::parse(env!("CARGO_PKG_VERSION")).ok()
}

fn validate_admin_surfaces(
    slug: &str,
    field: &str,
    surfaces: &[String],
) -> Result<HashSet<String>, ManifestError> {
    let mut normalized = HashSet::new();

    for surface in surfaces {
        let surface = surface.trim();
        if !is_valid_admin_surface(surface) {
            return Err(ManifestError::InvalidModuleAdminSurface {
                slug: slug.to_string(),
                field: field.to_string(),
                value: surface.to_string(),
            });
        }

        normalized.insert(surface.to_string());
    }

    Ok(normalized)
}

fn validate_catalog_metadata(slug: &str, spec: &ManifestModuleSpec) -> Result<(), ManifestError> {
    let ownership = spec.ownership.trim();
    if !is_valid_module_ownership(ownership) {
        return Err(ManifestError::InvalidModuleOwnership {
            slug: slug.to_string(),
            value: ownership.to_string(),
        });
    }

    let trust_level = spec.trust_level.trim();
    if !is_valid_trust_level(trust_level) {
        return Err(ManifestError::InvalidModuleTrustLevel {
            slug: slug.to_string(),
            value: trust_level.to_string(),
        });
    }

    let recommended = validate_admin_surfaces(
        slug,
        "recommended_admin_surfaces",
        &spec.recommended_admin_surfaces,
    )?;
    let showcase = validate_admin_surfaces(
        slug,
        "showcase_admin_surfaces",
        &spec.showcase_admin_surfaces,
    )?;

    if let Some(surface) = recommended.intersection(&showcase).next() {
        return Err(ManifestError::ConflictingModuleAdminSurface {
            slug: slug.to_string(),
            surface: surface.clone(),
        });
    }

    validate_marketplace_metadata(slug, spec)?;

    Ok(())
}

fn validate_marketplace_metadata(
    slug: &str,
    spec: &ManifestModuleSpec,
) -> Result<(), ManifestError> {
    if let Some(description) = spec
        .description
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        if description.chars().count() < 20 {
            return Err(ManifestError::InvalidModuleMarketplaceMetadata {
                slug: slug.to_string(),
                field: "description".to_string(),
                reason: "must be at least 20 characters".to_string(),
            });
        }
    }

    if let Some(icon_url) = spec
        .icon_url
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        validate_marketplace_asset_url(slug, "icon", icon_url, &["svg"])?;
    }

    if let Some(banner_url) = spec
        .banner_url
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        validate_marketplace_asset_url(
            slug,
            "banner",
            banner_url,
            &["png", "jpg", "jpeg", "webp", "svg"],
        )?;
    }

    for (index, screenshot) in spec.screenshots.iter().enumerate() {
        let screenshot = screenshot.trim();
        if screenshot.is_empty() {
            continue;
        }

        validate_marketplace_asset_url(
            slug,
            &format!("screenshots[{index}]"),
            screenshot,
            &["png", "jpg", "jpeg", "webp", "svg"],
        )?;
    }

    Ok(())
}

fn validate_marketplace_asset_url(
    slug: &str,
    field: &str,
    value: &str,
    allowed_extensions: &[&str],
) -> Result<(), ManifestError> {
    let parsed = reqwest::Url::parse(value).map_err(|error| {
        ManifestError::InvalidModuleMarketplaceMetadata {
            slug: slug.to_string(),
            field: field.to_string(),
            reason: format!("must be a valid absolute URL: {error}"),
        }
    })?;

    if !matches!(parsed.scheme(), "http" | "https") {
        return Err(ManifestError::InvalidModuleMarketplaceMetadata {
            slug: slug.to_string(),
            field: field.to_string(),
            reason: "must use http or https".to_string(),
        });
    }

    let path = parsed.path();
    let has_allowed_extension = allowed_extensions.iter().any(|extension| {
        path.rsplit('/')
            .next()
            .map(|segment| segment.to_ascii_lowercase())
            .is_some_and(|segment| segment.ends_with(&format!(".{extension}")))
    });

    if !has_allowed_extension {
        let allowed = allowed_extensions
            .iter()
            .map(|extension| format!(".{extension}"))
            .collect::<Vec<_>>()
            .join(", ");
        return Err(ManifestError::InvalidModuleMarketplaceMetadata {
            slug: slug.to_string(),
            field: field.to_string(),
            reason: format!("must point to one of: {allowed}"),
        });
    }

    Ok(())
}

pub struct ManifestManager;

impl ManifestManager {
    pub fn manifest_path() -> PathBuf {
        default_manifest_path()
    }

    pub fn manifest_ref() -> String {
        Self::manifest_path().display().to_string()
    }

    pub fn load() -> Result<ModulesManifest, ManifestError> {
        Self::load_from_path(Self::manifest_path())
    }

    pub fn load_from_path(path: impl AsRef<Path>) -> Result<ModulesManifest, ManifestError> {
        let path = path.as_ref();
        let raw = std::fs::read_to_string(path).map_err(|error| ManifestError::Read {
            path: path.display().to_string(),
            error: error.to_string(),
        })?;

        toml::from_str(&raw).map_err(|error| ManifestError::Parse {
            path: path.display().to_string(),
            error: error.to_string(),
        })
    }

    pub fn save(manifest: &ModulesManifest) -> Result<(), ManifestError> {
        Self::save_to_path(Self::manifest_path(), manifest)
    }

    pub fn save_to_path(
        path: impl AsRef<Path>,
        manifest: &ModulesManifest,
    ) -> Result<(), ManifestError> {
        let path = path.as_ref();
        let serialized =
            toml::to_string_pretty(manifest).map_err(|error| ManifestError::Write {
                path: path.display().to_string(),
                error: error.to_string(),
            })?;

        std::fs::write(path, serialized).map_err(|error| ManifestError::Write {
            path: path.display().to_string(),
            error: error.to_string(),
        })
    }

    pub fn installed_modules(manifest: &ModulesManifest) -> Vec<InstalledManifestModule> {
        let mut modules = manifest
            .modules
            .iter()
            .map(|(slug, spec)| InstalledManifestModule {
                slug: slug.clone(),
                source: spec.source.clone(),
                crate_name: spec.crate_name.clone(),
                version: spec.version.clone(),
                git: spec.git.clone(),
                rev: spec.rev.clone(),
                path: spec.path.clone(),
                required: spec.required,
                depends_on: spec.depends_on.clone(),
            })
            .collect::<Vec<_>>();

        modules.sort_by(|left, right| left.slug.cmp(&right.slug));
        modules
    }

    pub fn catalog_modules(
        manifest: &ModulesManifest,
    ) -> Result<Vec<CatalogManifestModule>, ManifestError> {
        let mut catalog = builtin_module_catalog()
            .into_iter()
            .map(|(slug, spec)| (slug.to_string(), spec))
            .collect::<HashMap<_, _>>();

        for (slug, spec) in &manifest.modules {
            catalog.insert(slug.clone(), spec.clone());
        }

        let mut modules = catalog
            .into_iter()
            .map(|(slug, spec)| {
                let path = module_package_manifest_path(&spec);
                let module_root_exists = match module_root_path(&spec).as_ref() {
                    Some(path) => path.exists(),
                    None => false,
                };
                let manifest_exists = match path.as_ref() {
                    Some(path) => path.exists(),
                    None => false,
                };
                if spec.source == "path" && module_root_exists && !manifest_exists {
                    return Err(ManifestError::MissingModulePackageManifest {
                        slug: slug.clone(),
                        path: path
                            .as_ref()
                            .map(|path| path.display().to_string())
                            .unwrap_or_else(|| "<unknown>".to_string()),
                    });
                }

                let ui_surface_flags = module_package_ui_surface_flags(&spec)?;
                let spec = apply_module_package_manifest(&slug, &spec)?;
                validate_catalog_metadata(&slug, &spec)?;
                let ui_classification = resolved_catalog_module_ui_classification(
                    &slug,
                    spec.ui_classification.as_deref(),
                    ui_surface_flags.has_admin_ui,
                    ui_surface_flags.has_storefront_ui,
                )?;

                Ok(CatalogManifestModule {
                    slug: slug.to_string(),
                    source: spec.source,
                    crate_name: spec.crate_name,
                    name: spec.name,
                    category: spec.category,
                    tags: spec.tags,
                    icon_url: spec.icon_url,
                    banner_url: spec.banner_url,
                    screenshots: spec.screenshots,
                    version: spec.version,
                    description: spec.description,
                    git: spec.git,
                    rev: spec.rev,
                    path: spec.path,
                    required: spec.required,
                    depends_on: spec.depends_on,
                    ownership: spec.ownership,
                    trust_level: spec.trust_level,
                    rustok_min_version: spec.rustok_min_version,
                    rustok_max_version: spec.rustok_max_version,
                    publisher: None,
                    checksum_sha256: None,
                    signature: None,
                    versions: Vec::new(),
                    has_admin_ui: ui_surface_flags.has_admin_ui,
                    has_storefront_ui: ui_surface_flags.has_storefront_ui,
                    ui_classification,
                    recommended_admin_surfaces: spec.recommended_admin_surfaces,
                    showcase_admin_surfaces: spec.showcase_admin_surfaces,
                    settings_schema: spec.settings_schema,
                })
            })
            .collect::<Result<Vec<_>, ManifestError>>()?;

        modules.sort_by(|left, right| left.slug.cmp(&right.slug));
        Ok(modules)
    }

    pub fn build_modules(manifest: &ModulesManifest) -> HashMap<String, BuildModuleSpec> {
        manifest
            .modules
            .iter()
            .map(|(slug, spec)| {
                (
                    slug.clone(),
                    BuildModuleSpec {
                        source: spec.source.clone(),
                        crate_name: spec.crate_name.clone(),
                        version: spec.version.clone(),
                        git: spec.git.clone(),
                        rev: spec.rev.clone(),
                        path: spec.path.clone(),
                    },
                )
            })
            .collect()
    }

    pub fn deployment_profile(manifest: &ModulesManifest) -> DeploymentProfile {
        match (
            manifest.build.server.embed_admin,
            manifest.build.server.embed_storefront,
        ) {
            (true, true) => DeploymentProfile::Monolith,
            (true, false) => DeploymentProfile::ServerWithAdmin,
            (false, true) => DeploymentProfile::ServerWithStorefront,
            (false, false) => DeploymentProfile::HeadlessApi,
        }
    }

    pub fn deployment_surface_contract(manifest: &ModulesManifest) -> DeploymentSurfaceContract {
        DeploymentSurfaceContract {
            profile: Self::deployment_profile(manifest),
            embed_admin: manifest.build.server.embed_admin,
            embed_storefront: manifest.build.server.embed_storefront,
        }
    }

    pub fn build_execution_plan(manifest: &ModulesManifest) -> BuildExecutionPlan {
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

        let admin_build = admin_frontend_build_plan(manifest, &cargo_profile);
        let storefront_build =
            storefront_frontend_build_plan(manifest, &cargo_profile, cargo_target.as_deref());

        BuildExecutionPlan {
            cargo_package,
            cargo_profile,
            cargo_target,
            cargo_features,
            cargo_command: command_parts.join(" "),
            admin_build,
            storefront_build,
        }
    }

    pub fn install_builtin_module(
        manifest: &mut ModulesManifest,
        slug: &str,
        version: Option<String>,
    ) -> Result<ManifestDiff, ManifestError> {
        if manifest.modules.contains_key(slug) {
            return Err(ManifestError::ModuleAlreadyInstalled(slug.to_string()));
        }

        let mut spec = builtin_module_catalog()
            .remove(slug)
            .ok_or_else(|| ManifestError::UnknownModule(slug.to_string()))?;

        if let Some(version) = version {
            let version = version.trim();
            if version.is_empty() {
                return Err(ManifestError::InvalidVersion);
            }
            spec.version = Some(version.to_string());
        }

        manifest.modules.insert(slug.to_string(), spec);

        if builtin_default_enabled().contains(slug)
            && !manifest
                .settings
                .default_enabled
                .iter()
                .any(|item| item == slug)
        {
            manifest.settings.default_enabled.push(slug.to_string());
            manifest.settings.default_enabled.sort();
        }

        Self::validate(manifest)?;
        Ok(ManifestDiff::added(
            slug,
            manifest
                .modules
                .get(slug)
                .and_then(|spec| spec.version.as_deref()),
        ))
    }

    pub fn uninstall_module(
        manifest: &mut ModulesManifest,
        slug: &str,
    ) -> Result<ManifestDiff, ManifestError> {
        let spec = manifest
            .modules
            .get(slug)
            .cloned()
            .ok_or_else(|| ManifestError::ModuleNotInstalled(slug.to_string()))?;

        if spec.required {
            return Err(ManifestError::RequiredModule(slug.to_string()));
        }

        let dependents = manifest
            .modules
            .iter()
            .filter(|(candidate_slug, _)| candidate_slug.as_str() != slug)
            .filter(|(_, candidate_spec)| candidate_spec.depends_on.iter().any(|dep| dep == slug))
            .map(|(candidate_slug, _)| candidate_slug.clone())
            .collect::<Vec<_>>();

        if !dependents.is_empty() {
            return Err(ManifestError::HasDependents {
                slug: slug.to_string(),
                dependents: dependents.join(", "),
            });
        }

        manifest.modules.remove(slug);
        manifest
            .settings
            .default_enabled
            .retain(|item| item != slug);
        Self::validate(manifest)?;
        Ok(ManifestDiff::removed(slug))
    }

    pub fn upgrade_module(
        manifest: &mut ModulesManifest,
        slug: &str,
        version: String,
    ) -> Result<ManifestDiff, ManifestError> {
        let version = version.trim();
        if version.is_empty() {
            return Err(ManifestError::InvalidVersion);
        }

        let spec = manifest
            .modules
            .get_mut(slug)
            .ok_or_else(|| ManifestError::ModuleNotInstalled(slug.to_string()))?;

        if spec.version.as_deref() == Some(version) {
            return Err(ManifestError::VersionUnchanged(
                slug.to_string(),
                version.to_string(),
            ));
        }

        spec.version = Some(version.to_string());
        Self::validate(manifest)?;
        Ok(ManifestDiff::upgraded(slug, version))
    }

    pub fn validate_module_settings(
        module_slug: &str,
        settings: serde_json::Value,
    ) -> Result<serde_json::Value, ManifestError> {
        let manifest = Self::load()?;
        let resolved_specs = resolve_module_specs(&manifest)?;

        let schema = if let Some(spec) = resolved_specs.get(module_slug) {
            spec.settings_schema.clone()
        } else if let Some(spec) = builtin_module_catalog().remove(module_slug) {
            apply_module_package_manifest(module_slug, &spec)?.settings_schema
        } else {
            HashMap::new()
        };

        normalize_module_settings(module_slug, &schema, settings)
    }

    pub fn validate(manifest: &ModulesManifest) -> Result<(), ManifestError> {
        let resolved_specs = resolve_module_specs(manifest)?;

        let installed = resolved_specs.keys().cloned().collect::<HashSet<_>>();

        let missing_defaults = manifest
            .settings
            .default_enabled
            .iter()
            .filter(|slug| !installed.contains(*slug))
            .cloned()
            .collect::<Vec<_>>();

        if !missing_defaults.is_empty() {
            return Err(ManifestError::UnknownDefaultEnabled(
                missing_defaults.join(", "),
            ));
        }

        let mut ordered_slugs = resolved_specs.keys().cloned().collect::<Vec<_>>();
        ordered_slugs.sort();

        for slug in ordered_slugs {
            let spec = resolved_specs
                .get(&slug)
                .expect("resolved module slug must exist");
            let missing = spec
                .depends_on
                .iter()
                .filter(|dependency| !installed.contains(*dependency))
                .cloned()
                .collect::<Vec<_>>();

            if !missing.is_empty() {
                return Err(ManifestError::MissingDependencies {
                    slug: slug.clone(),
                    missing: missing.join(", "),
                });
            }

            for conflict in &spec.conflicts_with {
                if installed.contains(conflict) {
                    return Err(ManifestError::ConflictingModule {
                        slug: slug.clone(),
                        conflicts_with: conflict.clone(),
                    });
                }
            }

            for (dependency, raw_req) in &spec.dependency_version_reqs {
                let Some(dependency_spec) = resolved_specs.get(dependency) else {
                    continue;
                };

                let installed_version = dependency_spec.version.as_deref().ok_or_else(|| {
                    ManifestError::MissingDependencyVersion {
                        slug: slug.clone(),
                        dependency: dependency.clone(),
                    }
                })?;
                let installed_version = Version::parse(installed_version).map_err(|_| {
                    ManifestError::InvalidModuleVersion {
                        slug: dependency.clone(),
                        value: installed_version.to_string(),
                    }
                })?;
                let version_req = VersionReq::parse(raw_req).map_err(|_| {
                    ManifestError::InvalidDependencyVersionReq {
                        slug: slug.clone(),
                        dependency: dependency.clone(),
                        value: raw_req.clone(),
                    }
                })?;

                if !version_req.matches(&installed_version) {
                    return Err(ManifestError::IncompatibleDependencyVersion {
                        slug: slug.clone(),
                        dependency: dependency.clone(),
                        required: raw_req.clone(),
                        installed: installed_version.to_string(),
                    });
                }
            }

            if let Some(current_version) = current_platform_version() {
                let min_ok = spec
                    .rustok_min_version
                    .as_deref()
                    .map(|raw| normalize_version_req(raw, false))
                    .map(|req| VersionReq::parse(&req))
                    .transpose()
                    .map_err(|_| ManifestError::IncompatibleRustokVersion {
                        slug: slug.clone(),
                        current_version: current_version.to_string(),
                        minimum: spec.rustok_min_version.clone(),
                        maximum: spec.rustok_max_version.clone(),
                    })?
                    .is_none_or(|req| req.matches(&current_version));
                let max_ok = spec
                    .rustok_max_version
                    .as_deref()
                    .map(|raw| normalize_version_req(raw, true))
                    .map(|req| VersionReq::parse(&req))
                    .transpose()
                    .map_err(|_| ManifestError::IncompatibleRustokVersion {
                        slug: slug.clone(),
                        current_version: current_version.to_string(),
                        minimum: spec.rustok_min_version.clone(),
                        maximum: spec.rustok_max_version.clone(),
                    })?
                    .is_none_or(|req| req.matches(&current_version));

                if !(min_ok && max_ok) {
                    return Err(ManifestError::IncompatibleRustokVersion {
                        slug: slug.clone(),
                        current_version: current_version.to_string(),
                        minimum: spec.rustok_min_version.clone(),
                        maximum: spec.rustok_max_version.clone(),
                    });
                }
            }
        }

        validate_build_surfaces(manifest)?;

        Ok(())
    }

    pub fn validate_with_registry(
        manifest: &ModulesManifest,
        registry: &ModuleRegistry,
    ) -> Result<(), ManifestError> {
        let resolved_specs = resolve_module_specs(manifest)?;
        let missing_in_registry: Vec<String> = manifest
            .modules
            .iter()
            .filter(|(slug, _)| {
                resolved_specs
                    .get(*slug)
                    .and_then(|spec| spec.entry_type.as_ref())
                    .is_some()
            })
            .map(|(slug, _)| slug)
            .filter(|slug| !registry.contains(slug))
            .cloned()
            .collect();

        if !missing_in_registry.is_empty() {
            return Err(ManifestError::MissingInRegistry(
                missing_in_registry.join(", "),
            ));
        }

        let required_mismatch: Vec<String> = registry
            .list()
            .into_iter()
            .filter_map(|module| {
                manifest.modules.get(module.slug()).map(|spec| {
                    (
                        module.slug(),
                        spec.required,
                        registry.is_core(module.slug()),
                    )
                })
            })
            .filter_map(|(slug, required, is_core)| {
                if required == is_core {
                    None
                } else {
                    Some(format!("{slug} (required={required}, core={is_core})"))
                }
            })
            .collect();

        if !required_mismatch.is_empty() {
            return Err(ManifestError::RequiredMismatch(
                required_mismatch.join(", "),
            ));
        }

        let dependency_mismatch: Vec<String> = registry
            .list()
            .into_iter()
            .filter_map(|module| {
                resolved_specs.get(module.slug()).and_then(|spec| {
                    let manifest_deps = normalize_deps(&spec.depends_on);
                    let registry_deps: HashSet<String> = module
                        .dependencies()
                        .iter()
                        .map(|dep| dep.to_string())
                        .collect();

                    if manifest_deps == registry_deps {
                        None
                    } else {
                        Some(format!(
                            "{} (manifest={:?}, registry={:?})",
                            module.slug(),
                            manifest_deps,
                            registry_deps
                        ))
                    }
                })
            })
            .collect();

        if !dependency_mismatch.is_empty() {
            return Err(ManifestError::DependencyMismatch(
                dependency_mismatch.join(", "),
            ));
        }

        Ok(())
    }
}

fn resolve_module_specs(
    manifest: &ModulesManifest,
) -> Result<HashMap<String, ManifestModuleSpec>, ManifestError> {
    let mut resolved_specs = HashMap::new();
    for (slug, spec) in &manifest.modules {
        let resolved = apply_module_package_manifest(slug, spec)?;
        resolved_specs.insert(slug.clone(), resolved);
    }

    Ok(resolved_specs)
}

fn validate_build_surfaces(manifest: &ModulesManifest) -> Result<(), ManifestError> {
    if !manifest.build.server.embed_admin && !manifest.build.admin.stack.trim().is_empty() {
        if manifest.build.admin.public_url.trim().is_empty() {
            return Err(ManifestError::InvalidBuildSurface(
                "Standalone admin requires build.admin.public_url".to_string(),
            ));
        }

        if manifest.build.admin.redirect_uris.is_empty() {
            return Err(ManifestError::InvalidBuildSurface(
                "Standalone admin requires at least one build.admin.redirect_uris entry"
                    .to_string(),
            ));
        }

        validate_urls(
            &manifest.build.admin.redirect_uris,
            "build.admin.redirect_uris",
        )?;
        validate_url(&manifest.build.admin.public_url, "build.admin.public_url")?;
    }

    let mut storefront_ids = HashSet::new();
    for storefront in &manifest.build.storefront {
        if storefront.id.trim().is_empty() {
            return Err(ManifestError::InvalidBuildSurface(
                "Each build.storefront entry requires a non-empty id".to_string(),
            ));
        }

        if !storefront_ids.insert(storefront.id.clone()) {
            return Err(ManifestError::InvalidBuildSurface(format!(
                "Duplicate storefront id '{}'",
                storefront.id
            )));
        }

        let is_standalone = !manifest.build.server.embed_storefront || storefront.stack == "next";
        if !is_standalone {
            continue;
        }

        if storefront.public_url.trim().is_empty() {
            return Err(ManifestError::InvalidBuildSurface(format!(
                "Standalone storefront '{}' requires public_url",
                storefront.id
            )));
        }

        if storefront.redirect_uris.is_empty() {
            return Err(ManifestError::InvalidBuildSurface(format!(
                "Standalone storefront '{}' requires at least one redirect_uri",
                storefront.id
            )));
        }

        validate_url(
            &storefront.public_url,
            &format!("build.storefront[{}].public_url", storefront.id),
        )?;
        validate_urls(
            &storefront.redirect_uris,
            &format!("build.storefront[{}].redirect_uris", storefront.id),
        )?;
    }

    Ok(())
}

fn validate_urls(urls: &[String], field: &str) -> Result<(), ManifestError> {
    for value in urls {
        validate_url(value, field)?;
    }

    Ok(())
}

fn validate_url(value: &str, field: &str) -> Result<(), ManifestError> {
    reqwest::Url::parse(value).map_err(|error| {
        ManifestError::InvalidBuildSurface(format!(
            "{field} contains invalid URL '{value}': {error}"
        ))
    })?;
    Ok(())
}

pub fn validate_registry_vs_manifest(registry: &ModuleRegistry) -> ServerResult<()> {
    let manifest = ManifestManager::load().map_err(|error| {
        ServerError::BadRequest(format!("modules.toml validation failed: {error}"))
    })?;

    ManifestManager::validate(&manifest)
        .and_then(|_| ManifestManager::validate_with_registry(&manifest, registry))
        .map_err(|error| {
            ServerError::BadRequest(format!("modules.toml validation failed: {error}"))
        })
}

#[cfg(test)]
mod tests {
    use super::{
        builtin_module_catalog, ManifestError, ManifestManager, ModulesManifest,
        StorefrontBuildConfig,
    };
    use crate::models::build::DeploymentProfile;
    use crate::modules::{build_registry, FrontendArtifactKind, FrontendBuildTool};
    use serial_test::serial;
    use std::collections::HashMap;
    use tempfile::tempdir;

    fn manifest_with_modules(slugs: &[&str]) -> ModulesManifest {
        let catalog = builtin_module_catalog();
        let modules = slugs
            .iter()
            .map(|slug| ((*slug).to_string(), catalog.get(slug).unwrap().clone()))
            .collect::<HashMap<_, _>>();

        ModulesManifest {
            schema: 2,
            app: "rustok-server".to_string(),
            modules,
            ..Default::default()
        }
    }

    fn write_module_manifest(crate_dir: &std::path::Path, contents: &str) {
        std::fs::create_dir_all(crate_dir).unwrap();
        std::fs::write(crate_dir.join("rustok-module.toml"), contents).unwrap();
    }

    fn write_surface_manifest(crate_dir: &std::path::Path, surface: &str, crate_name: &str) {
        let surface_dir = crate_dir.join(surface);
        std::fs::create_dir_all(&surface_dir).unwrap();
        std::fs::write(
            surface_dir.join("Cargo.toml"),
            format!(
                "[package]\nname = \"{crate_name}\"\nversion = \"0.1.0\"\nedition = \"2021\"\n"
            ),
        )
        .unwrap();
    }

    fn write_locale_bundle(dir: &std::path::Path, locale: &str, value: &str) {
        std::fs::create_dir_all(dir).unwrap();
        std::fs::write(
            dir.join(format!("{locale}.json")),
            format!("{{\"title\":\"{value}\"}}"),
        )
        .unwrap();
    }

    #[test]
    #[serial]
    fn derives_deployment_surface_contract_from_build_server_flags() {
        let mut manifest = ModulesManifest::default();

        let headless = ManifestManager::deployment_surface_contract(&manifest);
        assert_eq!(headless.profile, DeploymentProfile::HeadlessApi);
        assert!(!headless.embed_admin);
        assert!(!headless.embed_storefront);

        manifest.build.server.embed_admin = true;
        let server_with_admin = ManifestManager::deployment_surface_contract(&manifest);
        assert_eq!(
            server_with_admin.profile,
            DeploymentProfile::ServerWithAdmin
        );
        assert!(server_with_admin.embed_admin);
        assert!(!server_with_admin.embed_storefront);

        manifest.build.server.embed_storefront = true;
        let monolith = ManifestManager::deployment_surface_contract(&manifest);
        assert_eq!(monolith.profile, DeploymentProfile::Monolith);
        assert!(monolith.embed_admin);
        assert!(monolith.embed_storefront);

        manifest.build.server.embed_admin = false;
        let server_with_storefront = ManifestManager::deployment_surface_contract(&manifest);
        assert_eq!(
            server_with_storefront.profile,
            DeploymentProfile::ServerWithStorefront
        );
        assert!(!server_with_storefront.embed_admin);
        assert!(server_with_storefront.embed_storefront);
    }

    #[test]
    #[serial]
    fn derives_build_execution_plan_from_manifest() {
        let mut manifest = ModulesManifest {
            app: "rustok-server".to_string(),
            ..ModulesManifest::default()
        };
        manifest.build.profile = "release".to_string();
        manifest.build.target = "x86_64-unknown-linux-gnu".to_string();
        manifest.build.server.embed_admin = true;
        manifest.build.server.embed_storefront = true;

        let plan = ManifestManager::build_execution_plan(&manifest);

        assert_eq!(plan.cargo_package, "rustok-server");
        assert_eq!(plan.cargo_profile, "release");
        assert_eq!(
            plan.cargo_target.as_deref(),
            Some("x86_64-unknown-linux-gnu")
        );
        assert_eq!(
            plan.cargo_features,
            vec!["embed-admin".to_string(), "embed-storefront".to_string()]
        );
        assert_eq!(
            plan.cargo_command,
            "cargo build -p rustok-server --release --target x86_64-unknown-linux-gnu --features embed-admin,embed-storefront"
        );
        let admin_build = plan.admin_build.expect("expected admin build plan");
        assert_eq!(admin_build.surface, "admin");
        assert_eq!(admin_build.tool, FrontendBuildTool::Trunk);
        assert_eq!(admin_build.workspace_path, "apps/admin");
        assert_eq!(admin_build.artifact_path, "apps/admin/dist");
        assert_eq!(admin_build.artifact_kind, FrontendArtifactKind::Directory);
        assert_eq!(admin_build.command, "trunk build --release");

        let storefront_build = plan
            .storefront_build
            .expect("expected storefront build plan");
        assert_eq!(storefront_build.surface, "storefront");
        assert_eq!(storefront_build.tool, FrontendBuildTool::Cargo);
        assert_eq!(storefront_build.package, "rustok-storefront");
        assert_eq!(storefront_build.workspace_path, ".");
        assert_eq!(
            storefront_build.target.as_deref(),
            Some("x86_64-unknown-linux-gnu")
        );
        assert_eq!(
            storefront_build.command,
            "cargo build -p rustok-storefront --release --target x86_64-unknown-linux-gnu"
        );
    }

    #[test]
    #[serial]
    fn rejects_standalone_admin_without_redirect_uris() {
        let mut manifest = ModulesManifest::default();
        manifest.build.server.embed_admin = false;
        manifest.build.admin.stack = "next".to_string();
        manifest.build.admin.public_url = "http://localhost:3001".to_string();

        let result = ManifestManager::validate(&manifest);

        assert!(matches!(
            result,
            Err(ManifestError::InvalidBuildSurface(message))
            if message.contains("build.admin.redirect_uris")
        ));
    }

    #[test]
    #[serial]
    fn allows_standalone_surfaces_with_public_oauth_config() {
        let mut manifest = ModulesManifest::default();
        manifest.build.server.embed_admin = false;
        manifest.build.server.embed_storefront = false;
        manifest.build.admin.stack = "next".to_string();
        manifest.build.admin.public_url = "http://localhost:3001".to_string();
        manifest.build.admin.redirect_uris =
            vec!["http://localhost:3001/auth/callback".to_string()];
        manifest.build.storefront = vec![StorefrontBuildConfig {
            id: "default".to_string(),
            stack: "next".to_string(),
            public_url: "http://localhost:3000".to_string(),
            redirect_uris: vec!["http://localhost:3000/auth/callback".to_string()],
        }];

        let result = ManifestManager::validate(&manifest);

        assert!(result.is_ok(), "expected valid standalone surface config");
    }

    #[test]
    #[serial]
    fn allows_registry_superset_when_optional_module_is_removed_from_manifest() {
        let registry = build_registry();
        let manifest = manifest_with_modules(&[
            "index",
            "outbox",
            "content",
            "cart",
            "customer",
            "product",
            "region",
            "pricing",
            "inventory",
            "order",
            "payment",
            "fulfillment",
            "commerce",
            "pages",
            "tenant",
            "rbac",
        ]);

        let result = ManifestManager::validate_with_registry(&manifest, &registry);
        assert!(
            result.is_ok(),
            "optional registry modules may be absent from manifest"
        );
    }

    #[test]
    #[serial]
    fn uninstall_removes_default_enabled_entry() {
        let mut manifest = manifest_with_modules(&[
            "index",
            "outbox",
            "content",
            "cart",
            "customer",
            "product",
            "region",
            "pricing",
            "inventory",
            "order",
            "payment",
            "fulfillment",
            "commerce",
            "pages",
            "tenant",
            "rbac",
        ]);
        manifest.settings.default_enabled = vec![
            "content".to_string(),
            "cart".to_string(),
            "customer".to_string(),
            "product".to_string(),
            "region".to_string(),
            "pricing".to_string(),
            "inventory".to_string(),
            "order".to_string(),
            "payment".to_string(),
            "fulfillment".to_string(),
            "commerce".to_string(),
            "pages".to_string(),
        ];

        ManifestManager::uninstall_module(&mut manifest, "pages").unwrap();

        assert!(!manifest
            .settings
            .default_enabled
            .iter()
            .any(|slug| slug == "pages"));
    }

    #[test]
    #[serial]
    fn install_builtin_module_restores_catalog_defaults() {
        let mut manifest = manifest_with_modules(&["index", "outbox", "content", "tenant", "rbac"]);

        ManifestManager::install_builtin_module(&mut manifest, "pages", Some("1.2.0".to_string()))
            .unwrap();

        assert!(manifest.modules.contains_key("pages"));
        assert!(manifest
            .settings
            .default_enabled
            .iter()
            .any(|slug| slug == "pages"));
    }

    #[test]
    #[serial]
    fn catalog_modules_overlay_metadata_from_rustok_module_manifest() {
        let temp = tempdir().unwrap();
        let manifest_path = temp.path().join("modules.toml");
        let crate_dir = temp.path().join("crates").join("rustok-blog");
        std::fs::create_dir_all(&crate_dir).unwrap();
        std::fs::write(
            crate_dir.join("rustok-module.toml"),
            r#"[module]
ownership = "third_party"
trust_level = "private"
recommended_admin_surfaces = ["custom-admin"]
showcase_admin_surfaces = ["next-admin", "storybook"]

[marketplace]
category = "editorial"
tags = ["editorial", "stories", "news"]
icon = "https://cdn.example.test/modules/blog/icon.svg"
banner = "https://cdn.example.test/modules/blog/banner.png"
screenshots = [
  "https://cdn.example.test/modules/blog/screenshot-1.png",
  "https://cdn.example.test/modules/blog/screenshot-2.png",
]
"#,
        )
        .unwrap();

        let mut manifest = manifest_with_modules(&[
            "index", "outbox", "blog", "content", "comments", "tenant", "rbac",
        ]);
        manifest.modules.get_mut("blog").unwrap().path = Some("crates/rustok-blog".to_string());
        ManifestManager::save_to_path(&manifest_path, &manifest).unwrap();

        let previous = std::env::var("RUSTOK_MODULES_MANIFEST").ok();
        unsafe {
            std::env::set_var("RUSTOK_MODULES_MANIFEST", &manifest_path);
        }

        let result = ManifestManager::catalog_modules(&manifest).unwrap();

        match previous {
            Some(value) => unsafe {
                std::env::set_var("RUSTOK_MODULES_MANIFEST", value);
            },
            None => unsafe {
                std::env::remove_var("RUSTOK_MODULES_MANIFEST");
            },
        }

        let blog = result
            .into_iter()
            .find(|module| module.slug == "blog")
            .unwrap();
        assert_eq!(blog.ownership, "third_party");
        assert_eq!(blog.trust_level, "private");
        assert_eq!(blog.category.as_deref(), Some("editorial"));
        assert_eq!(blog.tags, vec!["editorial", "news", "stories"]);
        assert_eq!(
            blog.icon_url.as_deref(),
            Some("https://cdn.example.test/modules/blog/icon.svg")
        );
        assert_eq!(
            blog.banner_url.as_deref(),
            Some("https://cdn.example.test/modules/blog/banner.png")
        );
        assert_eq!(
            blog.screenshots,
            vec![
                "https://cdn.example.test/modules/blog/screenshot-1.png",
                "https://cdn.example.test/modules/blog/screenshot-2.png",
            ]
        );
        assert_eq!(blog.recommended_admin_surfaces, vec!["custom-admin"]);
        assert_eq!(
            blog.showcase_admin_surfaces,
            vec!["next-admin", "storybook"]
        );
    }

    fn catalog_modules_error_for_blog_manifest(contents: &str) -> ManifestError {
        let temp = tempdir().unwrap();
        let manifest_path = temp.path().join("modules.toml");
        let crate_dir = temp.path().join("crates").join("rustok-blog");
        write_module_manifest(&crate_dir, contents);

        let mut manifest = manifest_with_modules(&[
            "index", "outbox", "blog", "content", "comments", "tenant", "rbac",
        ]);
        manifest.modules.get_mut("blog").unwrap().path = Some("crates/rustok-blog".to_string());
        ManifestManager::save_to_path(&manifest_path, &manifest).unwrap();

        let previous = std::env::var("RUSTOK_MODULES_MANIFEST").ok();
        unsafe {
            std::env::set_var("RUSTOK_MODULES_MANIFEST", &manifest_path);
        }

        let result = ManifestManager::catalog_modules(&manifest);

        match previous {
            Some(value) => unsafe {
                std::env::set_var("RUSTOK_MODULES_MANIFEST", value);
            },
            None => unsafe {
                std::env::remove_var("RUSTOK_MODULES_MANIFEST");
            },
        }

        result.expect_err("catalog metadata should fail validation")
    }

    #[test]
    #[serial]
    fn catalog_modules_reject_short_marketplace_description() {
        let error = catalog_modules_error_for_blog_manifest(
            r#"[module]
description = "Too short"
ownership = "third_party"
trust_level = "private"
"#,
        );

        assert!(matches!(
            error,
            ManifestError::InvalidModuleMarketplaceMetadata {
                slug,
                field,
                reason,
            } if slug == "blog"
                && field == "description"
                && reason.contains("at least 20 characters")
        ));
    }

    #[test]
    #[serial]
    fn catalog_modules_reject_non_svg_marketplace_icon() {
        let error = catalog_modules_error_for_blog_manifest(
            r#"[module]
description = "Blog metadata description is long enough."
ownership = "third_party"
trust_level = "private"

[marketplace]
icon = "https://cdn.example.test/modules/blog/icon.png"
"#,
        );

        assert!(matches!(
            error,
            ManifestError::InvalidModuleMarketplaceMetadata {
                slug,
                field,
                reason,
            } if slug == "blog"
                && field == "icon"
                && reason.contains(".svg")
        ));
    }

    #[test]
    #[serial]
    fn catalog_modules_reject_invalid_marketplace_screenshot_url() {
        let error = catalog_modules_error_for_blog_manifest(
            r#"[module]
description = "Blog metadata description is long enough."
ownership = "third_party"
trust_level = "private"

[marketplace]
screenshots = ["not-a-url"]
"#,
        );

        assert!(matches!(
            error,
            ManifestError::InvalidModuleMarketplaceMetadata {
                slug,
                field,
                reason,
            } if slug == "blog"
                && field == "screenshots[0]"
                && reason.contains("valid absolute URL")
        ));
    }

    #[test]
    #[serial]
    fn validate_overlays_server_entrypoints_from_rustok_module_manifest() {
        let temp = tempdir().unwrap();
        let blog_dir = temp.path().join("crates").join("rustok-blog");
        let content_dir = temp.path().join("crates").join("rustok-content");
        let manifest_path = temp.path().join("modules.toml");
        write_module_manifest(
            &blog_dir,
            r#"[module]
slug = "blog"
name = "Blog"
version = "0.1.0"
ownership = "first_party"
trust_level = "verified"

[crate]
entry_type = "BlogModule"

[provides.graphql]
query = "graphql::BlogQuery"
mutation = "graphql::BlogMutation"

[provides.http]
routes = "controllers::routes"
webhook_routes = "controllers::webhook_routes"
"#,
        );
        write_module_manifest(
            &content_dir,
            r#"[module]
slug = "content"
name = "Content"
version = "0.1.0"
ownership = "first_party"
trust_level = "verified"
"#,
        );

        let mut manifest = manifest_with_modules(&[
            "index", "outbox", "blog", "content", "comments", "tenant", "rbac",
        ]);
        manifest.modules.get_mut("blog").unwrap().path = Some("crates/rustok-blog".to_string());
        manifest.modules.get_mut("content").unwrap().path =
            Some("crates/rustok-content".to_string());
        ManifestManager::save_to_path(&manifest_path, &manifest).unwrap();

        let previous = std::env::var("RUSTOK_MODULES_MANIFEST").ok();
        unsafe {
            std::env::set_var("RUSTOK_MODULES_MANIFEST", &manifest_path);
        }

        let result = ManifestManager::load()
            .and_then(|loaded| {
                ManifestManager::validate(&loaded)?;
                super::resolve_module_specs(&loaded)
            })
            .map(|specs| specs.get("blog").cloned());

        match previous {
            Some(value) => unsafe {
                std::env::set_var("RUSTOK_MODULES_MANIFEST", value);
            },
            None => unsafe {
                std::env::remove_var("RUSTOK_MODULES_MANIFEST");
            },
        }

        let blog = result
            .expect("manifest should validate with explicit server entry points")
            .expect("blog spec");
        assert_eq!(blog.entry_type.as_deref(), Some("rustok_blog::BlogModule"));
        assert_eq!(
            blog.graphql_query_type.as_deref(),
            Some("rustok_blog::graphql::BlogQuery")
        );
        assert_eq!(
            blog.graphql_mutation_type.as_deref(),
            Some("rustok_blog::graphql::BlogMutation")
        );
        assert_eq!(
            blog.http_routes_fn.as_deref(),
            Some("rustok_blog::controllers::routes")
        );
        assert_eq!(
            blog.http_webhook_routes_fn.as_deref(),
            Some("rustok_blog::controllers::webhook_routes")
        );
    }

    #[test]
    #[serial]
    fn catalog_modules_require_rustok_module_manifest_for_path_modules() {
        let temp = tempdir().unwrap();
        let manifest_path = temp.path().join("modules.toml");
        let crate_dir = temp.path().join("crates").join("rustok-blog");
        std::fs::create_dir_all(&crate_dir).unwrap();

        let mut manifest = manifest_with_modules(&[
            "index", "outbox", "blog", "content", "comments", "tenant", "rbac",
        ]);
        manifest.modules.get_mut("blog").unwrap().path = Some("crates/rustok-blog".to_string());
        ManifestManager::save_to_path(&manifest_path, &manifest).unwrap();

        let previous = std::env::var("RUSTOK_MODULES_MANIFEST").ok();
        unsafe {
            std::env::set_var("RUSTOK_MODULES_MANIFEST", &manifest_path);
        }

        let result = ManifestManager::catalog_modules(&manifest);

        match previous {
            Some(value) => unsafe {
                std::env::set_var("RUSTOK_MODULES_MANIFEST", value);
            },
            None => unsafe {
                std::env::remove_var("RUSTOK_MODULES_MANIFEST");
            },
        }

        assert!(matches!(
            result,
            Err(ManifestError::MissingModulePackageManifest { slug, .. }) if slug == "blog"
        ));
    }

    #[test]
    #[serial]
    fn catalog_modules_reject_conflicting_admin_surface_metadata() {
        let temp = tempdir().unwrap();
        let manifest_path = temp.path().join("modules.toml");
        let crate_dir = temp.path().join("crates").join("rustok-blog");
        std::fs::create_dir_all(&crate_dir).unwrap();
        std::fs::write(
            crate_dir.join("rustok-module.toml"),
            r#"[module]
ownership = "first_party"
trust_level = "verified"
recommended_admin_surfaces = ["leptos-admin", "next-admin"]
showcase_admin_surfaces = ["next-admin"]
"#,
        )
        .unwrap();

        let mut manifest = manifest_with_modules(&[
            "index", "outbox", "blog", "content", "comments", "tenant", "rbac",
        ]);
        manifest.modules.get_mut("blog").unwrap().path = Some("crates/rustok-blog".to_string());
        ManifestManager::save_to_path(&manifest_path, &manifest).unwrap();

        let previous = std::env::var("RUSTOK_MODULES_MANIFEST").ok();
        unsafe {
            std::env::set_var("RUSTOK_MODULES_MANIFEST", &manifest_path);
        }

        let result = ManifestManager::catalog_modules(&manifest);

        match previous {
            Some(value) => unsafe {
                std::env::set_var("RUSTOK_MODULES_MANIFEST", value);
            },
            None => unsafe {
                std::env::remove_var("RUSTOK_MODULES_MANIFEST");
            },
        }

        assert!(matches!(
            result,
            Err(ManifestError::ConflictingModuleAdminSurface { slug, surface })
                if slug == "blog" && surface == "next-admin"
        ));
    }

    #[test]
    #[serial]
    fn validate_module_settings_applies_defaults_from_module_manifest() {
        let temp = tempdir().unwrap();
        let blog_dir = temp.path().join("crates").join("rustok-blog");
        let manifest_path = temp.path().join("modules.toml");
        write_module_manifest(
            &blog_dir,
            r#"[module]
slug = "blog"
name = "Blog"
version = "0.1.0"
ownership = "first_party"
trust_level = "verified"

[settings]
postsPerPage = { type = "integer", default = 20, min = 1, max = 100 }
showAuthor = { type = "boolean", default = true }
"#,
        );

        let mut manifest = manifest_with_modules(&[
            "index", "outbox", "blog", "content", "comments", "tenant", "rbac",
        ]);
        manifest.modules.get_mut("blog").unwrap().path = Some("crates/rustok-blog".to_string());
        ManifestManager::save_to_path(&manifest_path, &manifest).unwrap();

        let previous = std::env::var("RUSTOK_MODULES_MANIFEST").ok();
        unsafe {
            std::env::set_var("RUSTOK_MODULES_MANIFEST", &manifest_path);
        }

        let result = ManifestManager::validate_module_settings("blog", serde_json::json!({}));

        match previous {
            Some(value) => unsafe {
                std::env::set_var("RUSTOK_MODULES_MANIFEST", value);
            },
            None => unsafe {
                std::env::remove_var("RUSTOK_MODULES_MANIFEST");
            },
        }

        let settings = result.expect("settings should be normalized from defaults");
        assert_eq!(settings["postsPerPage"], serde_json::json!(20));
        assert_eq!(settings["showAuthor"], serde_json::json!(true));
    }

    #[test]
    #[serial]
    fn validate_module_settings_rejects_unknown_keys() {
        let temp = tempdir().unwrap();
        let blog_dir = temp.path().join("crates").join("rustok-blog");
        let manifest_path = temp.path().join("modules.toml");
        write_module_manifest(
            &blog_dir,
            r#"[module]
slug = "blog"
name = "Blog"
version = "0.1.0"
ownership = "first_party"
trust_level = "verified"

[settings]
postsPerPage = { type = "integer", default = 20, min = 1, max = 100 }
"#,
        );

        let mut manifest = manifest_with_modules(&[
            "index", "outbox", "blog", "content", "comments", "tenant", "rbac",
        ]);
        manifest.modules.get_mut("blog").unwrap().path = Some("crates/rustok-blog".to_string());
        ManifestManager::save_to_path(&manifest_path, &manifest).unwrap();

        let previous = std::env::var("RUSTOK_MODULES_MANIFEST").ok();
        unsafe {
            std::env::set_var("RUSTOK_MODULES_MANIFEST", &manifest_path);
        }

        let result = ManifestManager::validate_module_settings(
            "blog",
            serde_json::json!({ "unknown": true }),
        );

        match previous {
            Some(value) => unsafe {
                std::env::set_var("RUSTOK_MODULES_MANIFEST", value);
            },
            None => unsafe {
                std::env::remove_var("RUSTOK_MODULES_MANIFEST");
            },
        }

        assert!(matches!(
            result,
            Err(ManifestError::InvalidModuleSettingValue { slug, key, .. })
                if slug == "blog" && key == "unknown"
        ));
    }

    #[test]
    #[serial]
    fn validate_module_settings_rejects_out_of_range_values() {
        let temp = tempdir().unwrap();
        let blog_dir = temp.path().join("crates").join("rustok-blog");
        let manifest_path = temp.path().join("modules.toml");
        write_module_manifest(
            &blog_dir,
            r#"[module]
slug = "blog"
name = "Blog"
version = "0.1.0"
ownership = "first_party"
trust_level = "verified"

[settings]
postsPerPage = { type = "integer", default = 20, min = 1, max = 100 }
"#,
        );

        let mut manifest = manifest_with_modules(&[
            "index", "outbox", "blog", "content", "comments", "tenant", "rbac",
        ]);
        manifest.modules.get_mut("blog").unwrap().path = Some("crates/rustok-blog".to_string());
        ManifestManager::save_to_path(&manifest_path, &manifest).unwrap();

        let previous = std::env::var("RUSTOK_MODULES_MANIFEST").ok();
        unsafe {
            std::env::set_var("RUSTOK_MODULES_MANIFEST", &manifest_path);
        }

        let result = ManifestManager::validate_module_settings(
            "blog",
            serde_json::json!({ "postsPerPage": 1000 }),
        );

        match previous {
            Some(value) => unsafe {
                std::env::set_var("RUSTOK_MODULES_MANIFEST", value);
            },
            None => unsafe {
                std::env::remove_var("RUSTOK_MODULES_MANIFEST");
            },
        }

        assert!(matches!(
            result,
            Err(ManifestError::InvalidModuleSettingValue { slug, key, .. })
                if slug == "blog" && key == "postsPerPage"
        ));
    }

    #[test]
    #[serial]
    fn validate_module_settings_rejects_values_outside_declared_options() {
        let temp = tempdir().unwrap();
        let blog_dir = temp.path().join("crates").join("rustok-blog");
        let manifest_path = temp.path().join("modules.toml");
        write_module_manifest(
            &blog_dir,
            r#"[module]
slug = "blog"
name = "Blog"
version = "0.1.0"
ownership = "first_party"
trust_level = "verified"

[settings]
layout = { type = "string", default = "grid", options = ["grid", "list"] }
"#,
        );

        let mut manifest = manifest_with_modules(&[
            "index", "outbox", "blog", "content", "comments", "tenant", "rbac",
        ]);
        manifest.modules.get_mut("blog").unwrap().path = Some("crates/rustok-blog".to_string());
        ManifestManager::save_to_path(&manifest_path, &manifest).unwrap();

        let previous = std::env::var("RUSTOK_MODULES_MANIFEST").ok();
        unsafe {
            std::env::set_var("RUSTOK_MODULES_MANIFEST", &manifest_path);
        }

        let result = ManifestManager::validate_module_settings(
            "blog",
            serde_json::json!({ "layout": "hero" }),
        );

        match previous {
            Some(value) => unsafe {
                std::env::set_var("RUSTOK_MODULES_MANIFEST", value);
            },
            None => unsafe {
                std::env::remove_var("RUSTOK_MODULES_MANIFEST");
            },
        }

        assert!(matches!(
            result,
            Err(ManifestError::InvalidModuleSettingValue { slug, key, reason })
                if slug == "blog"
                    && key == "layout"
                    && reason.contains("must be one of")
        ));
    }

    #[test]
    #[serial]
    fn validate_rejects_setting_schema_with_default_outside_declared_options() {
        let temp = tempdir().unwrap();
        let blog_dir = temp.path().join("crates").join("rustok-blog");
        let manifest_path = temp.path().join("modules.toml");
        write_module_manifest(
            &blog_dir,
            r#"[module]
slug = "blog"
name = "Blog"
version = "0.1.0"
ownership = "first_party"
trust_level = "verified"

[settings]
layout = { type = "string", default = "hero", options = ["grid", "list"] }
"#,
        );

        let mut manifest = manifest_with_modules(&[
            "index", "outbox", "blog", "content", "comments", "tenant", "rbac",
        ]);
        manifest.modules.get_mut("blog").unwrap().path = Some("crates/rustok-blog".to_string());
        ManifestManager::save_to_path(&manifest_path, &manifest).unwrap();

        let previous = std::env::var("RUSTOK_MODULES_MANIFEST").ok();
        unsafe {
            std::env::set_var("RUSTOK_MODULES_MANIFEST", &manifest_path);
        }

        let result = ManifestManager::validate_module_settings("blog", serde_json::json!({}));

        match previous {
            Some(value) => unsafe {
                std::env::set_var("RUSTOK_MODULES_MANIFEST", value);
            },
            None => unsafe {
                std::env::remove_var("RUSTOK_MODULES_MANIFEST");
            },
        }

        assert!(matches!(
            result,
            Err(ManifestError::InvalidModuleSettingSchema { slug, key, reason })
                if slug == "blog"
                    && key == "layout"
                    && reason.contains("default must be one of the declared options")
        ));
    }

    #[test]
    #[serial]
    fn validate_module_settings_rejects_unknown_object_keys_outside_declared_shape() {
        let temp = tempdir().unwrap();
        let blog_dir = temp.path().join("crates").join("rustok-blog");
        let manifest_path = temp.path().join("modules.toml");
        write_module_manifest(
            &blog_dir,
            r#"[module]
slug = "blog"
name = "Blog"
version = "0.1.0"
ownership = "first_party"
trust_level = "verified"

[settings]
seo = { type = "object", object_keys = ["metaTitle", "metaDescription", "indexable"] }
"#,
        );

        let mut manifest = manifest_with_modules(&[
            "index", "outbox", "blog", "content", "comments", "tenant", "rbac",
        ]);
        manifest.modules.get_mut("blog").unwrap().path = Some("crates/rustok-blog".to_string());
        ManifestManager::save_to_path(&manifest_path, &manifest).unwrap();

        let previous = std::env::var("RUSTOK_MODULES_MANIFEST").ok();
        unsafe {
            std::env::set_var("RUSTOK_MODULES_MANIFEST", &manifest_path);
        }

        let result = ManifestManager::validate_module_settings(
            "blog",
            serde_json::json!({
                "seo": {
                    "metaTitle": "Welcome",
                    "unknown": true
                }
            }),
        );

        match previous {
            Some(value) => unsafe {
                std::env::set_var("RUSTOK_MODULES_MANIFEST", value);
            },
            None => unsafe {
                std::env::remove_var("RUSTOK_MODULES_MANIFEST");
            },
        }

        assert!(matches!(
            result,
            Err(ManifestError::InvalidModuleSettingValue { slug, key, reason })
                if slug == "blog"
                    && key == "seo"
                    && reason.contains("unknown object key 'unknown'")
        ));
    }

    #[test]
    #[serial]
    fn validate_module_settings_rejects_array_items_that_do_not_match_declared_item_type() {
        let temp = tempdir().unwrap();
        let blog_dir = temp.path().join("crates").join("rustok-blog");
        let manifest_path = temp.path().join("modules.toml");
        write_module_manifest(
            &blog_dir,
            r#"[module]
slug = "blog"
name = "Blog"
version = "0.1.0"
ownership = "first_party"
trust_level = "verified"

[settings]
featuredPostIds = { type = "array", item_type = "string", default = [] }
"#,
        );

        let mut manifest = manifest_with_modules(&[
            "index", "outbox", "blog", "content", "comments", "tenant", "rbac",
        ]);
        manifest.modules.get_mut("blog").unwrap().path = Some("crates/rustok-blog".to_string());
        ManifestManager::save_to_path(&manifest_path, &manifest).unwrap();

        let previous = std::env::var("RUSTOK_MODULES_MANIFEST").ok();
        unsafe {
            std::env::set_var("RUSTOK_MODULES_MANIFEST", &manifest_path);
        }

        let result = ManifestManager::validate_module_settings(
            "blog",
            serde_json::json!({ "featuredPostIds": ["post-1", 2] }),
        );

        match previous {
            Some(value) => unsafe {
                std::env::set_var("RUSTOK_MODULES_MANIFEST", value);
            },
            None => unsafe {
                std::env::remove_var("RUSTOK_MODULES_MANIFEST");
            },
        }

        assert!(matches!(
            result,
            Err(ManifestError::InvalidModuleSettingValue { slug, key, reason })
                if slug == "blog"
                    && key == "featuredPostIds"
                    && reason.contains("array item at index 1 must be string")
        ));
    }

    #[test]
    #[serial]
    fn validate_rejects_setting_schema_with_item_type_on_non_array_setting() {
        let temp = tempdir().unwrap();
        let blog_dir = temp.path().join("crates").join("rustok-blog");
        let manifest_path = temp.path().join("modules.toml");
        write_module_manifest(
            &blog_dir,
            r#"[module]
slug = "blog"
name = "Blog"
version = "0.1.0"
ownership = "first_party"
trust_level = "verified"

[settings]
seo = { type = "object", item_type = "string" }
"#,
        );

        let mut manifest = manifest_with_modules(&[
            "index", "outbox", "blog", "content", "comments", "tenant", "rbac",
        ]);
        manifest.modules.get_mut("blog").unwrap().path = Some("crates/rustok-blog".to_string());
        ManifestManager::save_to_path(&manifest_path, &manifest).unwrap();

        let previous = std::env::var("RUSTOK_MODULES_MANIFEST").ok();
        unsafe {
            std::env::set_var("RUSTOK_MODULES_MANIFEST", &manifest_path);
        }

        let result = ManifestManager::validate_module_settings("blog", serde_json::json!({}));

        match previous {
            Some(value) => unsafe {
                std::env::set_var("RUSTOK_MODULES_MANIFEST", value);
            },
            None => unsafe {
                std::env::remove_var("RUSTOK_MODULES_MANIFEST");
            },
        }

        assert!(matches!(
            result,
            Err(ManifestError::InvalidModuleSettingSchema { slug, key, reason })
                if slug == "blog"
                    && key == "seo"
                    && reason.contains("item_type is only supported for array settings")
        ));
    }

    #[test]
    #[serial]
    fn validate_module_settings_rejects_nested_object_property_type_mismatch() {
        let temp = tempdir().unwrap();
        let blog_dir = temp.path().join("crates").join("rustok-blog");
        let manifest_path = temp.path().join("modules.toml");
        write_module_manifest(
            &blog_dir,
            r#"[module]
slug = "blog"
name = "Blog"
version = "0.1.0"
ownership = "first_party"
trust_level = "verified"

[settings]
seo = { type = "object", properties = { metaTitle = { type = "string" }, indexable = { type = "boolean", default = true } } }
"#,
        );

        let mut manifest = manifest_with_modules(&[
            "index", "outbox", "blog", "content", "comments", "tenant", "rbac",
        ]);
        manifest.modules.get_mut("blog").unwrap().path = Some("crates/rustok-blog".to_string());
        ManifestManager::save_to_path(&manifest_path, &manifest).unwrap();

        let previous = std::env::var("RUSTOK_MODULES_MANIFEST").ok();
        unsafe {
            std::env::set_var("RUSTOK_MODULES_MANIFEST", &manifest_path);
        }

        let result = ManifestManager::validate_module_settings(
            "blog",
            serde_json::json!({
                "seo": {
                    "metaTitle": 42
                }
            }),
        );

        match previous {
            Some(value) => unsafe {
                std::env::set_var("RUSTOK_MODULES_MANIFEST", value);
            },
            None => unsafe {
                std::env::remove_var("RUSTOK_MODULES_MANIFEST");
            },
        }

        assert!(matches!(
            result,
            Err(ManifestError::InvalidModuleSettingValue { slug, key, reason })
                if slug == "blog"
                    && key == "seo.metaTitle"
                    && reason.contains("expected string")
        ));
    }

    #[test]
    #[serial]
    fn validate_module_settings_rejects_nested_array_item_schema_mismatch() {
        let temp = tempdir().unwrap();
        let blog_dir = temp.path().join("crates").join("rustok-blog");
        let manifest_path = temp.path().join("modules.toml");
        write_module_manifest(
            &blog_dir,
            r#"[module]
slug = "blog"
name = "Blog"
version = "0.1.0"
ownership = "first_party"
trust_level = "verified"

[settings]
contentBlocks = { type = "array", items = { type = "object", properties = { kind = { type = "string" }, enabled = { type = "boolean" } } } }
"#,
        );

        let mut manifest = manifest_with_modules(&[
            "index", "outbox", "blog", "content", "comments", "tenant", "rbac",
        ]);
        manifest.modules.get_mut("blog").unwrap().path = Some("crates/rustok-blog".to_string());
        ManifestManager::save_to_path(&manifest_path, &manifest).unwrap();

        let previous = std::env::var("RUSTOK_MODULES_MANIFEST").ok();
        unsafe {
            std::env::set_var("RUSTOK_MODULES_MANIFEST", &manifest_path);
        }

        let result = ManifestManager::validate_module_settings(
            "blog",
            serde_json::json!({
                "contentBlocks": [
                    { "kind": "hero", "enabled": true },
                    { "kind": "gallery", "enabled": "yes" }
                ]
            }),
        );

        match previous {
            Some(value) => unsafe {
                std::env::set_var("RUSTOK_MODULES_MANIFEST", value);
            },
            None => unsafe {
                std::env::remove_var("RUSTOK_MODULES_MANIFEST");
            },
        }

        assert!(matches!(
            result,
            Err(ManifestError::InvalidModuleSettingValue { slug, key, reason })
                if slug == "blog"
                    && key == "contentBlocks[1].enabled"
                    && reason.contains("expected boolean")
        ));
    }

    #[test]
    #[serial]
    fn validate_rejects_dependency_version_mismatch_from_module_package_manifest() {
        let temp = tempdir().unwrap();
        let blog_dir = temp.path().join("crates").join("rustok-blog");
        let content_dir = temp.path().join("crates").join("rustok-content");
        write_module_manifest(
            &blog_dir,
            r#"[module]
slug = "blog"
name = "Blog"
version = "0.1.0"
ownership = "first_party"
trust_level = "verified"

[dependencies]
content = { version_req = ">=0.2.0" }
"#,
        );
        write_module_manifest(
            &content_dir,
            r#"[module]
slug = "content"
name = "Content"
version = "0.1.0"
ownership = "first_party"
trust_level = "verified"
"#,
        );

        let mut manifest = manifest_with_modules(&[
            "index", "outbox", "blog", "content", "comments", "tenant", "rbac",
        ]);
        manifest.modules.get_mut("blog").unwrap().path = Some("crates/rustok-blog".to_string());
        manifest.modules.get_mut("content").unwrap().path =
            Some("crates/rustok-content".to_string());

        let previous = std::env::var("RUSTOK_MODULES_MANIFEST").ok();
        unsafe {
            std::env::set_var("RUSTOK_MODULES_MANIFEST", temp.path().join("modules.toml"));
        }

        let result = ManifestManager::validate(&manifest);

        match previous {
            Some(value) => unsafe {
                std::env::set_var("RUSTOK_MODULES_MANIFEST", value);
            },
            None => unsafe {
                std::env::remove_var("RUSTOK_MODULES_MANIFEST");
            },
        }

        assert!(matches!(
            result,
            Err(ManifestError::IncompatibleDependencyVersion {
                slug,
                dependency,
                ..
            }) if slug == "blog" && dependency == "content"
        ));
    }

    #[test]
    #[serial]
    fn validate_rejects_conflicting_module_from_module_package_manifest() {
        let temp = tempdir().unwrap();
        let blog_dir = temp.path().join("crates").join("rustok-blog");
        let forum_dir = temp.path().join("crates").join("rustok-forum");
        write_module_manifest(
            &blog_dir,
            r#"[module]
slug = "blog"
name = "Blog"
version = "0.1.0"
ownership = "first_party"
trust_level = "verified"

[conflicts]
modules = ["forum"]
"#,
        );
        write_module_manifest(
            &forum_dir,
            r#"[module]
slug = "forum"
name = "Forum"
version = "0.1.0"
ownership = "first_party"
trust_level = "verified"
"#,
        );

        let mut manifest = manifest_with_modules(&[
            "index", "outbox", "content", "comments", "blog", "forum", "tenant", "rbac",
        ]);
        manifest.modules.get_mut("blog").unwrap().path = Some("crates/rustok-blog".to_string());
        manifest.modules.get_mut("forum").unwrap().path = Some("crates/rustok-forum".to_string());

        let previous = std::env::var("RUSTOK_MODULES_MANIFEST").ok();
        unsafe {
            std::env::set_var("RUSTOK_MODULES_MANIFEST", temp.path().join("modules.toml"));
        }

        let result = ManifestManager::validate(&manifest);

        match previous {
            Some(value) => unsafe {
                std::env::set_var("RUSTOK_MODULES_MANIFEST", value);
            },
            None => unsafe {
                std::env::remove_var("RUSTOK_MODULES_MANIFEST");
            },
        }

        assert!(matches!(
            result,
            Err(ManifestError::ConflictingModule {
                slug,
                conflicts_with,
            }) if slug == "blog" && conflicts_with == "forum"
        ));
    }

    #[test]
    #[serial]
    fn validate_uses_module_package_version_for_dependency_checks() {
        let temp = tempdir().unwrap();
        let blog_dir = temp.path().join("crates").join("rustok-blog");
        let content_dir = temp.path().join("crates").join("rustok-content");
        write_module_manifest(
            &blog_dir,
            r#"[module]
slug = "blog"
name = "Blog"
version = "0.1.0"
ownership = "first_party"
trust_level = "verified"

[dependencies]
content = { version_req = ">=0.1.0" }
"#,
        );
        write_module_manifest(
            &content_dir,
            r#"[module]
slug = "content"
name = "Content"
version = "0.1.0"
ownership = "first_party"
trust_level = "verified"
"#,
        );

        let mut manifest = manifest_with_modules(&[
            "index", "outbox", "blog", "content", "comments", "tenant", "rbac",
        ]);
        manifest.modules.get_mut("blog").unwrap().path = Some("crates/rustok-blog".to_string());
        manifest.modules.get_mut("content").unwrap().path =
            Some("crates/rustok-content".to_string());

        let previous = std::env::var("RUSTOK_MODULES_MANIFEST").ok();
        unsafe {
            std::env::set_var("RUSTOK_MODULES_MANIFEST", temp.path().join("modules.toml"));
        }

        let result = ManifestManager::validate(&manifest);

        match previous {
            Some(value) => unsafe {
                std::env::set_var("RUSTOK_MODULES_MANIFEST", value);
            },
            None => unsafe {
                std::env::remove_var("RUSTOK_MODULES_MANIFEST");
            },
        }

        assert!(
            result.is_ok(),
            "expected dependency version to be resolved from rustok-module.toml"
        );
    }

    #[test]
    #[serial]
    fn validate_rejects_admin_subcrate_without_manifest_wiring() {
        let temp = tempdir().unwrap();
        let blog_dir = temp.path().join("crates").join("rustok-blog");
        write_module_manifest(
            &blog_dir,
            r#"[module]
slug = "blog"
name = "Blog"
version = "0.1.0"
ownership = "first_party"
trust_level = "verified"
"#,
        );
        write_surface_manifest(&blog_dir, "admin", "rustok-blog-admin");

        let mut manifest = manifest_with_modules(&[
            "index", "outbox", "blog", "content", "comments", "tenant", "rbac",
        ]);
        manifest.modules.get_mut("blog").unwrap().path = Some("crates/rustok-blog".to_string());

        let previous = std::env::var("RUSTOK_MODULES_MANIFEST").ok();
        unsafe {
            std::env::set_var("RUSTOK_MODULES_MANIFEST", temp.path().join("modules.toml"));
        }

        let result = ManifestManager::validate(&manifest);

        match previous {
            Some(value) => unsafe {
                std::env::set_var("RUSTOK_MODULES_MANIFEST", value);
            },
            None => unsafe {
                std::env::remove_var("RUSTOK_MODULES_MANIFEST");
            },
        }

        assert!(matches!(
            result,
            Err(ManifestError::InvalidModuleUiWiring { slug, surface, reason })
                if slug == "blog"
                    && surface == "admin"
                    && reason.contains("[provides.admin_ui].leptos_crate")
        ));
    }

    #[test]
    #[serial]
    fn validate_rejects_storefront_wiring_without_subcrate() {
        let temp = tempdir().unwrap();
        let pages_dir = temp.path().join("crates").join("rustok-pages");
        write_module_manifest(
            &pages_dir,
            r#"[module]
slug = "pages"
name = "Pages"
version = "0.1.0"
ownership = "first_party"
trust_level = "verified"

[provides.storefront_ui]
leptos_crate = "rustok-pages-storefront"
"#,
        );

        let mut manifest =
            manifest_with_modules(&["index", "outbox", "pages", "content", "tenant", "rbac"]);
        manifest.modules.get_mut("pages").unwrap().path = Some("crates/rustok-pages".to_string());

        let previous = std::env::var("RUSTOK_MODULES_MANIFEST").ok();
        unsafe {
            std::env::set_var("RUSTOK_MODULES_MANIFEST", temp.path().join("modules.toml"));
        }

        let result = ManifestManager::validate(&manifest);

        match previous {
            Some(value) => unsafe {
                std::env::set_var("RUSTOK_MODULES_MANIFEST", value);
            },
            None => unsafe {
                std::env::remove_var("RUSTOK_MODULES_MANIFEST");
            },
        }

        assert!(matches!(
            result,
            Err(ManifestError::InvalidModuleUiWiring { slug, surface, reason })
                if slug == "pages"
                    && surface == "storefront"
                    && reason.contains("declares [provides.storefront_ui].leptos_crate")
        ));
    }

    #[test]
    #[serial]
    fn validate_accepts_manifest_declared_admin_i18n_bundles() {
        let temp = tempdir().unwrap();
        let blog_dir = temp.path().join("crates").join("rustok-blog");
        let next_messages_dir = temp
            .path()
            .join("apps")
            .join("next-admin")
            .join("packages")
            .join("blog")
            .join("messages");

        write_module_manifest(
            &blog_dir,
            r#"[module]
slug = "blog"
name = "Blog"
version = "0.1.0"
ownership = "first_party"
trust_level = "verified"

[provides.admin_ui]
leptos_crate = "rustok-blog-admin"
next_package = "@rustok/blog-admin"

[provides.admin_ui.i18n]
default_locale = "en"
supported_locales = ["en", "ru"]
leptos_locales_path = "admin/locales"
next_messages_path = "../../apps/next-admin/packages/blog/messages"
"#,
        );
        write_surface_manifest(&blog_dir, "admin", "rustok-blog-admin");
        write_locale_bundle(&blog_dir.join("admin").join("locales"), "en", "Blog");
        write_locale_bundle(&blog_dir.join("admin").join("locales"), "ru", "Блог");
        write_locale_bundle(&next_messages_dir, "en", "Blog");
        write_locale_bundle(&next_messages_dir, "ru", "Блог");

        let mut manifest = manifest_with_modules(&[
            "index", "outbox", "blog", "content", "comments", "tenant", "rbac",
        ]);
        manifest.modules.get_mut("blog").unwrap().path = Some("crates/rustok-blog".to_string());

        let previous = std::env::var("RUSTOK_MODULES_MANIFEST").ok();
        unsafe {
            std::env::set_var("RUSTOK_MODULES_MANIFEST", temp.path().join("modules.toml"));
        }

        let result = ManifestManager::validate(&manifest);

        match previous {
            Some(value) => unsafe {
                std::env::set_var("RUSTOK_MODULES_MANIFEST", value);
            },
            None => unsafe {
                std::env::remove_var("RUSTOK_MODULES_MANIFEST");
            },
        }

        assert!(result.is_ok(), "expected i18n UI wiring to validate");
    }

    #[test]
    #[serial]
    fn validate_rejects_ui_i18n_default_locale_outside_supported_locales() {
        let temp = tempdir().unwrap();
        let blog_dir = temp.path().join("crates").join("rustok-blog");

        write_module_manifest(
            &blog_dir,
            r#"[module]
slug = "blog"
name = "Blog"
version = "0.1.0"
ownership = "first_party"
trust_level = "verified"

[provides.admin_ui]
leptos_crate = "rustok-blog-admin"

[provides.admin_ui.i18n]
default_locale = "ru"
supported_locales = ["en"]
leptos_locales_path = "admin/locales"
"#,
        );
        write_surface_manifest(&blog_dir, "admin", "rustok-blog-admin");
        write_locale_bundle(&blog_dir.join("admin").join("locales"), "en", "Blog");

        let mut manifest = manifest_with_modules(&[
            "index", "outbox", "blog", "content", "comments", "tenant", "rbac",
        ]);
        manifest.modules.get_mut("blog").unwrap().path = Some("crates/rustok-blog".to_string());

        let previous = std::env::var("RUSTOK_MODULES_MANIFEST").ok();
        unsafe {
            std::env::set_var("RUSTOK_MODULES_MANIFEST", temp.path().join("modules.toml"));
        }

        let result = ManifestManager::validate(&manifest);

        match previous {
            Some(value) => unsafe {
                std::env::set_var("RUSTOK_MODULES_MANIFEST", value);
            },
            None => unsafe {
                std::env::remove_var("RUSTOK_MODULES_MANIFEST");
            },
        }

        assert!(matches!(
            result,
            Err(ManifestError::InvalidModuleUiWiring { slug, surface, reason })
                if slug == "blog"
                    && surface == "admin"
                    && reason.contains("default_locale 'ru'")
        ));
    }

    #[test]
    #[serial]
    fn validate_accepts_manifest_declared_i18n_with_script_and_numeric_region_locales() {
        let temp = tempdir().unwrap();
        let pages_dir = temp.path().join("crates").join("rustok-pages");

        write_module_manifest(
            &pages_dir,
            r#"[module]
slug = "pages"
name = "Pages"
version = "0.1.0"
ownership = "first_party"
trust_level = "verified"

[provides.storefront_ui]
leptos_crate = "rustok-pages-storefront"

[provides.storefront_ui.i18n]
default_locale = "zh-hant"
supported_locales = ["zh_hant", "es-419"]
leptos_locales_path = "storefront/locales"
"#,
        );
        write_surface_manifest(&pages_dir, "storefront", "rustok-pages-storefront");
        write_locale_bundle(
            &pages_dir.join("storefront").join("locales"),
            "zh-Hant",
            "Pages",
        );
        write_locale_bundle(
            &pages_dir.join("storefront").join("locales"),
            "es-419",
            "Pages",
        );

        let mut manifest =
            manifest_with_modules(&["index", "outbox", "pages", "content", "tenant", "rbac"]);
        manifest.modules.get_mut("pages").unwrap().path = Some("crates/rustok-pages".to_string());

        let previous = std::env::var("RUSTOK_MODULES_MANIFEST").ok();
        unsafe {
            std::env::set_var("RUSTOK_MODULES_MANIFEST", temp.path().join("modules.toml"));
        }

        let result = ManifestManager::validate(&manifest);

        match previous {
            Some(value) => unsafe {
                std::env::set_var("RUSTOK_MODULES_MANIFEST", value);
            },
            None => unsafe {
                std::env::remove_var("RUSTOK_MODULES_MANIFEST");
            },
        }

        assert!(
            result.is_ok(),
            "expected script/numeric locale UI wiring to validate"
        );
    }

    #[test]
    #[serial]
    fn validate_rejects_manifest_declared_i18n_bundle_missing_locale_file() {
        let temp = tempdir().unwrap();
        let forum_dir = temp.path().join("crates").join("rustok-forum");

        write_module_manifest(
            &forum_dir,
            r#"[module]
slug = "forum"
name = "Forum"
version = "0.1.0"
ownership = "first_party"
trust_level = "verified"

[provides.storefront_ui]
leptos_crate = "rustok-forum-storefront"

[provides.storefront_ui.i18n]
default_locale = "en"
supported_locales = ["en", "ru"]
leptos_locales_path = "storefront/locales"
"#,
        );
        write_surface_manifest(&forum_dir, "storefront", "rustok-forum-storefront");
        write_locale_bundle(&forum_dir.join("storefront").join("locales"), "en", "Forum");

        let mut manifest =
            manifest_with_modules(&["index", "outbox", "forum", "content", "tenant", "rbac"]);
        manifest.modules.get_mut("forum").unwrap().path = Some("crates/rustok-forum".to_string());

        let previous = std::env::var("RUSTOK_MODULES_MANIFEST").ok();
        unsafe {
            std::env::set_var("RUSTOK_MODULES_MANIFEST", temp.path().join("modules.toml"));
        }

        let result = ManifestManager::validate(&manifest);

        match previous {
            Some(value) => unsafe {
                std::env::set_var("RUSTOK_MODULES_MANIFEST", value);
            },
            None => unsafe {
                std::env::remove_var("RUSTOK_MODULES_MANIFEST");
            },
        }

        assert!(matches!(
            result,
            Err(ManifestError::InvalidModuleUiWiring { slug, surface, reason })
                if slug == "forum"
                    && surface == "storefront"
                    && reason.contains("missing locale bundle")
                    && reason.contains("ru.json")
        ));
    }
}
