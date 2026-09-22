use super::*;

pub(crate) fn is_valid_module_ownership(value: &str) -> bool {
    matches!(value, "first_party" | "third_party")
}

pub(crate) fn is_valid_trust_level(value: &str) -> bool {
    matches!(value, "core" | "verified" | "unverified" | "private")
}

fn is_valid_admin_surface(value: &str) -> bool {
    !value.is_empty()
        && value
            .chars()
            .all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '-')
}

pub(crate) fn validate_admin_surfaces(
    slug: &str,
    field: &str,
    surfaces: &[String],
) -> Result<HashSet<String>> {
    let mut normalized = HashSet::new();

    for surface in surfaces {
        let surface = surface.trim();
        if !is_valid_admin_surface(surface) {
            anyhow::bail!(
                "Module '{}' has invalid admin surface '{}' in {}",
                slug,
                surface,
                field
            );
        }
        normalized.insert(surface.to_string());
    }

    Ok(normalized)
}

pub(crate) fn module_validate_command(args: &[String]) -> Result<()> {
    let manifest_path = manifest_path();
    let manifest = load_manifest_from(&manifest_path)?;
    let workspace_manifest = load_workspace_manifest()?;
    let explicit_slug = args.first().map(String::as_str);
    let targets = selected_modules(&manifest, explicit_slug)?;

    println!("Validating module publish-readiness contracts...");

    for (slug, spec) in targets {
        let preview =
            build_module_publish_preview(&manifest_path, slug, spec, &workspace_manifest)?;
        println!(
            "  PASS {slug} -> {} v{}",
            preview.crate_name, preview.version
        );
    }

    Ok(())
}

fn selected_modules<'a>(
    manifest: &'a Manifest,
    slug: Option<&'a str>,
) -> Result<Vec<(&'a str, &'a ModuleSpec)>> {
    if let Some(slug) = slug {
        let spec = manifest
            .modules
            .get(slug)
            .with_context(|| format!("Unknown module slug '{slug}'"))?;
        return Ok(vec![(slug, spec)]);
    }

    let mut modules = manifest
        .modules
        .iter()
        .filter(|(_, spec)| spec.source == "path")
        .map(|(slug, spec)| (slug.as_str(), spec))
        .collect::<Vec<_>>();
    modules.sort_by(|left, right| left.0.cmp(right.0));
    Ok(modules)
}

pub(crate) fn to_pascal_case(s: &str) -> String {
    s.split('-')
        .map(|word| {
            let mut chars = word.chars();
            match chars.next() {
                None => String::new(),
                Some(first) => {
                    first.to_uppercase().collect::<String>() + &chars.as_str().to_lowercase()
                }
            }
        })
        .collect()
}
