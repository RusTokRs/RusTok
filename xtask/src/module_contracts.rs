use super::*;
use crate::manifest_validation::{
    is_valid_module_ownership, is_valid_trust_level, validate_admin_surfaces,
};

pub(crate) fn validate_module_package_metadata(
    slug: &str,
    metadata: &ModulePackageMetadata,
) -> Result<()> {
    let ownership = metadata.ownership.trim();
    if !is_valid_module_ownership(ownership) {
        anyhow::bail!("Module '{}' has invalid ownership '{}'", slug, ownership);
    }

    let trust_level = metadata.trust_level.trim();
    if !is_valid_trust_level(trust_level) {
        anyhow::bail!(
            "Module '{}' has invalid trust level '{}'",
            slug,
            trust_level
        );
    }

    if !matches!(metadata.runtime.trim(), "module" | "extension") {
        anyhow::bail!(
            "Module '{}' has invalid runtime '{}'; expected 'module' or 'extension'",
            slug,
            metadata.runtime
        );
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
        anyhow::bail!(
            "Module '{}' lists admin surface '{}' as both recommended and showcase",
            slug,
            surface
        );
    }

    Ok(())
}

pub(crate) fn validate_module_local_docs_file(
    slug: &str,
    path: &Path,
    required_headings: &[&str],
) -> Result<()> {
    if !path.exists() {
        anyhow::bail!(
            "Module '{slug}' requires local docs file {}",
            path.display()
        );
    }

    let content =
        fs::read_to_string(path).with_context(|| format!("Failed to read {}", path.display()))?;
    if module_local_docs_file_matches_current_english_contract(path, &content) {
        return Ok(());
    }
    for heading in required_headings {
        if !content.contains(heading) {
            anyhow::bail!(
                "Module '{slug}' local docs {} must contain heading '{}'",
                path.display(),
                heading
            );
        }
    }

    Ok(())
}

fn module_local_docs_file_matches_current_english_contract(path: &Path, content: &str) -> bool {
    let Some(file_name) = path.file_name().and_then(|name| name.to_str()) else {
        return false;
    };
    if file_name == "README.md" {
        return [
            "## Purpose",
            "## Responsibility Zone",
            "## Integration",
            "## Verification",
            "## Related Documents",
        ]
        .iter()
        .all(|heading| content.contains(heading));
    }
    if file_name == "implementation-plan.md" {
        return [
            "## Scope",
            "## Current State",
            "## Milestones",
            "## Verification",
            "## Update Rules",
        ]
        .iter()
        .all(|heading| content.contains(heading));
    }
    false
}
