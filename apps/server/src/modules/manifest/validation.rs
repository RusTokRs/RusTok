use super::types::*;
use rustok_api::normalize_locale_tag;
use semver::{Version, VersionReq};
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

pub fn merge_module_package_manifest(
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

pub fn qualify_module_type_path(crate_name: &str, value: Option<&str>) -> Option<String> {
    let value = value?.trim();
    if value.is_empty() {
        return None;
    }

    let crate_ident = crate_name.replace('-', "_");
    let relative = value.strip_prefix("crate::").unwrap_or(value);
    Some(format!("{crate_ident}::{relative}"))
}

pub fn qualify_module_member_path(crate_name: &str, value: Option<&str>) -> Option<String> {
    qualify_module_type_path(crate_name, value)
}

pub fn is_valid_module_setting_key(value: &str) -> bool {
    !value.is_empty()
        && value
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '_' || ch == '-')
}

pub fn is_supported_setting_type(value_type: &str) -> bool {
    matches!(
        value_type,
        "string" | "integer" | "number" | "boolean" | "object" | "array" | "json" | "any"
    )
}

pub fn declared_object_keys(spec: &ModuleSettingSpec) -> Vec<String> {
    if !spec.properties.is_empty() {
        let mut keys = spec.properties.keys().cloned().collect::<Vec<_>>();
        keys.sort();
        keys
    } else {
        spec.object_keys.clone()
    }
}

pub fn declared_item_type(spec: &ModuleSettingSpec) -> Option<&str> {
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

pub fn setting_value_matches_type(value_type: &str, value: &serde_json::Value) -> bool {
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

pub fn validate_setting_spec(
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

pub fn validate_setting_value(
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

pub fn normalize_module_settings(
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

pub fn validate_module_package_metadata(
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

pub fn normalize_module_ui_classification(value: &str) -> Option<String> {
    let normalized = value.trim().to_ascii_lowercase().replace('-', "_");
    match normalized.as_str() {
        "dual_surface" | "admin_only" | "storefront_only" | "no_ui" | "capability_only"
        | "future_ui" => Some(normalized),
        _ => None,
    }
}

pub fn resolved_catalog_module_ui_classification(
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

    Ok(merge_module_package_manifest(
        spec.clone(),
        package_manifest,
    ))
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

pub fn is_valid_module_ownership(value: &str) -> bool {
    matches!(value, "first_party" | "third_party")
}

pub fn is_valid_trust_level(value: &str) -> bool {
    matches!(value, "core" | "verified" | "unverified" | "private")
}

pub fn is_valid_module_slug(value: &str) -> bool {
    !value.is_empty()
        && value
            .chars()
            .all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '-' || ch == '_')
}

pub fn is_valid_admin_surface(value: &str) -> bool {
    !value.is_empty()
        && value
            .chars()
            .all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '-')
}

pub fn normalize_version_req(raw: &str, is_max: bool) -> String {
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

pub fn current_platform_version() -> Option<Version> {
    Version::parse(env!("CARGO_PKG_VERSION")).ok()
}

pub fn validate_admin_surfaces(
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

pub fn validate_catalog_metadata(
    slug: &str,
    spec: &ManifestModuleSpec,
) -> Result<(), ManifestError> {
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

pub fn validate_marketplace_metadata(
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

pub fn validate_marketplace_asset_url(
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

pub fn validate_urls(urls: &[String], field: &str) -> Result<(), ManifestError> {
    for value in urls {
        validate_url(value, field)?;
    }

    Ok(())
}

pub fn validate_url(value: &str, field: &str) -> Result<(), ManifestError> {
    reqwest::Url::parse(value).map_err(|error| {
        ManifestError::InvalidBuildSurface(format!(
            "{field} contains invalid URL '{value}': {error}"
        ))
    })?;
    Ok(())
}
