use super::*;

pub(crate) fn validate_module_entry_type_contract(
    slug: &str,
    manifest: &ModulePackageManifest,
    module_root: &Path,
) -> Result<()> {
    let lib_path = module_root.join("src").join("lib.rs");
    if !lib_path.exists() {
        return Ok(());
    }

    let content = fs::read_to_string(&lib_path)
        .with_context(|| format!("Failed to read {}", lib_path.display()))?;
    let has_runtime_module_impl = content.contains("impl RusToKModule for");
    let entry_type = manifest
        .crate_contract
        .entry_type
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty());

    if !has_runtime_module_impl {
        if entry_type.is_some() {
            anyhow::bail!(
                "Module '{slug}' declares crate.entry_type in rustok-module.toml, but {} does not implement RusToKModule",
                lib_path.display()
            );
        }
        return Ok(());
    }

    let Some(entry_type) = entry_type else {
        anyhow::bail!(
            "Module '{slug}' must declare [crate].entry_type in rustok-module.toml because {} implements RusToKModule",
            lib_path.display()
        );
    };

    let has_entry_struct = content.contains(&format!("pub struct {entry_type};"))
        || content.contains(&format!("pub struct {entry_type} {{"))
        || content.contains(&format!("pub struct {entry_type}<"))
        || content.contains(&format!("struct {entry_type};"))
        || content.contains(&format!("struct {entry_type} {{"))
        || content.contains(&format!("struct {entry_type}<"));
    if !has_entry_struct {
        anyhow::bail!(
            "Module '{slug}' declares crate.entry_type='{}', but {} is missing runtime struct '{}'",
            entry_type,
            lib_path.display(),
            entry_type
        );
    }

    if !content.contains(&format!("impl RusToKModule for {entry_type}")) {
        anyhow::bail!(
            "Module '{slug}' declares crate.entry_type='{}', but {} is missing 'impl RusToKModule for {}'",
            entry_type,
            lib_path.display(),
            entry_type
        );
    }

    Ok(())
}

pub(crate) fn validate_module_runtime_metadata_contract(
    slug: &str,
    manifest: &ModulePackageManifest,
    module_root: &Path,
) -> Result<()> {
    let lib_path = module_root.join("src").join("lib.rs");
    if !lib_path.exists() {
        return Ok(());
    }

    let content = fs::read_to_string(&lib_path)
        .with_context(|| format!("Failed to read {}", lib_path.display()))?;
    if !content.contains("impl RusToKModule for") {
        return Ok(());
    }

    let runtime_slug = extract_runtime_string_method(&content, "slug").with_context(|| {
        format!(
            "Module '{slug}' must expose fn slug(&self) -> &'static str in {}",
            lib_path.display()
        )
    })?;
    let runtime_name = extract_runtime_string_method(&content, "name").with_context(|| {
        format!(
            "Module '{slug}' must expose fn name(&self) -> &'static str in {}",
            lib_path.display()
        )
    })?;
    let runtime_description =
        extract_runtime_string_method(&content, "description").with_context(|| {
            format!(
                "Module '{slug}' must expose fn description(&self) -> &'static str in {}",
                lib_path.display()
            )
        })?;

    if manifest.module.slug.trim() != runtime_slug {
        anyhow::bail!(
            "Module '{slug}' slug mismatch between rustok-module.toml ('{}') and RusToKModule::slug() ('{}')",
            manifest.module.slug.trim(),
            runtime_slug
        );
    }

    if manifest.module.name.trim() != runtime_name {
        anyhow::bail!(
            "Module '{slug}' name mismatch between rustok-module.toml ('{}') and RusToKModule::name() ('{}')",
            manifest.module.name.trim(),
            runtime_name
        );
    }

    if manifest.module.description.trim() != runtime_description {
        anyhow::bail!(
            "Module '{slug}' description mismatch between rustok-module.toml ('{}') and RusToKModule::description() ('{}')",
            manifest.module.description.trim(),
            runtime_description
        );
    }

    Ok(())
}
