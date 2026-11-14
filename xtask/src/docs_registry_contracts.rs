use super::*;
use crate::docs_markdown::{CORE_MODULES, Document, MODULE_COLUMNS, OPTIONAL_MODULES};

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
    if spec.runtime.trim() == "extension" {
        return Ok(());
    }

    let registry_path = manifest_path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join("docs")
        .join("modules")
        .join("registry.md");
    let rows = load_registry_docs_rows(&registry_path)?;
    validate_registry_row(&rows, slug, spec)
}

fn validate_registry_row(
    rows: &HashMap<String, RegistryDocsRow>,
    slug: &str,
    spec: &ModuleSpec,
) -> Result<()> {
    if spec.runtime.trim() == "extension" {
        return Ok(());
    }
    let row = rows.get(slug).with_context(|| {
        format!("Module '{slug}' is missing central registry row in docs/modules/registry.md")
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
        validate_registry_row(&rows, slug, spec)?;
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
    parse_registry_docs_rows(&content)
}

fn parse_registry_docs_rows(content: &str) -> Result<HashMap<String, RegistryDocsRow>> {
    let document = Document::parse(content);
    let mut rows = HashMap::new();
    for (section, heading) in [
        (RegistryDocsSection::Core, CORE_MODULES),
        (RegistryDocsSection::Optional, OPTIONAL_MODULES),
    ] {
        let table = document.table(heading, MODULE_COLUMNS)?;
        for row in &table.rows {
            let [slug, name, dependencies, _] = row.cells.as_slice() else {
                anyhow::bail!(
                    "Malformed module row in docs/modules/registry.md ({})",
                    heading.1
                );
            };
            let slug = slug.text.trim();
            let name = name.text.trim();
            anyhow::ensure!(
                !slug.is_empty() && !name.is_empty(),
                "Empty module identity in docs/modules/registry.md ({})",
                heading.1
            );
            anyhow::ensure!(
                rows.insert(
                    slug.to_owned(),
                    RegistryDocsRow {
                        section,
                        crate_name: name.to_owned(),
                        dependencies: dependencies
                            .code
                            .iter()
                            .map(|code| code.trim().to_owned())
                            .filter(|code| !code.is_empty())
                            .collect(),
                    }
                )
                .is_none(),
                "docs/modules/registry.md contains duplicate module row for '{slug}'"
            );
        }
    }
    Ok(rows)
}

fn registry_docs_section_name(section: RegistryDocsSection) -> &'static str {
    match section {
        RegistryDocsSection::Core => "Core",
        RegistryDocsSection::Optional => "Optional",
    }
}

#[cfg(test)]
mod tests {
    use super::parse_registry_docs_rows;

    #[test]
    fn padded_headers_and_alignment_rows_are_not_modules() {
        let rows = parse_registry_docs_rows(
            "### Core Modules\n| Slug      | Crate       | Dependencies | Role |\n| :---- | ----: | :----: | ---- |\n| `core` | `rustok-core` | none | core |\n### Optional Modules\n| Slug         | Crate            | Dependencies | Role |\n| ---- | ---- | ---- | ---- |\n| `example` | `rustok-example` | `core` | example |\n"
        ).unwrap();
        assert_eq!(rows.len(), 2);
        assert!(rows.contains_key("core"));
        assert!(rows["example"].dependencies.contains("core"));
    }

    #[test]
    fn real_duplicate_module_rows_still_fail() {
        let error = parse_registry_docs_rows("### Core Modules\n| Slug | Crate | Dependencies | Role |\n| --- | --- | --- | --- |\n\n### Optional Modules\n| Slug | Crate | Dependencies | Role |\n| --- | --- | --- | --- |\n| `example` | `rustok-example` | none | first |\n| `example` | `rustok-example` | none | duplicate |\n").unwrap_err();
        assert!(
            error
                .to_string()
                .contains("duplicate module row for 'example'")
        );
    }

    #[test]
    fn unrelated_subsection_tables_do_not_extend_module_inventory() {
        let rows = parse_registry_docs_rows("### Core Modules\n| Slug | Crate | Dependencies | Role |\n| --- | --- | --- | --- |\n\n### Optional Modules\n| Slug | Crate | Dependencies | Role |\n| --- | --- | --- | --- |\n| `example` | `rustok-example` | none | module |\n### Notes\n| label | detail | extra | note |\n").unwrap();
        assert_eq!(rows.len(), 1);
    }
}
