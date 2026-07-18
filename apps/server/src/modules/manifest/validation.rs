use super::types::*;
use semver::Version;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

pub fn default_manifest_path() -> PathBuf {
    if let Ok(path) = std::env::var("RUSTOK_MODULES_MANIFEST") {
        return PathBuf::from(path);
    }

    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../modules.toml")
}

pub fn module_package_manifest_path(spec: &ManifestModuleSpec) -> Option<PathBuf> {
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

pub fn module_root_path(spec: &ManifestModuleSpec) -> Option<PathBuf> {
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

pub fn workspace_root_path() -> PathBuf {
    default_manifest_path()
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .to_path_buf()
}

pub fn resolve_module_contract_path(
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

pub fn validate_ui_i18n_bundle_dir(
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

pub fn validate_module_ui_i18n_contract(
    slug: &str,
    surface: &str,
    module_root: &Path,
    ui: &ModulePackageUiProvides,
) -> Result<(), ManifestError> {
    let Some(i18n) = ui.i18n.as_ref() else {
        return Ok(());
    };
    let i18n = rustok_modules::validate_static_module_ui_i18n_contract(
        &rustok_modules::StaticModuleUiI18nContract {
            default_locale: i18n.default_locale.clone(),
            supported_locales: i18n.supported_locales.clone(),
            leptos_locales_path: i18n.leptos_locales_path.clone(),
            next_messages_path: i18n.next_messages_path.clone(),
            has_leptos_crate: ui
                .leptos_crate
                .as_deref()
                .is_some_and(|value| !value.trim().is_empty()),
            has_next_package: ui
                .next_package
                .as_deref()
                .is_some_and(|value| !value.trim().is_empty()),
        },
    )
    .map_err(|error| map_static_module_ui_i18n_validation_error(slug, surface, error))?;

    if let Some(path) = i18n.leptos_locales_path.as_deref() {
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
            &i18n.supported_locales,
        )?;
    }

    if let Some(path) = i18n.next_messages_path.as_deref() {
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
            &i18n.supported_locales,
        )?;
    }

    Ok(())
}

fn map_static_module_ui_i18n_validation_error(
    slug: &str,
    surface: &str,
    error: rustok_modules::StaticModuleUiI18nValidationError,
) -> ManifestError {
    let reason = match error {
        rustok_modules::StaticModuleUiI18nValidationError::InvalidSupportedLocale { value } => {
            format!("i18n.supported_locales contains invalid locale '{value}'")
        }
        rustok_modules::StaticModuleUiI18nValidationError::MissingSupportedLocales => {
            "i18n.supported_locales must list at least one locale".to_string()
        }
        rustok_modules::StaticModuleUiI18nValidationError::InvalidDefaultLocale { value } => {
            format!("i18n.default_locale '{value}' is invalid")
        }
        rustok_modules::StaticModuleUiI18nValidationError::DefaultLocaleNotSupported { value } => {
            format!("i18n.default_locale '{value}' must be present in i18n.supported_locales")
        }
        rustok_modules::StaticModuleUiI18nValidationError::MissingBundlePath => {
            "i18n contract must declare leptos_locales_path and/or next_messages_path".to_string()
        }
        rustok_modules::StaticModuleUiI18nValidationError::LeptosPathWithoutCrate => {
            "i18n.leptos_locales_path requires [provides.*_ui].leptos_crate".to_string()
        }
        rustok_modules::StaticModuleUiI18nValidationError::NextPathWithoutPackage => {
            "i18n.next_messages_path requires [provides.*_ui].next_package".to_string()
        }
    };
    ManifestError::InvalidModuleUiWiring {
        slug: slug.to_string(),
        surface: surface.to_string(),
        reason,
    }
}

pub fn merge_module_package_manifest(
    mut spec: ManifestModuleSpec,
    package_manifest: ModulePackageManifest,
) -> Result<ManifestModuleSpec, ManifestError> {
    let crate_name = spec.crate_name.clone();
    let metadata = package_manifest.module;
    let module_slug = metadata.slug.clone().unwrap_or_else(|| crate_name.clone());

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
    let graphql = package_manifest.provides.graphql.as_ref();
    let http = package_manifest.provides.http.as_ref();
    let entrypoints = rustok_modules::resolve_static_module_entrypoints(
        rustok_modules::StaticModuleEntrypointContract {
            crate_name: crate_name.clone(),
            entry_type: package_manifest.crate_contract.entry_type.clone(),
            graphql_query_type: graphql.and_then(|value| value.query.clone()),
            graphql_mutation_type: graphql.and_then(|value| value.mutation.clone()),
            http_routes_fn: http.and_then(|value| value.routes.clone()),
            http_axum_router_fn: http.and_then(|value| value.axum_router.clone()),
            http_webhook_routes_fn: http.and_then(|value| value.webhook_routes.clone()),
            http_axum_webhook_router_fn: http
                .and_then(|value| value.axum_webhook_router.clone()),
        },
    )
    .map_err(|error| {
        let reason = match error {
            rustok_modules::StaticModuleEntrypointValidationError::Http(error) => match error {
                rustok_modules::StaticModuleHttpProvidesValidationError::RoutesAndAxumRouter => {
                    "[provides.http] cannot declare both routes and axum_router"
                }
                rustok_modules::StaticModuleHttpProvidesValidationError::WebhookRoutesAndAxumWebhookRouter => {
                    "[provides.http] cannot declare both webhook_routes and axum_webhook_router"
                }
            },
        };
        ManifestError::InvalidModuleHttpWiring {
            slug: module_slug.clone(),
            reason: reason.to_string(),
        }
    })?;
    if entrypoints.entry_type.is_some() {
        spec.entry_type = entrypoints.entry_type;
    }
    if entrypoints.graphql_query_type.is_some() {
        spec.graphql_query_type = entrypoints.graphql_query_type;
    }
    if entrypoints.graphql_mutation_type.is_some() {
        spec.graphql_mutation_type = entrypoints.graphql_mutation_type;
    }
    if entrypoints.http_routes_fn.is_some() {
        spec.http_routes_fn = entrypoints.http_routes_fn;
    }
    if entrypoints.http_axum_router_fn.is_some() {
        spec.http_axum_router_fn = entrypoints.http_axum_router_fn;
    }
    if entrypoints.http_webhook_routes_fn.is_some() {
        spec.http_webhook_routes_fn = entrypoints.http_webhook_routes_fn;
    }
    if entrypoints.http_axum_webhook_router_fn.is_some() {
        spec.http_axum_webhook_router_fn = entrypoints.http_axum_webhook_router_fn;
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

    Ok(spec)
}

pub fn module_setting_shape_value(spec: &ModuleSettingSpec) -> Option<serde_json::Value> {
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

pub fn to_module_settings_schema(
    schema: &HashMap<String, ModuleSettingSpec>,
) -> HashMap<String, rustok_modules::ModuleSettingSpec> {
    schema
        .iter()
        .map(|(key, spec)| (key.clone(), to_owner_setting_spec(spec)))
        .collect()
}

pub fn map_module_settings_validation_error(
    error: rustok_modules::ModuleSettingsValidationError,
) -> ManifestError {
    match error {
        rustok_modules::ModuleSettingsValidationError::InvalidKey { module_slug, key } => {
            ManifestError::InvalidModuleSettingKey {
                slug: module_slug,
                key,
            }
        }
        rustok_modules::ModuleSettingsValidationError::InvalidSchema {
            module_slug,
            key,
            reason,
        } => ManifestError::InvalidModuleSettingSchema {
            slug: module_slug,
            key,
            reason,
        },
        rustok_modules::ModuleSettingsValidationError::InvalidValue {
            module_slug,
            key,
            reason,
        } => ManifestError::InvalidModuleSettingValue {
            slug: module_slug,
            key,
            reason,
        },
    }
}

fn to_owner_setting_spec(spec: &ModuleSettingSpec) -> rustok_modules::ModuleSettingSpec {
    rustok_modules::ModuleSettingSpec {
        value_type: spec.value_type.clone(),
        required: spec.required,
        default: spec.default.clone(),
        description: spec.description.clone(),
        min: spec.min,
        max: spec.max,
        options: spec.options.clone(),
        object_keys: spec.object_keys.clone(),
        item_type: spec.item_type.clone(),
        properties: to_module_settings_schema(&spec.properties),
        items: spec
            .items
            .as_deref()
            .map(to_owner_setting_spec)
            .map(Box::new),
    }
}

fn to_static_module_package_contract(
    package_manifest: &ModulePackageManifest,
) -> rustok_modules::StaticModulePackageContract {
    let metadata = &package_manifest.module;
    rustok_modules::StaticModulePackageContract {
        declared_slug: metadata.slug.clone(),
        version: metadata.version.clone(),
        ownership: metadata.ownership.clone(),
        trust_level: metadata.trust_level.clone(),
        recommended_admin_surfaces: metadata.recommended_admin_surfaces.clone(),
        showcase_admin_surfaces: metadata.showcase_admin_surfaces.clone(),
        dependencies: package_manifest
            .dependencies
            .iter()
            .map(|(slug, dependency)| (slug.clone(), dependency.version_req.clone()))
            .collect(),
        conflicts: package_manifest.conflicts.modules.clone(),
        settings_schema: to_module_settings_schema(&package_manifest.settings),
    }
}

fn map_static_module_package_validation_error(
    slug: &str,
    error: rustok_modules::StaticModulePackageValidationError,
) -> ManifestError {
    match error {
        rustok_modules::StaticModulePackageValidationError::SlugMismatch { found, .. } => {
            ManifestError::ModulePackageSlugMismatch {
                slug: slug.to_string(),
                found,
            }
        }
        rustok_modules::StaticModulePackageValidationError::InvalidVersion { value } => {
            ManifestError::InvalidModuleVersion {
                slug: slug.to_string(),
                value,
            }
        }
        rustok_modules::StaticModulePackageValidationError::InvalidOwnership { value } => {
            ManifestError::InvalidModuleOwnership {
                slug: slug.to_string(),
                value,
            }
        }
        rustok_modules::StaticModulePackageValidationError::InvalidTrustLevel { value } => {
            ManifestError::InvalidModuleTrustLevel {
                slug: slug.to_string(),
                value,
            }
        }
        rustok_modules::StaticModulePackageValidationError::InvalidAdminSurface {
            field,
            value,
        } => ManifestError::InvalidModuleAdminSurface {
            slug: slug.to_string(),
            field,
            value,
        },
        rustok_modules::StaticModulePackageValidationError::ConflictingAdminSurface { surface } => {
            ManifestError::ConflictingModuleAdminSurface {
                slug: slug.to_string(),
                surface,
            }
        }
        rustok_modules::StaticModulePackageValidationError::InvalidDependency { dependency } => {
            ManifestError::InvalidModuleDependency {
                slug: slug.to_string(),
                dependency,
            }
        }
        rustok_modules::StaticModulePackageValidationError::InvalidDependencyVersionReq {
            dependency,
            value,
        } => ManifestError::InvalidDependencyVersionReq {
            slug: slug.to_string(),
            dependency,
            value,
        },
        rustok_modules::StaticModulePackageValidationError::InvalidConflict { conflict } => {
            ManifestError::InvalidModuleConflict {
                slug: slug.to_string(),
                conflict,
            }
        }
        rustok_modules::StaticModulePackageValidationError::Settings(error) => {
            map_module_settings_validation_error(error)
        }
    }
}

fn to_static_module_catalog_contract(
    spec: &ManifestModuleSpec,
) -> rustok_modules::StaticModuleCatalogContract {
    rustok_modules::StaticModuleCatalogContract {
        ownership: spec.ownership.clone(),
        trust_level: spec.trust_level.clone(),
        recommended_admin_surfaces: spec.recommended_admin_surfaces.clone(),
        showcase_admin_surfaces: spec.showcase_admin_surfaces.clone(),
        description: spec.description.clone(),
        icon_url: spec.icon_url.clone(),
        banner_url: spec.banner_url.clone(),
        screenshots: spec.screenshots.clone(),
    }
}

fn map_static_module_catalog_validation_error(
    slug: &str,
    error: rustok_modules::StaticModuleCatalogValidationError,
) -> ManifestError {
    match error {
        rustok_modules::StaticModuleCatalogValidationError::InvalidOwnership { value } => {
            ManifestError::InvalidModuleOwnership {
                slug: slug.to_string(),
                value,
            }
        }
        rustok_modules::StaticModuleCatalogValidationError::InvalidTrustLevel { value } => {
            ManifestError::InvalidModuleTrustLevel {
                slug: slug.to_string(),
                value,
            }
        }
        rustok_modules::StaticModuleCatalogValidationError::InvalidAdminSurface {
            field,
            value,
        } => ManifestError::InvalidModuleAdminSurface {
            slug: slug.to_string(),
            field,
            value,
        },
        rustok_modules::StaticModuleCatalogValidationError::ConflictingAdminSurface { surface } => {
            ManifestError::ConflictingModuleAdminSurface {
                slug: slug.to_string(),
                surface,
            }
        }
        rustok_modules::StaticModuleCatalogValidationError::InvalidMarketplaceMetadata {
            field,
            reason,
        } => ManifestError::InvalidModuleMarketplaceMetadata {
            slug: slug.to_string(),
            field,
            reason,
        },
    }
}

pub fn to_static_module_topology_contract(
    specs: &HashMap<String, ManifestModuleSpec>,
    default_enabled: &[String],
    platform_version: Option<Version>,
) -> rustok_modules::StaticModuleTopologyContract {
    rustok_modules::StaticModuleTopologyContract {
        modules: specs
            .iter()
            .map(|(slug, spec)| {
                (
                    slug.clone(),
                    rustok_modules::StaticModuleTopologyModule {
                        version: spec.version.clone(),
                        dependencies: spec.depends_on.clone(),
                        dependency_version_requirements: spec.dependency_version_reqs.clone(),
                        conflicts: spec.conflicts_with.clone(),
                        rustok_min_version: spec.rustok_min_version.clone(),
                        rustok_max_version: spec.rustok_max_version.clone(),
                    },
                )
            })
            .collect(),
        default_enabled: default_enabled.to_vec(),
        platform_version,
    }
}

pub fn map_static_module_topology_validation_error(
    error: rustok_modules::StaticModuleTopologyValidationError,
) -> ManifestError {
    match error {
        rustok_modules::StaticModuleTopologyValidationError::UnknownDefaultEnabled { slugs } => {
            ManifestError::UnknownDefaultEnabled(slugs.join(", "))
        }
        rustok_modules::StaticModuleTopologyValidationError::MissingDependencies {
            slug,
            dependencies,
        } => ManifestError::MissingDependencies {
            slug,
            missing: dependencies.join(", "),
        },
        rustok_modules::StaticModuleTopologyValidationError::Conflict { slug, conflict } => {
            ManifestError::ConflictingModule {
                slug,
                conflicts_with: conflict,
            }
        }
        rustok_modules::StaticModuleTopologyValidationError::MissingDependencyVersion {
            slug,
            dependency,
        } => ManifestError::MissingDependencyVersion { slug, dependency },
        rustok_modules::StaticModuleTopologyValidationError::InvalidModuleVersion {
            slug,
            value,
        } => ManifestError::InvalidModuleVersion { slug, value },
        rustok_modules::StaticModuleTopologyValidationError::InvalidDependencyVersionRequirement {
            slug,
            dependency,
            value,
        } => ManifestError::InvalidDependencyVersionReq {
            slug,
            dependency,
            value,
        },
        rustok_modules::StaticModuleTopologyValidationError::IncompatibleDependencyVersion {
            slug,
            dependency,
            required,
            installed,
        } => ManifestError::IncompatibleDependencyVersion {
            slug,
            dependency,
            required,
            installed,
        },
        rustok_modules::StaticModuleTopologyValidationError::IncompatiblePlatformVersion {
            slug,
            current_version,
            minimum,
            maximum,
        } => ManifestError::IncompatibleRustokVersion {
            slug,
            current_version,
            minimum,
            maximum,
        },
    }
}

pub fn validate_module_package_metadata(
    slug: &str,
    package_manifest: &ModulePackageManifest,
) -> Result<(), ManifestError> {
    let contract = to_static_module_package_contract(package_manifest);
    rustok_modules::validate_static_module_package_contract(slug, &contract)
        .map_err(|error| map_static_module_package_validation_error(slug, error))
}
pub fn validate_module_ui_wiring(
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

pub fn module_package_ui_surface_flags(
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

pub fn resolved_catalog_module_ui_classification(
    slug: &str,
    explicit: Option<&str>,
    has_admin_ui: bool,
    has_storefront_ui: bool,
) -> Result<String, ManifestError> {
    rustok_modules::resolve_static_module_ui_classification(
        explicit,
        has_admin_ui,
        has_storefront_ui,
    )
    .map_err(|error| match error {
        rustok_modules::StaticModuleUiClassificationError::Invalid { value } => {
            ManifestError::InvalidModuleUiClassification {
                slug: slug.to_string(),
                value,
            }
        }
    })
}

pub fn apply_module_package_manifest(
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

    merge_module_package_manifest(spec.clone(), package_manifest)
}

pub fn first_party_module(
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

pub fn builtin_module_catalog() -> HashMap<&'static str, ManifestModuleSpec> {
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

pub fn builtin_default_enabled() -> HashSet<&'static str> {
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

pub fn current_platform_version() -> Option<Version> {
    Version::parse(env!("CARGO_PKG_VERSION")).ok()
}

pub fn validate_catalog_metadata(
    slug: &str,
    spec: &ManifestModuleSpec,
) -> Result<(), ManifestError> {
    let contract = to_static_module_catalog_contract(spec);
    rustok_modules::validate_static_module_catalog_contract(&contract)
        .map_err(|error| map_static_module_catalog_validation_error(slug, error))
}
pub fn resolve_module_specs(
    manifest: &ModulesManifest,
) -> Result<HashMap<String, ManifestModuleSpec>, ManifestError> {
    let mut resolved_specs = HashMap::new();
    for (slug, spec) in &manifest.modules {
        let resolved = apply_module_package_manifest(slug, spec)?;
        resolved_specs.insert(slug.clone(), resolved);
    }

    Ok(resolved_specs)
}

pub fn validate_build_surfaces(manifest: &ModulesManifest) -> Result<(), ManifestError> {
    let contract = rustok_modules::PlatformBuildSurfaceContract {
        embed_admin: manifest.build.server.embed_admin,
        embed_storefront: manifest.build.server.embed_storefront,
        admin: rustok_modules::PlatformAdminBuildSurfaceContract {
            stack: manifest.build.admin.stack.clone(),
            public_url: manifest.build.admin.public_url.clone(),
            redirect_uris: manifest.build.admin.redirect_uris.clone(),
        },
        storefronts: manifest
            .build
            .storefront
            .iter()
            .map(
                |storefront| rustok_modules::PlatformStorefrontBuildSurfaceContract {
                    id: storefront.id.clone(),
                    stack: storefront.stack.clone(),
                    public_url: storefront.public_url.clone(),
                    redirect_uris: storefront.redirect_uris.clone(),
                },
            )
            .collect(),
    };
    rustok_modules::validate_platform_build_surface_contract(&contract)
        .map_err(|error| ManifestError::InvalidBuildSurface(error.to_string()))
}
