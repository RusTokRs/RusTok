use anyhow::{Context, Result};
use serde::Deserialize;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Deserialize)]
struct Manifest {
    schema: u32,
    #[allow(dead_code)]
    app: String,
    #[allow(dead_code)]
    build: Option<BuildConfig>,
    modules: HashMap<String, ModuleSpec>,
    settings: Option<Settings>,
}

#[derive(Debug, Deserialize)]
struct BuildConfig {
    #[allow(dead_code)]
    target: Option<String>,
    #[allow(dead_code)]
    profile: Option<String>,
    #[allow(dead_code)]
    deployment_profile: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ModuleSpec {
    #[serde(rename = "crate")]
    crate_name: String,
    source: String,
    path: Option<String>,
    version: Option<String>,
    git: Option<String>,
    #[allow(dead_code)]
    rev: Option<String>,
    #[allow(dead_code)]
    depends_on: Option<Vec<String>>,
    #[allow(dead_code)]
    features: Option<Vec<String>>,
}

#[derive(Debug, Deserialize)]
struct Settings {
    default_enabled: Option<Vec<String>>,
}

#[derive(Debug, Deserialize, Default)]
struct ModulePackageManifest {
    #[serde(default)]
    module: ModulePackageMetadata,
}

#[derive(Debug, Deserialize, Default)]
struct ModulePackageMetadata {
    #[serde(default)]
    ownership: String,
    #[serde(default)]
    trust_level: String,
    #[serde(default)]
    recommended_admin_surfaces: Vec<String>,
    #[serde(default)]
    showcase_admin_surfaces: Vec<String>,
}

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().collect();

    if args.len() < 2 {
        print_usage();
        return Ok(());
    }

    match args[1].as_str() {
        "generate-registry" => generate_registry()?,
        "validate-manifest" => validate_manifest()?,
        "list-modules" => list_modules()?,
        _ => {
            eprintln!("Unknown command: {}", args[1]);
            print_usage();
            std::process::exit(1);
        }
    }

    Ok(())
}

fn print_usage() {
    println!("Usage: cargo xtask <command>");
    println!();
    println!("Commands:");
    println!("  generate-registry   Generate ModuleRegistry from modules.toml");
    println!("  validate-manifest   Validate modules.toml and rustok-module.toml files");
    println!("  list-modules        List all configured modules");
}

fn manifest_path() -> PathBuf {
    PathBuf::from("modules.toml")
}

fn load_manifest() -> Result<Manifest> {
    load_manifest_from(&manifest_path())
}

fn load_manifest_from(path: &Path) -> Result<Manifest> {
    let content =
        fs::read_to_string(path).with_context(|| format!("Failed to read {}", path.display()))?;
    let manifest: Manifest =
        toml::from_str(&content).with_context(|| format!("Failed to parse {}", path.display()))?;

    if manifest.schema != 2 {
        anyhow::bail!("Unsupported manifest schema: {}", manifest.schema);
    }

    Ok(manifest)
}

fn module_package_manifest_path(manifest_path: &Path, spec: &ModuleSpec) -> Option<PathBuf> {
    if spec.source != "path" {
        return None;
    }

    let module_path = spec.path.as_ref()?;
    Some(
        manifest_path
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .join(module_path)
            .join("rustok-module.toml"),
    )
}

fn load_module_package_manifest(path: &Path) -> Result<ModulePackageManifest> {
    let content =
        fs::read_to_string(path).with_context(|| format!("Failed to read {}", path.display()))?;
    toml::from_str(&content).with_context(|| format!("Failed to parse {}", path.display()))
}

fn is_valid_module_ownership(value: &str) -> bool {
    matches!(value, "first_party" | "third_party")
}

fn is_valid_trust_level(value: &str) -> bool {
    matches!(value, "core" | "verified" | "unverified" | "private")
}

fn is_valid_admin_surface(value: &str) -> bool {
    !value.is_empty()
        && value
            .chars()
            .all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '-')
}

fn validate_admin_surfaces(
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

fn validate_module_package_metadata(slug: &str, metadata: &ModulePackageMetadata) -> Result<()> {
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

fn generate_registry() -> Result<()> {
    println!("Generating ModuleRegistry from modules.toml...");

    let manifest = load_manifest()?;
    let output_path = Path::new("apps/server/src/modules/generated.rs");

    fs::create_dir_all(output_path.parent().unwrap())
        .context("Failed to create modules directory")?;

    let mut code = String::new();
    code.push_str("// AUTO-GENERATED by `cargo xtask generate-registry`\n");
    code.push_str("// DO NOT EDIT MANUALLY\n");
    code.push_str("// Generated from modules.toml\n\n");
    code.push_str("use rustok_core::ModuleRegistry;\n\n");

    for (slug, spec) in &manifest.modules {
        let module_struct = to_pascal_case(slug);
        let crate_name = spec.crate_name.replace("-", "_");
        code.push_str(&format!("use {}::{}Module;\n", crate_name, module_struct));
    }

    code.push_str("\n/// Build ModuleRegistry from configured modules\n");
    code.push_str("pub fn build_registry() -> ModuleRegistry {\n");
    code.push_str("    let mut registry = ModuleRegistry::new();\n\n");

    for slug in manifest.modules.keys() {
        let module_struct = to_pascal_case(slug);
        code.push_str(&format!("    // Register {} module\n", slug));
        code.push_str(&format!(
            "    registry.register(Box::new({}Module::new()));\n\n",
            module_struct
        ));
    }

    code.push_str("    registry\n");
    code.push_str("}\n");

    fs::write(output_path, code).context("Failed to write generated.rs")?;

    println!("вњ“ Generated: {}", output_path.display());
    println!("  Registered {} modules", manifest.modules.len());

    Ok(())
}

fn validate_manifest() -> Result<()> {
    println!("Validating modules.toml and rustok-module.toml...");

    let manifest_path = manifest_path();
    let manifest = load_manifest_from(&manifest_path)?;
    let installed = manifest.modules.keys().cloned().collect::<HashSet<_>>();

    let missing_defaults = manifest
        .settings
        .as_ref()
        .and_then(|settings| settings.default_enabled.as_ref())
        .into_iter()
        .flatten()
        .filter(|slug| !installed.contains(*slug))
        .cloned()
        .collect::<Vec<_>>();

    if !missing_defaults.is_empty() {
        anyhow::bail!(
            "default_enabled contains modules not present in modules.toml: {}",
            missing_defaults.join(", ")
        );
    }

    let mut module_manifest_count = 0usize;

    for (slug, spec) in &manifest.modules {
        match spec.source.as_str() {
            "path" => {
                if spec.path.is_none() {
                    anyhow::bail!("Module '{}' has source='path' but no path specified", slug);
                }
            }
            "git" => {
                if spec.git.is_none() {
                    anyhow::bail!(
                        "Module '{}' has source='git' but no git URL specified",
                        slug
                    );
                }
            }
            "registry" | "crates-io" => {
                if spec.version.is_none() {
                    anyhow::bail!(
                        "Module '{}' has source='{}' but no version specified",
                        slug,
                        spec.source
                    );
                }
            }
            other => anyhow::bail!("Module '{}' has invalid source '{}'", slug, other),
        }

        let missing_dependencies = spec
            .depends_on
            .as_deref()
            .unwrap_or_default()
            .iter()
            .filter(|dependency| !installed.contains(*dependency))
            .cloned()
            .collect::<Vec<_>>();

        if !missing_dependencies.is_empty() {
            anyhow::bail!(
                "Module '{}' depends on missing modules: {}",
                slug,
                missing_dependencies.join(", ")
            );
        }

        if spec.source == "path" {
            let package_path =
                module_package_manifest_path(&manifest_path, spec).with_context(|| {
                    format!("Module '{}' has source='path' but no path specified", slug)
                })?;
            if !package_path.exists() {
                anyhow::bail!(
                    "Module '{}' requires rustok-module.toml at {}",
                    slug,
                    package_path.display()
                );
            }

            let package_manifest = load_module_package_manifest(&package_path)?;
            validate_module_package_metadata(slug, &package_manifest.module)?;
            module_manifest_count += 1;
        }
    }

    println!("вњ“ Manifest is valid");
    println!("  Schema: {}", manifest.schema);
    println!("  Modules: {}", manifest.modules.len());
    println!("  Module manifests: {}", module_manifest_count);

    Ok(())
}

fn list_modules() -> Result<()> {
    let manifest = load_manifest()?;

    println!("Configured modules:");
    println!();

    for (slug, spec) in &manifest.modules {
        println!("  {}:", slug);
        println!("    crate: {}", spec.crate_name);
        println!("    source: {}", spec.source);
        if let Some(ref path) = spec.path {
            println!("    path: {}", path);
        }
        if let Some(ref version) = spec.version {
            println!("    version: {}", version);
        }
        if let Some(ref depends_on) = spec.depends_on {
            println!("    depends_on: {:?}", depends_on);
        }
        println!();
    }

    Ok(())
}

fn to_pascal_case(s: &str) -> String {
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
