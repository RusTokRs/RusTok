pub mod plans;
pub mod types;
pub mod validation;

#[cfg(test)]
pub mod tests;

pub use plans::*;
pub use types::*;
pub use validation::*;

use crate::error::{Error as ServerError, Result as ServerResult};
use crate::models::build::DeploymentProfile;
use crate::services::build_service::ModuleSpec as BuildModuleSpec;
use rustok_api::module_registry_contract::{
    validate_module_registry_contract, ManifestModuleContract, ModuleRegistryContractError,
    RegistryModuleContract,
};
use rustok_core::ModuleRegistry;
use semver::{Version, VersionReq};
use std::collections::{BTreeSet, HashMap, HashSet};
use std::path::{Path, PathBuf};

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
        let manifest_contracts = manifest.modules.iter().map(|(slug, manifest_spec)| {
            let resolved_spec = resolved_specs
                .get(slug)
                .expect("resolved manifest module must exist");
            ManifestModuleContract {
                slug: slug.clone(),
                required: manifest_spec.required,
                dependencies: resolved_spec.depends_on.iter().cloned().collect(),
                has_runtime_entry: resolved_spec.entry_type.is_some(),
            }
        });
        let registry_contracts = registry
            .list()
            .into_iter()
            .map(|module| RegistryModuleContract {
                slug: module.slug().to_string(),
                core: registry.is_core(module.slug()),
                dependencies: module
                    .dependencies()
                    .iter()
                    .map(|dependency| dependency.to_string())
                    .collect::<BTreeSet<_>>(),
            });

        validate_module_registry_contract(manifest_contracts, registry_contracts).map_err(|error| {
            match error {
                ModuleRegistryContractError::MissingInRegistry(details) => {
                    ManifestError::MissingInRegistry(details)
                }
                ModuleRegistryContractError::RequiredMismatch(details) => {
                    ManifestError::RequiredMismatch(details)
                }
                ModuleRegistryContractError::DependencyMismatch(details) => {
                    ManifestError::DependencyMismatch(details)
                }
            }
        })
    }
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
