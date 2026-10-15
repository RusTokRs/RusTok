use std::collections::{HashMap, VecDeque};
use crate::error::{ManifestError, Result};
use crate::types::{ModulePackageManifest, ModulesManifest};

/// Canonical module slug validator.
///
/// Slugs must be 1 to 48 characters long, consisting only of lowercase ASCII letters,
/// digits, and underscores, without leading or trailing underscores.
pub fn is_valid_module_slug(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 48
        && value.chars().all(|character| {
            character.is_ascii_lowercase() || character.is_ascii_digit() || character == '_'
        })
        && !value.starts_with('_')
        && !value.ends_with('_')
}

/// Supported UI classifications according to `docs/modules/manifest.md`.
pub fn is_valid_ui_classification(value: &str) -> bool {
    matches!(
        value,
        "dual_surface"
            | "admin_only"
            | "storefront_only"
            | "no_ui"
            | "capability_only"
            | "future_ui"
    )
}

/// Supported module ownership values.
pub fn is_valid_ownership(value: &str) -> bool {
    matches!(value, "first_party" | "third_party")
}

/// Supported module trust levels.
pub fn is_valid_trust_level(value: &str) -> bool {
    matches!(value, "core" | "verified" | "unverified" | "private")
}

/// Validate platform composition contract (`modules.toml`).
pub fn validate_modules_manifest(manifest: &ModulesManifest) -> Result<()> {
    if manifest.schema != 2 {
        return Err(ManifestError::UnsupportedSchema(manifest.schema));
    }

    // 1. Validate slugs
    for slug in manifest.modules.keys() {
        if !is_valid_module_slug(slug) {
            return Err(ManifestError::InvalidSlug(slug.clone()));
        }
    }

    // 2. Validate default_enabled
    if let Some(settings) = &manifest.settings {
        if let Some(default_enabled) = &settings.default_enabled {
            for slug in default_enabled {
                let Some(spec) = manifest.modules.get(slug) else {
                    return Err(ManifestError::UnknownModule(slug.clone()));
                };
                if spec.required {
                    return Err(ManifestError::Validation(format!(
                        "Module '{slug}' is marked as required=true and must not be in default_enabled"
                    )));
                }
            }
        }
    }

    // 3. Validate dependency graph (depends_on)
    validate_dependency_graph(manifest)?;

    Ok(())
}

fn validate_dependency_graph(manifest: &ModulesManifest) -> Result<()> {
    let mut in_degrees: HashMap<&str, usize> = HashMap::new();
    let mut adjacency: HashMap<&str, Vec<&str>> = HashMap::new();

    for slug in manifest.modules.keys() {
        in_degrees.insert(slug.as_str(), 0);
        adjacency.insert(slug.as_str(), Vec::new());
    }

    for (slug, spec) in &manifest.modules {
        if let Some(deps) = &spec.depends_on {
            for dep in deps {
                if !manifest.modules.contains_key(dep) {
                    return Err(ManifestError::MissingDependency(
                        slug.clone(),
                        dep.clone(),
                    ));
                }
                // dep -> slug edge
                adjacency.entry(dep.as_str()).or_default().push(slug.as_str());
                *in_degrees.entry(slug.as_str()).or_default() += 1;
            }
        }
    }

    // Topological sort (Kahn's algorithm)
    let mut queue: VecDeque<&str> = in_degrees
        .iter()
        .filter_map(|(&node, &degree)| if degree == 0 { Some(node) } else { None })
        .collect();

    let mut visited_count = 0;
    while let Some(node) = queue.pop_front() {
        visited_count += 1;
        if let Some(neighbors) = adjacency.get(node) {
            for &neighbor in neighbors {
                if let Some(degree) = in_degrees.get_mut(neighbor) {
                    *degree -= 1;
                    if *degree == 0 {
                        queue.push_back(neighbor);
                    }
                }
            }
        }
    }

    if visited_count != manifest.modules.len() {
        let cyclic_nodes: Vec<String> = in_degrees
            .into_iter()
            .filter_map(|(node, degree)| if degree > 0 { Some(node.to_string()) } else { None })
            .collect();
        return Err(ManifestError::CyclicDependency(cyclic_nodes.join(", ")));
    }

    Ok(())
}

/// Validate scoped module package contract (`rustok-module.toml`).
pub fn validate_module_package_manifest(manifest: &ModulePackageManifest) -> Result<()> {
    let slug = &manifest.module.slug;
    if !is_valid_module_slug(slug) {
        return Err(ManifestError::InvalidSlug(slug.clone()));
    }

    let ownership = &manifest.module.ownership;
    if !is_valid_ownership(ownership) {
        return Err(ManifestError::InvalidOwnership(
            ownership.clone(),
            slug.clone(),
        ));
    }

    let trust_level = &manifest.module.trust_level;
    if !is_valid_trust_level(trust_level) {
        return Err(ManifestError::InvalidTrustLevel(
            trust_level.clone(),
            slug.clone(),
        ));
    }

    let ui_classification = &manifest.module.ui_classification;
    if !is_valid_ui_classification(ui_classification) {
        return Err(ManifestError::InvalidUiClassification(
            ui_classification.clone(),
            slug.clone(),
        ));
    }

    // Check UI surface consistency with ui_classification
    let has_admin_ui = manifest
        .provides
        .admin_ui
        .as_ref()
        .and_then(|admin| admin.leptos_crate.as_ref())
        .map(|krate| !krate.trim().is_empty())
        .unwrap_or(false);

    let has_storefront_ui = manifest
        .provides
        .storefront_ui
        .as_ref()
        .and_then(|storefront| storefront.leptos_crate.as_ref())
        .map(|krate| !krate.trim().is_empty())
        .unwrap_or(false);

    match (has_admin_ui, has_storefront_ui) {
        (true, true) => {
            if ui_classification != "dual_surface" {
                return Err(ManifestError::Validation(format!(
                    "Module '{slug}' declares both admin and storefront UI, but ui_classification is '{ui_classification}' (expected 'dual_surface')"
                )));
            }
        }
        (true, false) => {
            if ui_classification != "admin_only" {
                return Err(ManifestError::Validation(format!(
                    "Module '{slug}' declares admin UI only, but ui_classification is '{ui_classification}' (expected 'admin_only')"
                )));
            }
        }
        (false, true) => {
            if ui_classification != "storefront_only" {
                return Err(ManifestError::Validation(format!(
                    "Module '{slug}' declares storefront UI only, but ui_classification is '{ui_classification}' (expected 'storefront_only')"
                )));
            }
        }
        (false, false) => {
            if !matches!(
                ui_classification.as_str(),
                "no_ui" | "capability_only" | "future_ui"
            ) {
                return Err(ManifestError::Validation(format!(
                    "Module '{slug}' declares no UI surfaces, but ui_classification is '{ui_classification}' (expected 'no_ui', 'capability_only', or 'future_ui')"
                )));
            }
        }
    }

    Ok(())
}
