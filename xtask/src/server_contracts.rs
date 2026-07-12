use super::*;
use crate::manifest_validation::to_pascal_case;

pub(crate) fn validate_module_server_registry_contract(
    manifest_path: &Path,
    slug: &str,
    spec: &ModuleSpec,
    manifest: &ModulePackageManifest,
    module_root: &Path,
) -> Result<()> {
    // Capability extensions are catalogued modules, but are deliberately not
    // `RusToKModule` registry entries. Their eventual host composition is
    // supplied by the generic runtime-extension contract, rather than an
    // AI-/capability-specific import in apps/server.
    if spec.runtime.trim() == "extension" {
        return Ok(());
    }

    let workspace_root = manifest_path.parent().with_context(|| {
        format!(
            "Failed to resolve workspace root from modules manifest {}",
            manifest_path.display()
        )
    })?;
    let server_modules_path = workspace_root
        .join("apps")
        .join("server")
        .join("src")
        .join("modules")
        .join("mod.rs");
    let server_modules_content = fs::read_to_string(&server_modules_path)
        .with_context(|| format!("Failed to read {}", server_modules_path.display()))?;
    let inferred_module_entry =
        infer_server_registry_module_expr(spec, manifest, module_root, slug)?.with_context(
            || format!("Module '{slug}' must resolve to a runtime ModuleRegistry entry"),
        )?;

    if spec.required {
        if !server_modules_registers_direct_entry(
            &server_modules_content,
            slug,
            &inferred_module_entry,
        ) {
            anyhow::bail!(
                "Required module '{slug}' must be registered directly in apps/server/src/modules/mod.rs"
            );
        }
        return Ok(());
    }

    let server_features = load_server_module_feature_map(workspace_root)?;
    let feature_name = format!("mod-{slug}");
    let feature_entries = server_features.get(&feature_name).with_context(|| {
        format!(
            "Optional module '{slug}' must expose feature '{}' in apps/server/Cargo.toml",
            feature_name
        )
    })?;

    let dependency_feature = format!("dep:{}", spec.crate_name);
    let capability_only = manifest.module.ui_classification.trim() == "capability_only";
    let allows_always_linked_server_dependency =
        capability_only && !feature_entries.contains("dep:");
    if !feature_entries.contains(&dependency_feature) && !allows_always_linked_server_dependency {
        anyhow::bail!(
            "Optional module '{slug}' must wire feature '{}' to '{}' in apps/server/Cargo.toml",
            feature_name,
            dependency_feature
        );
    }

    let expected_module_features = spec
        .depends_on
        .as_deref()
        .unwrap_or(&[])
        .iter()
        .map(|dependency| format!("mod-{}", dependency.trim()))
        .collect::<HashSet<_>>();
    let actual_module_features = feature_entries
        .iter()
        .filter(|entry| entry.starts_with("mod-"))
        .cloned()
        .collect::<HashSet<_>>();
    if expected_module_features != actual_module_features {
        anyhow::bail!(
            "Optional module '{slug}' has server feature graph drift in apps/server/Cargo.toml: expected mod-dependencies {:?}, got {:?}",
            expected_module_features,
            actual_module_features
        );
    }

    let crate_ident = spec.crate_name.replace('-', "_");
    if !inferred_module_entry.starts_with(&format!("{crate_ident}::")) {
        anyhow::bail!(
            "Optional module '{slug}' resolved unexpected server registry entry '{}'; expected '{}' namespace",
            inferred_module_entry,
            crate_ident
        );
    }

    if server_modules_registers_direct_entry(&server_modules_content, slug, &inferred_module_entry)
    {
        anyhow::bail!(
            "Optional module '{slug}' must not be registered directly in apps/server/src/modules/mod.rs; use feature/codegen wiring instead"
        );
    }

    Ok(())
}

fn server_modules_registers_direct_entry(
    content: &str,
    slug: &str,
    inferred_module_entry: &str,
) -> bool {
    let entry_ident = inferred_module_entry
        .rsplit("::")
        .next()
        .unwrap_or(inferred_module_entry);
    let slug_pascal = format!("{}Module", to_pascal_case(slug));
    let slug_binding = format!("{}_module", slug.replace('-', "_"));

    [
        format!(".register({entry_ident})"),
        format!(".register({inferred_module_entry})"),
        format!(".register({slug_pascal})"),
        format!(".register({slug_binding})"),
    ]
    .into_iter()
    .any(|needle| content.contains(&needle))
}

fn infer_server_registry_module_expr(
    spec: &ModuleSpec,
    manifest: &ModulePackageManifest,
    module_root: &Path,
    slug: &str,
) -> Result<Option<String>> {
    if let Some(entry_type) = manifest
        .crate_contract
        .entry_type
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        return Ok(Some(format!(
            "{}::{}",
            spec.crate_name.replace('-', "_"),
            entry_type
        )));
    }

    if let Some(entry_type) = infer_runtime_module_entry_type(module_root)? {
        return Ok(Some(format!(
            "{}::{}",
            spec.crate_name.replace('-', "_"),
            entry_type
        )));
    }

    let has_package_manifest = module_root.join("rustok-module.toml").exists();
    if has_package_manifest {
        return Ok(None);
    }

    Ok(Some(format!(
        "{}::{}Module",
        spec.crate_name.replace('-', "_"),
        to_pascal_case(slug)
    )))
}
