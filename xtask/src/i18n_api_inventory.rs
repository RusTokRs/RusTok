use super::*;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

const TARGET_PACKAGE: &str = "rustok-ui-i18n";
const COMPATIBILITY_SYMBOLS: &[&str] = &[
    "FluentValue",
    "LanguageIdentifier",
    "FluentCatalog",
    "push_locale_candidate",
    "push_unique",
];
const COMPATIBILITY_MODULES: &[&str] = &["bundle", "error", "locale", "macros", "messages"];
const SKIPPED_DIRECTORIES: &[&str] = &[
    ".git",
    ".next",
    "node_modules",
    "output",
    "target",
    "vendor",
];

#[derive(Debug, Deserialize)]
struct CargoMetadata {
    packages: Vec<CargoMetadataPackage>,
}

#[derive(Debug, Deserialize)]
struct CargoMetadataPackage {
    name: String,
    manifest_path: PathBuf,
    dependencies: Vec<CargoMetadataDependency>,
}

#[derive(Debug, Deserialize)]
struct CargoMetadataDependency {
    name: String,
    rename: Option<String>,
}

#[derive(Debug, Clone, Eq, Ord, PartialEq, PartialOrd, Serialize)]
struct I18nApiFinding {
    package: String,
    crate_alias: String,
    symbol: String,
    path: String,
    line: usize,
    evidence: &'static str,
}

#[derive(Debug, Serialize)]
struct I18nApiInventoryReport {
    consumer_packages: Vec<String>,
    findings: Vec<I18nApiFinding>,
}

pub(crate) fn i18n_api_inventory(args: &[String]) -> Result<()> {
    let json = match args {
        [] => false,
        [flag] if flag == "--json" => true,
        _ => anyhow::bail!("Usage: cargo xtask i18n-api-inventory [--json]"),
    };

    let report = collect_i18n_api_inventory()?;
    if json {
        println!("{}", serde_json::to_string_pretty(&report)?);
        return Ok(());
    }

    println!("rustok-ui-i18n compatibility API inventory");
    println!("  consumer packages: {}", report.consumer_packages.len());
    println!("  candidate usages: {}", report.findings.len());
    println!();

    if report.consumer_packages.is_empty() {
        println!("No workspace package declares a dependency on {TARGET_PACKAGE}.");
        return Ok(());
    }

    println!("Consumers:");
    for package in &report.consumer_packages {
        println!("  - {package}");
    }

    if report.findings.is_empty() {
        println!();
        println!("No compatibility-only symbol candidates were found in consumer Rust sources.");
        println!("This is repository evidence for review, not permission to remove exports automatically.");
        return Ok(());
    }

    println!();
    println!("Candidate compatibility usages:");
    for finding in &report.findings {
        println!(
            "  {}:{} [{}] {} via {} ({})",
            finding.path,
            finding.line,
            finding.package,
            finding.symbol,
            finding.crate_alias,
            finding.evidence
        );
    }
    println!();
    println!(
        "Evidence is intentionally conservative: same-file candidates should be reviewed before narrowing an export."
    );

    Ok(())
}

fn collect_i18n_api_inventory() -> Result<I18nApiInventoryReport> {
    let root = workspace_root();
    let metadata = cargo_metadata(&root)?;
    let mut consumer_packages = BTreeSet::new();
    let mut findings = BTreeSet::new();

    for package in metadata.packages {
        if package.name == TARGET_PACKAGE {
            continue;
        }

        let aliases = package
            .dependencies
            .iter()
            .filter(|dependency| dependency.name == TARGET_PACKAGE)
            .map(|dependency| {
                dependency
                    .rename
                    .as_deref()
                    .unwrap_or(&dependency.name)
                    .replace('-', "_")
            })
            .collect::<BTreeSet<_>>();

        if aliases.is_empty() {
            continue;
        }

        consumer_packages.insert(package.name.clone());
        let package_root = package
            .manifest_path
            .parent()
            .with_context(|| {
                format!(
                    "Failed to resolve package root for {}",
                    package.manifest_path.display()
                )
            })?
            .to_path_buf();
        let mut source_files = Vec::new();
        collect_rust_sources(&package_root, &mut source_files)?;
        source_files.sort();

        for source_path in source_files {
            let content = fs::read_to_string(&source_path)
                .with_context(|| format!("Failed to read {}", source_path.display()))?;
            let display_path = source_path
                .strip_prefix(&root)
                .unwrap_or(&source_path)
                .to_string_lossy()
                .replace('\\', "/");

            for alias in &aliases {
                scan_source_text(
                    &package.name,
                    alias,
                    &display_path,
                    &content,
                    &mut findings,
                );
            }
        }
    }

    Ok(I18nApiInventoryReport {
        consumer_packages: consumer_packages.into_iter().collect(),
        findings: findings.into_iter().collect(),
    })
}

fn cargo_metadata(root: &Path) -> Result<CargoMetadata> {
    let cargo = std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
    let output = Command::new(cargo)
        .args(["metadata", "--no-deps", "--format-version", "1"])
        .current_dir(root)
        .output()
        .context("Failed to run cargo metadata for i18n API inventory")?;

    if !output.status.success() {
        anyhow::bail!(
            "cargo metadata failed for i18n API inventory: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }

    serde_json::from_slice(&output.stdout).context("Failed to parse cargo metadata JSON")
}

fn collect_rust_sources(dir: &Path, files: &mut Vec<PathBuf>) -> Result<()> {
    let entries =
        fs::read_dir(dir).with_context(|| format!("Failed to read {}", dir.display()))?;

    for entry in entries {
        let entry = entry.with_context(|| format!("Failed to read entry under {}", dir.display()))?;
        let path = entry.path();
        let file_type = entry
            .file_type()
            .with_context(|| format!("Failed to inspect {}", path.display()))?;

        if file_type.is_symlink() {
            continue;
        }
        if file_type.is_dir() {
            let name = entry.file_name();
            if SKIPPED_DIRECTORIES
                .iter()
                .any(|skipped| name == std::ffi::OsStr::new(skipped))
            {
                continue;
            }
            // Nested Cargo packages are inventoried from their own cargo-metadata
            // package entry. Do not attribute their sources to the parent package.
            if path.join("Cargo.toml").is_file() {
                continue;
            }
            collect_rust_sources(&path, files)?;
            continue;
        }
        if file_type.is_file() && path.extension().and_then(|ext| ext.to_str()) == Some("rs") {
            files.push(path);
        }
    }

    Ok(())
}

fn scan_source_text(
    package: &str,
    crate_alias: &str,
    path: &str,
    content: &str,
    findings: &mut BTreeSet<I18nApiFinding>,
) {
    if !content.contains(crate_alias) {
        return;
    }

    for (index, line) in content.lines().enumerate() {
        for symbol in COMPATIBILITY_SYMBOLS {
            if !line.contains(symbol) {
                continue;
            }
            findings.insert(I18nApiFinding {
                package: package.to_string(),
                crate_alias: crate_alias.to_string(),
                symbol: (*symbol).to_string(),
                path: path.to_string(),
                line: index + 1,
                evidence: if line.contains(crate_alias) {
                    "direct-path"
                } else {
                    "same-file-candidate"
                },
            });
        }

        for module in COMPATIBILITY_MODULES {
            let module_path = format!("{module}::");
            if !line.contains(&module_path) {
                continue;
            }
            findings.insert(I18nApiFinding {
                package: package.to_string(),
                crate_alias: crate_alias.to_string(),
                symbol: format!("module::{module}"),
                path: path.to_string(),
                line: index + 1,
                evidence: if line.contains(crate_alias) {
                    "direct-path"
                } else {
                    "same-file-candidate"
                },
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scanner_keeps_multiline_use_tree_candidates() {
        let source = r#"
use rustok_ui_i18n::{
    FluentValue,
    bundle::build_fluent_catalog_report,
};
"#;
        let mut findings = BTreeSet::new();
        scan_source_text(
            "consumer",
            "rustok_ui_i18n",
            "crates/consumer/src/lib.rs",
            source,
            &mut findings,
        );

        assert!(findings.iter().any(|finding| finding.symbol == "FluentValue"));
        assert!(findings.iter().any(|finding| finding.symbol == "module::bundle"));
        assert!(findings
            .iter()
            .all(|finding| finding.evidence == "same-file-candidate"));
    }

    #[test]
    fn scanner_ignores_symbols_without_the_dependency_alias() {
        let source = "use unic_langid::LanguageIdentifier;\n";
        let mut findings = BTreeSet::new();
        scan_source_text(
            "consumer",
            "rustok_ui_i18n",
            "crates/consumer/src/lib.rs",
            source,
            &mut findings,
        );

        assert!(findings.is_empty());
    }

    #[test]
    fn scanner_marks_direct_paths_separately() {
        let source = "type Catalog = rustok_ui_i18n::FluentCatalog;\n";
        let mut findings = BTreeSet::new();
        scan_source_text(
            "consumer",
            "rustok_ui_i18n",
            "crates/consumer/src/lib.rs",
            source,
            &mut findings,
        );

        let finding = findings.iter().next().expect("direct usage should be found");
        assert_eq!(finding.symbol, "FluentCatalog");
        assert_eq!(finding.evidence, "direct-path");
    }
}
