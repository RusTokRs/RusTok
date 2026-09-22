use super::*;

pub(crate) fn validate_host_ui_crate_wiring(
    slug: &str,
    crate_name: &str,
    host_manifest_path: &Path,
    expected_ui_manifest_path: Option<&Path>,
    propagated_features: &[&str],
) -> Result<()> {
    let host_manifest = load_toml_value(host_manifest_path)?;
    let host_dependencies = host_manifest
        .get("dependencies")
        .and_then(TomlValue::as_table)
        .with_context(|| format!("{} is missing [dependencies]", host_manifest_path.display()))?;
    if !host_dependencies.contains_key(crate_name) {
        anyhow::bail!(
            "Module '{slug}' declares UI crate '{}', but {} is missing dependency '{}'",
            crate_name,
            host_manifest_path.display(),
            crate_name
        );
    }

    let ui_crate_manifest_path =
        resolve_host_ui_crate_manifest_path(host_manifest_path, crate_name)?;
    if let Some(expected_ui_manifest_path) = expected_ui_manifest_path {
        let actual_ui_manifest_path =
            fs::canonicalize(&ui_crate_manifest_path).unwrap_or(ui_crate_manifest_path.clone());
        let expected_ui_manifest_path = fs::canonicalize(expected_ui_manifest_path)
            .unwrap_or_else(|_| expected_ui_manifest_path.to_path_buf());
        if actual_ui_manifest_path != expected_ui_manifest_path {
            anyhow::bail!(
                "Module '{slug}' declares UI crate '{}', but {} points to '{}' instead of canonical '{}'",
                crate_name,
                host_manifest_path.display(),
                actual_ui_manifest_path.display(),
                expected_ui_manifest_path.display()
            );
        }
    }

    let ui_crate_manifest = load_toml_value(&ui_crate_manifest_path)?;
    let ui_features = ui_crate_manifest
        .get("features")
        .and_then(TomlValue::as_table)
        .cloned()
        .unwrap_or_default();
    let host_features = host_manifest
        .get("features")
        .and_then(TomlValue::as_table)
        .cloned()
        .unwrap_or_default();

    for feature_name in propagated_features {
        if !ui_features.contains_key(*feature_name) {
            continue;
        }

        let Some(host_feature_entries) = host_features
            .get(*feature_name)
            .and_then(TomlValue::as_array)
        else {
            anyhow::bail!(
                "Module '{slug}' requires host feature '{}' in {} for UI crate '{}'",
                feature_name,
                host_manifest_path.display(),
                crate_name
            );
        };

        let expected_entry = format!("{crate_name}/{feature_name}");
        let has_entry = host_feature_entries.iter().any(|entry| {
            entry
                .as_str()
                .map(|value| value.trim() == expected_entry)
                .unwrap_or(false)
        });
        if !has_entry {
            anyhow::bail!(
                "Module '{slug}' declares UI crate '{}', but {} feature '{}' is missing '{}'",
                crate_name,
                host_manifest_path.display(),
                feature_name,
                expected_entry
            );
        }
    }

    Ok(())
}

fn resolve_host_ui_crate_manifest_path(
    host_manifest_path: &Path,
    crate_name: &str,
) -> Result<PathBuf> {
    let host_manifest = load_toml_value(host_manifest_path)?;
    let dependency = host_manifest
        .get("dependencies")
        .and_then(TomlValue::as_table)
        .and_then(|dependencies| dependencies.get(crate_name))
        .with_context(|| {
            format!(
                "{} is missing dependency '{}'",
                host_manifest_path.display(),
                crate_name
            )
        })?;
    let dependency_table = dependency.as_table().with_context(|| {
        format!(
            "Dependency '{}' in {} must be declared as an inline table",
            crate_name,
            host_manifest_path.display()
        )
    })?;
    let relative_path = dependency_table
        .get("path")
        .and_then(TomlValue::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .with_context(|| {
            format!(
                "Dependency '{}' in {} must declare path for host UI wiring validation",
                crate_name,
                host_manifest_path.display()
            )
        })?;

    let base_dir = host_manifest_path.parent().with_context(|| {
        format!(
            "Failed to resolve parent directory for {}",
            host_manifest_path.display()
        )
    })?;
    Ok(base_dir.join(relative_path).join("Cargo.toml"))
}
