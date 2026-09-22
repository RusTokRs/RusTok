use super::*;

pub(crate) fn load_server_module_feature_map(
    workspace_root: &Path,
) -> Result<HashMap<String, HashSet<String>>> {
    let server_manifest_path = workspace_root
        .join("apps")
        .join("server")
        .join("Cargo.toml");
    let server_manifest = load_toml_value(&server_manifest_path)?;
    let feature_table = server_manifest
        .get("features")
        .and_then(TomlValue::as_table)
        .with_context(|| format!("{} is missing [features]", server_manifest_path.display()))?;

    let mut features = HashMap::new();
    for (feature_name, value) in feature_table {
        let values = value.as_array().with_context(|| {
            format!(
                "Feature '{}' in {} must be a TOML array",
                feature_name,
                server_manifest_path.display()
            )
        })?;
        let entries = values
            .iter()
            .filter_map(TomlValue::as_str)
            .map(|entry| entry.trim().to_string())
            .filter(|entry| !entry.is_empty())
            .collect::<HashSet<_>>();
        features.insert(feature_name.to_string(), entries);
    }

    Ok(features)
}

pub(crate) fn validate_default_enabled_server_contract(
    manifest_path: &Path,
    manifest: &Manifest,
) -> Result<()> {
    let Some(default_enabled) = manifest
        .settings
        .as_ref()
        .and_then(|settings| settings.default_enabled.as_ref())
    else {
        return Ok(());
    };

    let workspace_root = manifest_path.parent().with_context(|| {
        format!(
            "Failed to resolve workspace root from modules manifest {}",
            manifest_path.display()
        )
    })?;
    let server_features = load_server_module_feature_map(workspace_root)?;
    let server_default_features = server_features.get("default").cloned().unwrap_or_default();
    let invalid_required_defaults = default_enabled
        .iter()
        .map(|slug| slug.trim())
        .filter(|slug| !slug.is_empty())
        .filter(|slug| {
            manifest
                .modules
                .get(*slug)
                .map(|spec| spec.required)
                .unwrap_or(false)
        })
        .map(|slug| slug.to_string())
        .collect::<Vec<_>>();

    if !invalid_required_defaults.is_empty() {
        anyhow::bail!(
            "default_enabled must list only optional modules; required modules are always active: {}",
            invalid_required_defaults.join(", ")
        );
    }

    let default_enabled_set = default_enabled
        .iter()
        .map(|slug| slug.trim())
        .filter(|slug| !slug.is_empty())
        .collect::<HashSet<_>>();
    let missing_default_dependencies = default_enabled
        .iter()
        .map(|slug| slug.trim())
        .filter(|slug| !slug.is_empty())
        .filter_map(|slug| manifest.modules.get(slug).map(|spec| (slug, spec)))
        .flat_map(|(slug, spec)| {
            spec.depends_on
                .as_deref()
                .unwrap_or(&[])
                .iter()
                .map(move |dependency| (slug, dependency.trim()))
        })
        .filter(|(_, dependency)| !dependency.is_empty())
        .filter(|(_, dependency)| {
            manifest
                .modules
                .get(*dependency)
                .map(|spec| !spec.required)
                .unwrap_or(false)
        })
        .filter(|(_, dependency)| !default_enabled_set.contains(*dependency))
        .map(|(slug, dependency)| format!("{slug} -> {dependency}"))
        .collect::<Vec<_>>();

    if !missing_default_dependencies.is_empty() {
        anyhow::bail!(
            "default_enabled must include optional dependency closure: {}",
            missing_default_dependencies.join(", ")
        );
    }

    let missing_server_defaults = default_enabled
        .iter()
        .map(|slug| slug.trim())
        .filter(|slug| !slug.is_empty())
        .filter(|slug| {
            manifest
                .modules
                .get(*slug)
                .map(|spec| !spec.required)
                .unwrap_or(false)
        })
        .filter(|slug| !server_default_features.contains(&format!("mod-{slug}")))
        .map(|slug| slug.to_string())
        .collect::<Vec<_>>();

    if !missing_server_defaults.is_empty() {
        anyhow::bail!(
            "default_enabled modules must be present in apps/server default features: {}",
            missing_server_defaults.join(", ")
        );
    }

    Ok(())
}
