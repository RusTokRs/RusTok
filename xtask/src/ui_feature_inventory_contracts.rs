use super::*;

pub(crate) fn validate_host_ui_feature_inventory(
    host_manifest_path: &Path,
    host_dependencies: &toml::map::Map<String, TomlValue>,
    host_features: &toml::map::Map<String, TomlValue>,
    declared_ui_crates: &HashMap<String, (String, PathBuf)>,
    surface: &str,
) -> Result<()> {
    for host_feature_name in ["hydrate", "ssr"] {
        let Some(entries) = host_features
            .get(host_feature_name)
            .and_then(TomlValue::as_array)
        else {
            continue;
        };

        for entry in entries {
            let Some(entry) = entry
                .as_str()
                .map(str::trim)
                .filter(|value| !value.is_empty())
            else {
                continue;
            };
            let Some((crate_name, crate_feature)) = entry.split_once('/') else {
                continue;
            };
            if crate_feature != host_feature_name {
                continue;
            }

            let declared_ui = declared_ui_crates.get(crate_name);
            let host_dependency = host_dependencies
                .get(crate_name)
                .and_then(TomlValue::as_table)
                .and_then(|dependency| dependency.get("path"))
                .and_then(TomlValue::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty());

            let Some(host_dependency_path) = host_dependency else {
                if declared_ui.is_some() {
                    anyhow::bail!(
                        "Host UI feature drift: {} feature '{}' references '{}', but {} is missing dependency '{}'",
                        host_manifest_path.display(),
                        host_feature_name,
                        entry,
                        host_manifest_path.display(),
                        crate_name
                    );
                }
                continue;
            };

            let base_dir = host_manifest_path.parent().with_context(|| {
                format!(
                    "Failed to resolve parent directory for {}",
                    host_manifest_path.display()
                )
            })?;
            let actual_ui_manifest_path = base_dir.join(host_dependency_path).join("Cargo.toml");
            if !actual_ui_manifest_path.exists() {
                continue;
            }
            let surface_dir = actual_ui_manifest_path
                .parent()
                .and_then(|path| path.file_name())
                .and_then(|name| name.to_str());
            if surface_dir != Some(surface) {
                continue;
            }

            let Some(module_root) = actual_ui_manifest_path.parent().and_then(Path::parent) else {
                continue;
            };
            if !module_root.join("rustok-module.toml").exists() {
                continue;
            }

            let Some((owner_slug, expected_ui_manifest_path)) = declared_ui else {
                anyhow::bail!(
                    "Host UI feature drift: {} feature '{}' references '{}', but no module manifest declares it as {} UI",
                    host_manifest_path.display(),
                    host_feature_name,
                    entry,
                    surface
                );
            };

            let actual_ui_manifest_path =
                fs::canonicalize(&actual_ui_manifest_path).unwrap_or(actual_ui_manifest_path);
            let expected_ui_manifest_path = fs::canonicalize(expected_ui_manifest_path)
                .unwrap_or_else(|_| expected_ui_manifest_path.clone());
            if actual_ui_manifest_path != expected_ui_manifest_path {
                anyhow::bail!(
                    "Host UI feature drift: {} feature '{}' references '{}' for module '{}', but path '{}' does not match canonical '{}'",
                    host_manifest_path.display(),
                    host_feature_name,
                    entry,
                    owner_slug,
                    actual_ui_manifest_path.display(),
                    expected_ui_manifest_path.display()
                );
            }
        }
    }

    Ok(())
}
