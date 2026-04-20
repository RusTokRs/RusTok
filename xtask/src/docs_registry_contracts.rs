use super::*;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum RegistryDocsSection {
    Core,
    Optional,
}

#[derive(Debug)]
struct RegistryDocsRow {
    section: RegistryDocsSection,
    crate_name: String,
    dependencies: HashSet<String>,
}

pub(crate) fn validate_module_registry_docs_contract(
    manifest_path: &Path,
    slug: &str,
    spec: &ModuleSpec,
) -> Result<()> {
    let registry_path = manifest_path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join("docs")
        .join("modules")
        .join("registry.md");
    let rows = load_registry_docs_rows(&registry_path)?;
    let row = rows.get(slug).with_context(|| {
        format!(
            "Module '{slug}' is missing central registry row in {}",
            registry_path.display()
        )
    })?;

    let expected_section = if spec.required {
        RegistryDocsSection::Core
    } else {
        RegistryDocsSection::Optional
    };
    if row.section != expected_section {
        anyhow::bail!(
            "Module '{slug}' is documented in the wrong docs/modules/registry.md section: expected {}, got {}",
            registry_docs_section_name(expected_section),
            registry_docs_section_name(row.section)
        );
    }

    if row.crate_name != spec.crate_name {
        anyhow::bail!(
            "Module '{slug}' crate mismatch between modules.toml and docs/modules/registry.md: modules.toml='{}', docs='{}'",
            spec.crate_name,
            row.crate_name
        );
    }

    let manifest_dependencies = normalize_dependency_set(spec.depends_on.as_deref().unwrap_or(&[]));
    if row.dependencies != manifest_dependencies {
        anyhow::bail!(
            "Module '{slug}' dependency mismatch between modules.toml and docs/modules/registry.md: modules.toml={:?}, docs={:?}",
            manifest_dependencies,
            row.dependencies
        );
    }

    Ok(())
}

pub(crate) fn validate_central_module_registry_inventory_contract(
    manifest_path: &Path,
    manifest: &Manifest,
) -> Result<()> {
    let registry_path = manifest_path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join("docs")
        .join("modules")
        .join("registry.md");
    let rows = load_registry_docs_rows(&registry_path)?;

    for (slug, spec) in &manifest.modules {
        validate_module_registry_docs_contract(manifest_path, slug, spec)?;
    }

    let documented = rows.keys().cloned().collect::<HashSet<_>>();
    let declared = manifest.modules.keys().cloned().collect::<HashSet<_>>();
    let undocumented = documented
        .difference(&declared)
        .cloned()
        .collect::<Vec<_>>();
    if !undocumented.is_empty() {
        anyhow::bail!(
            "docs/modules/registry.md contains module rows not present in modules.toml: {}",
            undocumented.join(", ")
        );
    }

    Ok(())
}

fn load_registry_docs_rows(path: &Path) -> Result<HashMap<String, RegistryDocsRow>> {
    let content =
        fs::read_to_string(path).with_context(|| format!("Failed to read {}", path.display()))?;
    let mut rows = HashMap::new();
    let mut section = None;

    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("## ") {
            section = None;
        }
        if trimmed.starts_with("### Core") {
            section = Some(RegistryDocsSection::Core);
            continue;
        }
        if trimmed.starts_with("### Optional") {
            section = Some(RegistryDocsSection::Optional);
            continue;
        }
        if !trimmed.starts_with('|')
            || trimmed.starts_with("|---")
            || trimmed.starts_with("| Slug |")
        {
            continue;
        }

        let Some(active_section) = section else {
            continue;
        };

        let columns = trimmed
            .trim_matches('|')
            .split('|')
            .map(|column| column.trim())
            .collect::<Vec<_>>();
        let minimum_columns = match active_section {
            RegistryDocsSection::Core => 3,
            RegistryDocsSection::Optional => 4,
        };
        if columns.len() < minimum_columns {
            continue;
        }

        let slug = strip_wrapping_backticks(columns[0]);
        let crate_name = strip_wrapping_backticks(columns[1]);
        if slug.is_empty() || crate_name.is_empty() {
            continue;
        }

        let dependencies = match active_section {
            RegistryDocsSection::Core => HashSet::new(),
            RegistryDocsSection::Optional => extract_backtick_tokens(columns[2]),
        };

        if rows
            .insert(
                slug.clone(),
                RegistryDocsRow {
                    section: active_section,
                    crate_name,
                    dependencies,
                },
            )
            .is_some()
        {
            anyhow::bail!("docs/modules/registry.md contains duplicate module row for '{slug}'");
        }
    }

    Ok(rows)
}

fn strip_wrapping_backticks(value: &str) -> String {
    value
        .trim()
        .trim_start_matches('`')
        .trim_end_matches('`')
        .trim()
        .to_string()
}

fn extract_backtick_tokens(value: &str) -> HashSet<String> {
    let mut dependencies = HashSet::new();
    let mut remainder = value;

    while let Some(start) = remainder.find('`') {
        let after_start = &remainder[start + 1..];
        let Some(end) = after_start.find('`') else {
            break;
        };
        let dependency = after_start[..end].trim();
        if !dependency.is_empty() {
            dependencies.insert(dependency.to_string());
        }
        remainder = &after_start[end + 1..];
    }

    dependencies
}

fn registry_docs_section_name(section: RegistryDocsSection) -> &'static str {
    match section {
        RegistryDocsSection::Core => "Core",
        RegistryDocsSection::Optional => "Optional",
    }
}
