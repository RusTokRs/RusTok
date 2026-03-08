use crate::models::build::DeploymentProfile;
use crate::services::build_service::ModuleSpec as BuildModuleSpec;
use rustok_core::ModuleRegistry;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use thiserror::Error;

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
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct StorefrontBuildConfig {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub stack: String,
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
    pub version: Option<String>,
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
    pub ownership: String,
    #[serde(default)]
    pub trust_level: String,
    #[serde(default)]
    pub rustok_min_version: Option<String>,
    #[serde(default)]
    pub rustok_max_version: Option<String>,
    #[serde(default)]
    pub recommended_admin_surfaces: Vec<String>,
    #[serde(default)]
    pub showcase_admin_surfaces: Vec<String>,
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
    pub version: Option<String>,
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
    pub recommended_admin_surfaces: Vec<String>,
    pub showcase_admin_surfaces: Vec<String>,
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
struct ModulePackageManifest {
    #[serde(default)]
    module: ModulePackageMetadata,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct ModulePackageMetadata {
    #[serde(default)]
    ownership: String,
    #[serde(default)]
    trust_level: String,
    #[serde(default)]
    rustok_min_version: Option<String>,
    #[serde(default)]
    rustok_max_version: Option<String>,
    #[serde(default)]
    recommended_admin_surfaces: Vec<String>,
    #[serde(default)]
    showcase_admin_surfaces: Vec<String>,
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
    #[error("Module '{slug}' has invalid admin surface '{value}' in {field}")]
    InvalidModuleAdminSurface {
        slug: String,
        field: String,
        value: String,
    },
    #[error("Module '{slug}' lists admin surface '{surface}' as both recommended and showcase")]
    ConflictingModuleAdminSurface { slug: String, surface: String },
}

fn is_registry_managed_module(spec: &ManifestModuleSpec) -> bool {
    spec.crate_name != "rustok-outbox"
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

fn merge_module_package_metadata(
    mut spec: ManifestModuleSpec,
    metadata: ModulePackageMetadata,
) -> ManifestModuleSpec {
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
    if !metadata.recommended_admin_surfaces.is_empty() {
        spec.recommended_admin_surfaces = metadata.recommended_admin_surfaces;
    }
    if !metadata.showcase_admin_surfaces.is_empty() {
        spec.showcase_admin_surfaces = metadata.showcase_admin_surfaces;
    }
    spec
}

fn validate_module_package_metadata(
    slug: &str,
    metadata: &ModulePackageMetadata,
) -> Result<(), ManifestError> {
    let ownership = metadata.ownership.trim();
    if !is_valid_module_ownership(ownership) {
        return Err(ManifestError::InvalidModuleOwnership {
            slug: slug.to_string(),
            value: ownership.to_string(),
        });
    }

    let trust_level = metadata.trust_level.trim();
    if !is_valid_trust_level(trust_level) {
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

    Ok(())
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
    validate_module_package_metadata(slug, &package_manifest.module)?;

    Ok(merge_module_package_metadata(
        spec.clone(),
        package_manifest.module,
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
            "commerce",
            first_party_module(
                "rustok-commerce",
                "crates/rustok-commerce",
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
                &["content"],
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
                &[],
                &["leptos-admin"],
                &[],
            ),
        ),
        (
            "alloy",
            first_party_module(
                "alloy-scripting",
                "crates/alloy-scripting",
                false,
                &[],
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
    HashSet::from(["content", "commerce", "pages"])
}

fn is_valid_module_ownership(value: &str) -> bool {
    matches!(value, "first_party" | "third_party")
}

fn is_valid_trust_level(value: &str) -> bool {
    matches!(value, "core" | "verified" | "unverified" | "private")
}

fn is_valid_admin_surface(value: &str) -> bool {
    !value.is_empty()
        && value
            .chars()
            .all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '-')
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
                let manifest_exists = match path.as_ref() {
                    Some(path) => path.exists(),
                    None => false,
                };
                if spec.source == "path" && !manifest_exists {
                    return Err(ManifestError::MissingModulePackageManifest {
                        slug: slug.clone(),
                        path: path
                            .as_ref()
                            .map(|path| path.display().to_string())
                            .unwrap_or_else(|| "<unknown>".to_string()),
                    });
                }

                let spec = apply_module_package_manifest(&slug, &spec)?;
                validate_catalog_metadata(&slug, &spec)?;

                Ok(CatalogManifestModule {
                    slug: slug.to_string(),
                    source: spec.source,
                    crate_name: spec.crate_name,
                    version: spec.version,
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
                    recommended_admin_surfaces: spec.recommended_admin_surfaces,
                    showcase_admin_surfaces: spec.showcase_admin_surfaces,
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

    pub fn validate(manifest: &ModulesManifest) -> Result<(), ManifestError> {
        let installed = manifest.modules.keys().cloned().collect::<HashSet<_>>();

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

        for (slug, spec) in &manifest.modules {
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
        }

        Ok(())
    }

    pub fn validate_with_registry(
        manifest: &ModulesManifest,
        registry: &ModuleRegistry,
    ) -> Result<(), ManifestError> {
        let missing_in_registry: Vec<String> = manifest
            .modules
            .iter()
            .filter(|(_, spec)| is_registry_managed_module(spec))
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
                manifest.modules.get(module.slug()).and_then(|spec| {
                    if !is_registry_managed_module(spec) {
                        None
                    } else {
                        Some((
                            module.slug(),
                            spec.required,
                            registry.is_core(module.slug()),
                        ))
                    }
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
                manifest.modules.get(module.slug()).and_then(|spec| {
                    if !is_registry_managed_module(spec) {
                        None
                    } else {
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

pub fn validate_registry_vs_manifest(registry: &ModuleRegistry) -> loco_rs::Result<()> {
    let manifest = ManifestManager::load().map_err(|error| {
        loco_rs::Error::BadRequest(format!("modules.toml validation failed: {error}"))
    })?;

    ManifestManager::validate(&manifest)
        .and_then(|_| ManifestManager::validate_with_registry(&manifest, registry))
        .map_err(|error| {
            loco_rs::Error::BadRequest(format!("modules.toml validation failed: {error}"))
        })
}

#[cfg(test)]
mod tests {
    use super::{builtin_module_catalog, ManifestError, ManifestManager, ModulesManifest};
    use crate::modules::build_registry;
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

    #[test]
    fn allows_registry_superset_when_optional_module_is_removed_from_manifest() {
        let registry = build_registry();
        let manifest = manifest_with_modules(&[
            "index", "outbox", "content", "commerce", "pages", "tenant", "rbac",
        ]);

        let result = ManifestManager::validate_with_registry(&manifest, &registry);
        assert!(
            result.is_ok(),
            "optional registry modules may be absent from manifest"
        );
    }

    #[test]
    fn uninstall_removes_default_enabled_entry() {
        let mut manifest = manifest_with_modules(&[
            "index", "outbox", "content", "commerce", "pages", "tenant", "rbac",
        ]);
        manifest.settings.default_enabled = vec![
            "content".to_string(),
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
    fn install_builtin_module_restores_catalog_defaults() {
        let mut manifest = manifest_with_modules(&["index", "outbox", "tenant", "rbac"]);

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
"#,
        )
        .unwrap();

        let mut manifest = manifest_with_modules(&["index", "outbox", "blog", "tenant", "rbac"]);
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
        assert_eq!(blog.recommended_admin_surfaces, vec!["custom-admin"]);
        assert_eq!(
            blog.showcase_admin_surfaces,
            vec!["next-admin", "storybook"]
        );
    }

    #[test]
    fn catalog_modules_require_rustok_module_manifest_for_path_modules() {
        let temp = tempdir().unwrap();
        let manifest_path = temp.path().join("modules.toml");
        let crate_dir = temp.path().join("crates").join("rustok-blog");
        std::fs::create_dir_all(&crate_dir).unwrap();

        let mut manifest = manifest_with_modules(&["index", "outbox", "blog", "tenant", "rbac"]);
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

        let mut manifest = manifest_with_modules(&["index", "outbox", "blog", "tenant", "rbac"]);
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
}
