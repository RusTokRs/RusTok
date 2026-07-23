//! Platform-domain validation for static module package metadata.

use semver::{Version, VersionReq};
use std::collections::{BTreeSet, HashMap};
use thiserror::Error;
use url::Url;

use crate::{ModuleSettingSpec, ModuleSettingsValidationError, validate_module_settings_schema};

/// Host-parsed static package metadata supplied to the module control plane.
#[derive(Debug, Clone, Default)]
pub struct StaticModulePackageContract {
    pub declared_slug: Option<String>,
    pub version: Option<String>,
    pub ownership: String,
    pub trust_level: String,
    pub recommended_admin_surfaces: Vec<String>,
    pub showcase_admin_surfaces: Vec<String>,
    pub dependencies: HashMap<String, String>,
    pub conflicts: Vec<String>,
    pub settings_schema: HashMap<String, ModuleSettingSpec>,
}

/// Stable semantic failures for host-parsed static package metadata.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum StaticModulePackageValidationError {
    #[error("module package declares slug '{found}', expected '{expected}'")]
    SlugMismatch { expected: String, found: String },
    #[error("module package has invalid version '{value}'")]
    InvalidVersion { value: String },
    #[error("module package has invalid ownership '{value}'")]
    InvalidOwnership { value: String },
    #[error("module package has invalid trust level '{value}'")]
    InvalidTrustLevel { value: String },
    #[error("module package has invalid admin surface '{value}' in {field}")]
    InvalidAdminSurface { field: String, value: String },
    #[error("module package lists admin surface '{surface}' as both recommended and showcase")]
    ConflictingAdminSurface { surface: String },
    #[error("module package declares invalid dependency '{dependency}'")]
    InvalidDependency { dependency: String },
    #[error("module package dependency '{dependency}' has invalid version requirement '{value}'")]
    InvalidDependencyVersionReq { dependency: String, value: String },
    #[error("module package declares invalid conflict '{conflict}'")]
    InvalidConflict { conflict: String },
    #[error(transparent)]
    Settings(#[from] ModuleSettingsValidationError),
}

/// Host-parsed catalog metadata supplied to the module control plane.
#[derive(Debug, Clone, Default)]
pub struct StaticModuleCatalogContract {
    pub ownership: String,
    pub trust_level: String,
    pub recommended_admin_surfaces: Vec<String>,
    pub showcase_admin_surfaces: Vec<String>,
    pub description: Option<String>,
    pub icon_url: Option<String>,
    pub banner_url: Option<String>,
    pub screenshots: Vec<String>,
}

/// Host-parsed static module topology supplied to the module control plane.
/// Filesystem and build-surface concerns remain host-owned.
#[derive(Debug, Clone, Default)]
pub struct StaticModuleTopologyContract {
    pub modules: HashMap<String, StaticModuleTopologyModule>,
    pub default_enabled: Vec<String>,
    pub platform_version: Option<Version>,
}

/// One static module's topology facts, independent of host manifest parsing.
#[derive(Debug, Clone, Default)]
pub struct StaticModuleTopologyModule {
    pub version: Option<String>,
    pub dependencies: Vec<String>,
    pub dependency_version_requirements: HashMap<String, String>,
    pub conflicts: Vec<String>,
    pub rustok_min_version: Option<String>,
    pub rustok_max_version: Option<String>,
}

/// Stable semantic failures for a host-parsed static module topology.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum StaticModuleTopologyValidationError {
    #[error("default-enabled modules are not installed: {slugs:?}")]
    UnknownDefaultEnabled { slugs: Vec<String> },
    #[error("module '{slug}' depends on missing modules: {dependencies:?}")]
    MissingDependencies {
        slug: String,
        dependencies: Vec<String>,
    },
    #[error("module '{slug}' conflicts with installed module '{conflict}'")]
    Conflict { slug: String, conflict: String },
    #[error("module '{slug}' requires a version for dependency '{dependency}'")]
    MissingDependencyVersion { slug: String, dependency: String },
    #[error("module '{slug}' has invalid version '{value}'")]
    InvalidModuleVersion { slug: String, value: String },
    #[error("module '{slug}' dependency '{dependency}' has invalid version requirement '{value}'")]
    InvalidDependencyVersionRequirement {
        slug: String,
        dependency: String,
        value: String,
    },
    #[error(
        "module '{slug}' requires '{dependency}' version '{required}', but installed '{installed}'"
    )]
    IncompatibleDependencyVersion {
        slug: String,
        dependency: String,
        required: String,
        installed: String,
    },
    #[error("module '{slug}' is incompatible with RusToK {current_version}")]
    IncompatiblePlatformVersion {
        slug: String,
        current_version: String,
        minimum: Option<String>,
        maximum: Option<String>,
    },
}

/// Stable semantic failures for static catalog metadata.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum StaticModuleCatalogValidationError {
    #[error("catalog metadata has invalid ownership '{value}'")]
    InvalidOwnership { value: String },
    #[error("catalog metadata has invalid trust level '{value}'")]
    InvalidTrustLevel { value: String },
    #[error("catalog metadata has invalid admin surface '{value}' in {field}")]
    InvalidAdminSurface { field: String, value: String },
    #[error("catalog metadata lists admin surface '{surface}' as both recommended and showcase")]
    ConflictingAdminSurface { surface: String },
    #[error("catalog marketplace metadata '{field}' is invalid: {reason}")]
    InvalidMarketplaceMetadata { field: String, reason: String },
}

/// Stable semantic failures for a static catalog UI classification.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum StaticModuleUiClassificationError {
    #[error("catalog UI classification '{value}' does not match its declared surfaces")]
    Invalid { value: String },
}

/// Stable semantic failures for static RusToK platform-version constraints.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum StaticModulePlatformVersionError {
    #[error("invalid minimum platform version requirement '{value}'")]
    InvalidMinimum { value: String },
    #[error("invalid maximum platform version requirement '{value}'")]
    InvalidMaximum { value: String },
}

/// Host-parsed static UI i18n metadata supplied to the module control plane.
#[derive(Debug, Clone, Default)]
pub struct StaticModuleUiI18nContract {
    pub default_locale: Option<String>,
    pub supported_locales: Vec<String>,
    pub leptos_locales_path: Option<String>,
    pub next_messages_path: Option<String>,
    pub has_leptos_crate: bool,
    pub has_next_package: bool,
}

/// Normalized static UI i18n metadata for host filesystem validation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StaticModuleUiI18nResolved {
    pub supported_locales: Vec<String>,
    pub leptos_locales_path: Option<String>,
    pub next_messages_path: Option<String>,
}

/// Stable semantic failures for static UI i18n metadata.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum StaticModuleUiI18nValidationError {
    #[error("i18n.supported_locales contains invalid locale '{value}'")]
    InvalidSupportedLocale { value: String },
    #[error("i18n.supported_locales must list at least one locale")]
    MissingSupportedLocales,
    #[error("i18n.default_locale '{value}' is invalid")]
    InvalidDefaultLocale { value: String },
    #[error("i18n.default_locale '{value}' must be present in i18n.supported_locales")]
    DefaultLocaleNotSupported { value: String },
    #[error("i18n contract must declare leptos_locales_path and/or next_messages_path")]
    MissingBundlePath,
    #[error("i18n.leptos_locales_path requires a Leptos crate")]
    LeptosPathWithoutCrate,
    #[error("i18n.next_messages_path requires a Next package")]
    NextPathWithoutPackage,
}

/// Host-parsed static HTTP surface declarations supplied to the control plane.
#[derive(Debug, Clone, Copy, Default)]
pub struct StaticModuleHttpProvidesContract {
    pub has_routes: bool,
    pub has_axum_router: bool,
    pub has_webhook_routes: bool,
    pub has_axum_webhook_router: bool,
}

/// Host-parsed crate-local binding declarations. The owner normalizes these
/// into stable Rust paths after validating the mutually exclusive HTTP shape.
#[derive(Debug, Clone, Default)]
pub struct StaticModuleEntrypointContract {
    pub crate_name: String,
    pub entry_type: Option<String>,
    pub graphql_query_type: Option<String>,
    pub graphql_mutation_type: Option<String>,
    pub http_routes_fn: Option<String>,
    pub http_axum_router_fn: Option<String>,
    pub http_webhook_routes_fn: Option<String>,
    pub http_axum_webhook_router_fn: Option<String>,
}

/// Owner-normalized runtime bindings that a host may attach to its static
/// module specification. They do not contain executable handles.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StaticModuleEntrypoints {
    pub entry_type: Option<String>,
    pub graphql_query_type: Option<String>,
    pub graphql_mutation_type: Option<String>,
    pub http_routes_fn: Option<String>,
    pub http_axum_router_fn: Option<String>,
    pub http_webhook_routes_fn: Option<String>,
    pub http_axum_webhook_router_fn: Option<String>,
}

/// Stable semantic failures for static HTTP surface declarations.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum StaticModuleHttpProvidesValidationError {
    #[error("[provides.http] cannot declare both routes and axum_router")]
    RoutesAndAxumRouter,
    #[error("[provides.http] cannot declare both webhook_routes and axum_webhook_router")]
    WebhookRoutesAndAxumWebhookRouter,
}

/// Stable semantic failures while resolving host-parsed static bindings.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum StaticModuleEntrypointValidationError {
    #[error(transparent)]
    Http(#[from] StaticModuleHttpProvidesValidationError),
}

/// Validates module-domain metadata from a host-parsed `rustok-module.toml`.
pub fn validate_static_module_package_contract(
    module_slug: &str,
    contract: &StaticModulePackageContract,
) -> Result<(), StaticModulePackageValidationError> {
    if let Some(found_slug) = contract
        .declared_slug
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        if !is_valid_static_module_slug(found_slug) || found_slug != module_slug {
            return Err(StaticModulePackageValidationError::SlugMismatch {
                expected: module_slug.to_string(),
                found: found_slug.to_string(),
            });
        }
    }

    if let Some(version) = contract
        .version
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        Version::parse(version).map_err(|_| {
            StaticModulePackageValidationError::InvalidVersion {
                value: version.to_string(),
            }
        })?;
    }

    let ownership = contract.ownership.trim();
    if !ownership.is_empty() && !matches!(ownership, "first_party" | "third_party") {
        return Err(StaticModulePackageValidationError::InvalidOwnership {
            value: ownership.to_string(),
        });
    }

    let trust_level = contract.trust_level.trim();
    if !trust_level.is_empty()
        && !matches!(trust_level, "core" | "verified" | "unverified" | "private")
    {
        return Err(StaticModulePackageValidationError::InvalidTrustLevel {
            value: trust_level.to_string(),
        });
    }

    let recommended = collect_admin_surfaces(
        "recommended_admin_surfaces",
        &contract.recommended_admin_surfaces,
    )
    .map_err(
        |(field, value)| StaticModulePackageValidationError::InvalidAdminSurface { field, value },
    )?;
    let showcase =
        collect_admin_surfaces("showcase_admin_surfaces", &contract.showcase_admin_surfaces)
            .map_err(
                |(field, value)| StaticModulePackageValidationError::InvalidAdminSurface {
                    field,
                    value,
                },
            )?;
    if let Some(surface) = recommended.intersection(&showcase).next() {
        return Err(
            StaticModulePackageValidationError::ConflictingAdminSurface {
                surface: surface.clone(),
            },
        );
    }

    for (dependency, version_req) in &contract.dependencies {
        let dependency = dependency.trim();
        if !is_valid_static_module_slug(dependency) {
            return Err(StaticModulePackageValidationError::InvalidDependency {
                dependency: dependency.to_string(),
            });
        }
        let version_req = version_req.trim();
        if !version_req.is_empty() {
            VersionReq::parse(version_req).map_err(|_| {
                StaticModulePackageValidationError::InvalidDependencyVersionReq {
                    dependency: dependency.to_string(),
                    value: version_req.to_string(),
                }
            })?;
        }
    }

    for conflict in &contract.conflicts {
        let conflict = conflict.trim();
        if !is_valid_static_module_slug(conflict) || conflict == module_slug {
            return Err(StaticModulePackageValidationError::InvalidConflict {
                conflict: conflict.to_string(),
            });
        }
    }

    validate_module_settings_schema(module_slug, &contract.settings_schema)?;
    Ok(())
}

fn collect_admin_surfaces(
    field: &str,
    surfaces: &[String],
) -> Result<BTreeSet<String>, (String, String)> {
    let mut normalized = BTreeSet::new();
    for surface in surfaces {
        let surface = surface.trim();
        if surface.is_empty()
            || !surface.chars().all(|character| {
                character.is_ascii_lowercase() || character.is_ascii_digit() || character == '-'
            })
        {
            return Err((field.to_string(), surface.to_string()));
        }
        normalized.insert(surface.to_string());
    }
    Ok(normalized)
}

/// Returns whether a slug uses the portable static-module identifier grammar.
pub fn is_valid_static_module_slug(value: &str) -> bool {
    !value.is_empty()
        && value.chars().all(|character| {
            character.is_ascii_lowercase()
                || character.is_ascii_digit()
                || character == '-'
                || character == '_'
        })
}

/// Validates module-domain metadata from one host-parsed static catalog entry.
pub fn validate_static_module_catalog_contract(
    contract: &StaticModuleCatalogContract,
) -> Result<(), StaticModuleCatalogValidationError> {
    let ownership = contract.ownership.trim();
    if !matches!(ownership, "first_party" | "third_party") {
        return Err(StaticModuleCatalogValidationError::InvalidOwnership {
            value: ownership.to_string(),
        });
    }
    let trust_level = contract.trust_level.trim();
    if !matches!(trust_level, "core" | "verified" | "unverified" | "private") {
        return Err(StaticModuleCatalogValidationError::InvalidTrustLevel {
            value: trust_level.to_string(),
        });
    }
    let recommended = collect_admin_surfaces(
        "recommended_admin_surfaces",
        &contract.recommended_admin_surfaces,
    )
    .map_err(
        |(field, value)| StaticModuleCatalogValidationError::InvalidAdminSurface { field, value },
    )?;
    let showcase =
        collect_admin_surfaces("showcase_admin_surfaces", &contract.showcase_admin_surfaces)
            .map_err(
                |(field, value)| StaticModuleCatalogValidationError::InvalidAdminSurface {
                    field,
                    value,
                },
            )?;
    if let Some(surface) = recommended.intersection(&showcase).next() {
        return Err(
            StaticModuleCatalogValidationError::ConflictingAdminSurface {
                surface: surface.clone(),
            },
        );
    }
    validate_catalog_marketplace_metadata(contract)
}

/// Validates static module defaults, dependencies, conflicts, and compatibility
/// after the host has resolved manifest and package overlays.
pub fn validate_static_module_topology_contract(
    contract: &StaticModuleTopologyContract,
) -> Result<(), StaticModuleTopologyValidationError> {
    let mut unknown_defaults = contract
        .default_enabled
        .iter()
        .filter(|slug| !contract.modules.contains_key(*slug))
        .cloned()
        .collect::<Vec<_>>();
    unknown_defaults.sort();
    unknown_defaults.dedup();
    if !unknown_defaults.is_empty() {
        return Err(StaticModuleTopologyValidationError::UnknownDefaultEnabled {
            slugs: unknown_defaults,
        });
    }

    let mut slugs = contract.modules.keys().cloned().collect::<Vec<_>>();
    slugs.sort();
    for slug in slugs {
        let module = contract
            .modules
            .get(&slug)
            .expect("static topology key must resolve to its module contract");
        let missing_dependencies = module
            .dependencies
            .iter()
            .filter(|dependency| !contract.modules.contains_key(*dependency))
            .cloned()
            .collect::<Vec<_>>();
        if !missing_dependencies.is_empty() {
            return Err(StaticModuleTopologyValidationError::MissingDependencies {
                slug,
                dependencies: missing_dependencies,
            });
        }

        if let Some(conflict) = module
            .conflicts
            .iter()
            .find(|conflict| contract.modules.contains_key(*conflict))
        {
            return Err(StaticModuleTopologyValidationError::Conflict {
                slug,
                conflict: conflict.clone(),
            });
        }

        let mut version_requirements = module
            .dependency_version_requirements
            .iter()
            .collect::<Vec<_>>();
        version_requirements.sort_by(|(left, _), (right, _)| left.cmp(right));
        for (dependency, raw_requirement) in version_requirements {
            let Some(dependency_module) = contract.modules.get(dependency) else {
                continue;
            };
            let installed = dependency_module.version.as_deref().ok_or_else(|| {
                StaticModuleTopologyValidationError::MissingDependencyVersion {
                    slug: slug.clone(),
                    dependency: dependency.clone(),
                }
            })?;
            let installed = Version::parse(installed).map_err(|_| {
                StaticModuleTopologyValidationError::InvalidModuleVersion {
                    slug: dependency.clone(),
                    value: installed.to_string(),
                }
            })?;
            let requirement = VersionReq::parse(raw_requirement).map_err(|_| {
                StaticModuleTopologyValidationError::InvalidDependencyVersionRequirement {
                    slug: slug.clone(),
                    dependency: dependency.clone(),
                    value: raw_requirement.clone(),
                }
            })?;
            if !requirement.matches(&installed) {
                return Err(
                    StaticModuleTopologyValidationError::IncompatibleDependencyVersion {
                        slug: slug.clone(),
                        dependency: dependency.clone(),
                        required: raw_requirement.clone(),
                        installed: installed.to_string(),
                    },
                );
            }
        }

        if let Some(current_version) = contract.platform_version.as_ref() {
            let compatible = static_module_platform_version_is_compatible(
                current_version,
                module.rustok_min_version.as_deref(),
                module.rustok_max_version.as_deref(),
            )
            .unwrap_or(false);
            if !compatible {
                return Err(
                    StaticModuleTopologyValidationError::IncompatiblePlatformVersion {
                        slug,
                        current_version: current_version.to_string(),
                        minimum: module.rustok_min_version.clone(),
                        maximum: module.rustok_max_version.clone(),
                    },
                );
            }
        }
    }
    Ok(())
}

/// Applies the canonical shared static-manifest versus static-registry contract
/// through the module control-plane boundary. Hosts extract runtime facts from
/// their compile-time registry but do not reimplement the comparison.
pub fn validate_static_module_registry_contracts(
    manifest_modules: impl IntoIterator<
        Item = rustok_api::module_registry_contract::ManifestModuleContract,
    >,
    registry_modules: impl IntoIterator<
        Item = rustok_api::module_registry_contract::RegistryModuleContract,
    >,
) -> Result<(), rustok_api::module_registry_contract::ModuleRegistryContractError> {
    rustok_api::module_registry_contract::validate_module_registry_contract(
        manifest_modules,
        registry_modules,
    )
}

/// Resolves the canonical UI classification for host-parsed static surfaces.
///
/// An absent explicit classification derives from the supplied surface flags.
/// A declared classification must normalize to a supported value and agree with
/// those flags; `capability_only` and `future_ui` are valid only without UI.
pub fn resolve_static_module_ui_classification(
    explicit: Option<&str>,
    has_admin_ui: bool,
    has_storefront_ui: bool,
) -> Result<String, StaticModuleUiClassificationError> {
    let derived = match (has_admin_ui, has_storefront_ui) {
        (true, true) => "dual_surface",
        (true, false) => "admin_only",
        (false, true) => "storefront_only",
        (false, false) => "no_ui",
    };
    let Some(explicit) = explicit.map(str::trim).filter(|value| !value.is_empty()) else {
        return Ok(derived.to_string());
    };

    let normalized = explicit.to_ascii_lowercase().replace('-', "_");
    let matches_surface_contract = match normalized.as_str() {
        "dual_surface" => has_admin_ui && has_storefront_ui,
        "admin_only" => has_admin_ui && !has_storefront_ui,
        "storefront_only" => !has_admin_ui && has_storefront_ui,
        "no_ui" | "capability_only" | "future_ui" => !has_admin_ui && !has_storefront_ui,
        _ => false,
    };
    if !matches_surface_contract {
        return Err(StaticModuleUiClassificationError::Invalid {
            value: explicit.to_string(),
        });
    }

    Ok(normalized)
}

/// Checks whether a static module's optional platform-version constraints admit
/// the host-supplied RusToK version.
pub fn static_module_platform_version_is_compatible(
    current_version: &Version,
    minimum: Option<&str>,
    maximum: Option<&str>,
) -> Result<bool, StaticModulePlatformVersionError> {
    let minimum = minimum
        .map(|value| parse_static_platform_version_requirement(value, false))
        .transpose()?;
    let maximum = maximum
        .map(|value| parse_static_platform_version_requirement(value, true))
        .transpose()?;
    Ok(
        minimum.is_none_or(|requirement| requirement.matches(current_version))
            && maximum.is_none_or(|requirement| requirement.matches(current_version)),
    )
}

/// Validates and normalizes static UI i18n metadata before the host resolves
/// declared locale-bundle paths.
pub fn validate_static_module_ui_i18n_contract(
    contract: &StaticModuleUiI18nContract,
) -> Result<StaticModuleUiI18nResolved, StaticModuleUiI18nValidationError> {
    let supported_locales = contract
        .supported_locales
        .iter()
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
        .map(|locale| {
            rustok_api::normalize_locale_tag(locale).ok_or_else(|| {
                StaticModuleUiI18nValidationError::InvalidSupportedLocale {
                    value: locale.to_string(),
                }
            })
        })
        .collect::<Result<BTreeSet<_>, _>>()?
        .into_iter()
        .collect::<Vec<_>>();
    if supported_locales.is_empty() {
        return Err(StaticModuleUiI18nValidationError::MissingSupportedLocales);
    }

    if let Some(default_locale) = contract
        .default_locale
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        let default_locale = rustok_api::normalize_locale_tag(default_locale).ok_or_else(|| {
            StaticModuleUiI18nValidationError::InvalidDefaultLocale {
                value: default_locale.to_string(),
            }
        })?;
        if !supported_locales.contains(&default_locale) {
            return Err(
                StaticModuleUiI18nValidationError::DefaultLocaleNotSupported {
                    value: default_locale,
                },
            );
        }
    }

    let leptos_locales_path = contract
        .leptos_locales_path
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string);
    let next_messages_path = contract
        .next_messages_path
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string);
    if leptos_locales_path.is_none() && next_messages_path.is_none() {
        return Err(StaticModuleUiI18nValidationError::MissingBundlePath);
    }
    if leptos_locales_path.is_some() && !contract.has_leptos_crate {
        return Err(StaticModuleUiI18nValidationError::LeptosPathWithoutCrate);
    }
    if next_messages_path.is_some() && !contract.has_next_package {
        return Err(StaticModuleUiI18nValidationError::NextPathWithoutPackage);
    }

    Ok(StaticModuleUiI18nResolved {
        supported_locales,
        leptos_locales_path,
        next_messages_path,
    })
}

/// Validates mutually exclusive static HTTP provider declarations before the
/// host qualifies crate-local symbols.
pub fn validate_static_module_http_provides_contract(
    contract: StaticModuleHttpProvidesContract,
) -> Result<(), StaticModuleHttpProvidesValidationError> {
    if contract.has_routes && contract.has_axum_router {
        return Err(StaticModuleHttpProvidesValidationError::RoutesAndAxumRouter);
    }
    if contract.has_webhook_routes && contract.has_axum_webhook_router {
        return Err(StaticModuleHttpProvidesValidationError::WebhookRoutesAndAxumWebhookRouter);
    }
    Ok(())
}

/// Validates the static HTTP surface shape and qualifies crate-local binding
/// declarations without depending on the server's manifest or runtime types.
pub fn resolve_static_module_entrypoints(
    contract: StaticModuleEntrypointContract,
) -> Result<StaticModuleEntrypoints, StaticModuleEntrypointValidationError> {
    validate_static_module_http_provides_contract(StaticModuleHttpProvidesContract {
        has_routes: contract.http_routes_fn.is_some(),
        has_axum_router: contract.http_axum_router_fn.is_some(),
        has_webhook_routes: contract.http_webhook_routes_fn.is_some(),
        has_axum_webhook_router: contract.http_axum_webhook_router_fn.is_some(),
    })?;

    Ok(StaticModuleEntrypoints {
        entry_type: qualify_static_module_symbol(&contract.crate_name, contract.entry_type),
        graphql_query_type: qualify_static_module_symbol(
            &contract.crate_name,
            contract.graphql_query_type,
        ),
        graphql_mutation_type: qualify_static_module_symbol(
            &contract.crate_name,
            contract.graphql_mutation_type,
        ),
        http_routes_fn: qualify_static_module_symbol(&contract.crate_name, contract.http_routes_fn),
        http_axum_router_fn: qualify_static_module_symbol(
            &contract.crate_name,
            contract.http_axum_router_fn,
        ),
        http_webhook_routes_fn: qualify_static_module_symbol(
            &contract.crate_name,
            contract.http_webhook_routes_fn,
        ),
        http_axum_webhook_router_fn: qualify_static_module_symbol(
            &contract.crate_name,
            contract.http_axum_webhook_router_fn,
        ),
    })
}

fn qualify_static_module_symbol(crate_name: &str, value: Option<String>) -> Option<String> {
    let value = value?.trim().to_string();
    if value.is_empty() {
        return None;
    }

    let crate_ident = crate_name.replace('-', "_");
    let relative = value.strip_prefix("crate::").unwrap_or(&value);
    Some(format!("{crate_ident}::{relative}"))
}

fn parse_static_platform_version_requirement(
    value: &str,
    is_maximum: bool,
) -> Result<VersionReq, StaticModulePlatformVersionError> {
    let wildcard = value.trim().replace(".x", ".*").replace(".X", ".*");
    let has_operator = wildcard.contains('<')
        || wildcard.contains('>')
        || wildcard.contains('=')
        || wildcard.contains('~')
        || wildcard.contains('^')
        || wildcard.contains('*')
        || wildcard.contains(',');
    let requirement = if has_operator {
        wildcard
    } else if is_maximum {
        format!("<= {wildcard}")
    } else {
        format!(">= {wildcard}")
    };
    VersionReq::parse(&requirement).map_err(|_| {
        if is_maximum {
            StaticModulePlatformVersionError::InvalidMaximum {
                value: value.to_string(),
            }
        } else {
            StaticModulePlatformVersionError::InvalidMinimum {
                value: value.to_string(),
            }
        }
    })
}

fn validate_catalog_marketplace_metadata(
    contract: &StaticModuleCatalogContract,
) -> Result<(), StaticModuleCatalogValidationError> {
    if let Some(description) = contract
        .description
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        if description.chars().count() < 20 {
            return Err(
                StaticModuleCatalogValidationError::InvalidMarketplaceMetadata {
                    field: "description".to_string(),
                    reason: "must be at least 20 characters".to_string(),
                },
            );
        }
    }
    if let Some(icon_url) = contract
        .icon_url
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        validate_marketplace_asset_url("icon", icon_url, &["svg"])?;
    }
    if let Some(banner_url) = contract
        .banner_url
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        validate_marketplace_asset_url(
            "banner",
            banner_url,
            &["png", "jpg", "jpeg", "webp", "svg"],
        )?;
    }
    for (index, screenshot) in contract.screenshots.iter().enumerate() {
        let screenshot = screenshot.trim();
        if !screenshot.is_empty() {
            validate_marketplace_asset_url(
                &format!("screenshots[{index}]"),
                screenshot,
                &["png", "jpg", "jpeg", "webp", "svg"],
            )?;
        }
    }
    Ok(())
}

fn validate_marketplace_asset_url(
    field: &str,
    value: &str,
    allowed_extensions: &[&str],
) -> Result<(), StaticModuleCatalogValidationError> {
    let parsed = Url::parse(value).map_err(|error| {
        StaticModuleCatalogValidationError::InvalidMarketplaceMetadata {
            field: field.to_string(),
            reason: format!("must be a valid absolute URL: {error}"),
        }
    })?;
    if !matches!(parsed.scheme(), "http" | "https") {
        return Err(
            StaticModuleCatalogValidationError::InvalidMarketplaceMetadata {
                field: field.to_string(),
                reason: "must use http or https".to_string(),
            },
        );
    }
    let has_allowed_extension = allowed_extensions.iter().any(|extension| {
        parsed
            .path()
            .rsplit('/')
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
        return Err(
            StaticModuleCatalogValidationError::InvalidMarketplaceMetadata {
                field: field.to_string(),
                reason: format!("must point to one of: {allowed}"),
            },
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{
        StaticModuleEntrypointContract, StaticModuleEntrypointValidationError,
        StaticModuleHttpProvidesContract, StaticModuleHttpProvidesValidationError,
        StaticModulePlatformVersionError, StaticModuleTopologyContract, StaticModuleTopologyModule,
        StaticModuleTopologyValidationError, StaticModuleUiClassificationError,
        StaticModuleUiI18nContract, StaticModuleUiI18nValidationError,
        resolve_static_module_entrypoints, resolve_static_module_ui_classification,
        static_module_platform_version_is_compatible,
        validate_static_module_http_provides_contract, validate_static_module_topology_contract,
        validate_static_module_ui_i18n_contract,
    };
    use semver::Version;
    use std::collections::HashMap;

    #[test]
    fn ui_classification_derives_and_normalizes_only_matching_surface_contracts() {
        assert_eq!(
            resolve_static_module_ui_classification(None, true, false),
            Ok("admin_only".to_string())
        );
        assert_eq!(
            resolve_static_module_ui_classification(Some("no-ui"), false, false),
            Ok("no_ui".to_string())
        );
        assert_eq!(
            resolve_static_module_ui_classification(Some("storefront_only"), true, false),
            Err(StaticModuleUiClassificationError::Invalid {
                value: "storefront_only".to_string(),
            })
        );
    }

    #[test]
    fn platform_version_constraints_normalize_bare_versions_in_owner_contract() {
        let current = Version::parse("1.4.2").expect("valid test version");
        assert_eq!(
            static_module_platform_version_is_compatible(&current, Some("1.4"), Some("1.5")),
            Ok(true)
        );
        assert_eq!(
            static_module_platform_version_is_compatible(&current, Some("^2.0"), None),
            Ok(false)
        );
        assert_eq!(
            static_module_platform_version_is_compatible(&current, Some("not-semver"), None),
            Err(StaticModulePlatformVersionError::InvalidMinimum {
                value: "not-semver".to_string(),
            })
        );
    }

    #[test]
    fn ui_i18n_contract_normalizes_locales_before_host_filesystem_checks() {
        let resolved = validate_static_module_ui_i18n_contract(&StaticModuleUiI18nContract {
            default_locale: Some("en".to_string()),
            supported_locales: vec!["ru".to_string(), "en".to_string(), "en".to_string()],
            leptos_locales_path: Some("admin/locales".to_string()),
            next_messages_path: None,
            has_leptos_crate: true,
            has_next_package: false,
        })
        .expect("valid static UI i18n contract");
        assert_eq!(resolved.supported_locales, vec!["en", "ru"]);
        assert_eq!(
            resolved.leptos_locales_path.as_deref(),
            Some("admin/locales")
        );

        assert_eq!(
            validate_static_module_ui_i18n_contract(&StaticModuleUiI18nContract {
                default_locale: None,
                supported_locales: vec!["en".to_string()],
                leptos_locales_path: Some("admin/locales".to_string()),
                next_messages_path: None,
                has_leptos_crate: false,
                has_next_package: false,
            }),
            Err(StaticModuleUiI18nValidationError::LeptosPathWithoutCrate)
        );
    }

    #[test]
    fn http_providers_reject_ambiguous_static_routes() {
        assert_eq!(
            validate_static_module_http_provides_contract(StaticModuleHttpProvidesContract {
                has_routes: true,
                has_axum_router: true,
                has_webhook_routes: false,
                has_axum_webhook_router: false,
            }),
            Err(StaticModuleHttpProvidesValidationError::RoutesAndAxumRouter)
        );
        assert_eq!(
            validate_static_module_http_provides_contract(StaticModuleHttpProvidesContract {
                has_routes: true,
                has_axum_router: false,
                has_webhook_routes: false,
                has_axum_webhook_router: false,
            }),
            Ok(())
        );
    }

    #[test]
    fn entrypoint_resolution_qualifies_crate_local_bindings_in_the_owner() {
        let resolved = resolve_static_module_entrypoints(StaticModuleEntrypointContract {
            crate_name: "rustok-example-module".to_string(),
            entry_type: Some("crate::Module".to_string()),
            graphql_query_type: Some("Query".to_string()),
            http_routes_fn: Some("crate::http::routes".to_string()),
            ..Default::default()
        })
        .expect("valid static entrypoints");
        assert_eq!(
            resolved.entry_type.as_deref(),
            Some("rustok_example_module::Module")
        );
        assert_eq!(
            resolved.graphql_query_type.as_deref(),
            Some("rustok_example_module::Query")
        );
        assert_eq!(
            resolved.http_routes_fn.as_deref(),
            Some("rustok_example_module::http::routes")
        );

        assert!(matches!(
            resolve_static_module_entrypoints(StaticModuleEntrypointContract {
                http_routes_fn: Some("routes".to_string()),
                http_axum_router_fn: Some("router".to_string()),
                ..Default::default()
            }),
            Err(StaticModuleEntrypointValidationError::Http(
                StaticModuleHttpProvidesValidationError::RoutesAndAxumRouter
            ))
        ));
    }

    #[test]
    fn topology_rejects_a_dependency_version_outside_the_owner_contract() {
        let mut modules = HashMap::new();
        modules.insert(
            "consumer".to_string(),
            StaticModuleTopologyModule {
                dependencies: vec!["provider".to_string()],
                dependency_version_requirements: HashMap::from([(
                    "provider".to_string(),
                    "^2.0".to_string(),
                )]),
                ..Default::default()
            },
        );
        modules.insert(
            "provider".to_string(),
            StaticModuleTopologyModule {
                version: Some("1.0.0".to_string()),
                ..Default::default()
            },
        );
        assert_eq!(
            validate_static_module_topology_contract(&StaticModuleTopologyContract {
                modules,
                default_enabled: vec!["consumer".to_string()],
                platform_version: Some(Version::parse("1.0.0").expect("valid test version")),
            }),
            Err(
                StaticModuleTopologyValidationError::IncompatibleDependencyVersion {
                    slug: "consumer".to_string(),
                    dependency: "provider".to_string(),
                    required: "^2.0".to_string(),
                    installed: "1.0.0".to_string(),
                }
            )
        );
    }
}
