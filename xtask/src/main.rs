use anyhow::{Context, Result};
use reqwest::blocking::Client;
use reqwest::Url;
use semver::Version;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::net::IpAddr;
use std::path::{Path, PathBuf};
use std::process::Command;
use toml::Value as TomlValue;

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
    #[serde(default)]
    marketplace: ModulePackageMarketplaceMetadata,
    #[serde(rename = "crate", default)]
    crate_contract: ModulePackageCrateContract,
    #[serde(default)]
    provides: ModulePackageProvides,
}

#[derive(Debug, Deserialize, Default)]
struct ModulePackageMetadata {
    #[serde(default)]
    slug: String,
    #[serde(default)]
    name: String,
    #[serde(default)]
    version: String,
    #[serde(default)]
    description: String,
    #[serde(default)]
    ownership: String,
    #[serde(default)]
    trust_level: String,
    #[serde(default)]
    recommended_admin_surfaces: Vec<String>,
    #[serde(default)]
    showcase_admin_surfaces: Vec<String>,
}

#[derive(Debug, Deserialize, Default)]
struct ModulePackageMarketplaceMetadata {
    #[serde(default)]
    category: Option<String>,
    #[serde(default)]
    tags: Vec<String>,
}

#[derive(Debug, Deserialize, Default)]
struct ModulePackageCrateContract {
    #[serde(default)]
    entry_type: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
struct ModulePackageProvides {
    #[serde(default)]
    admin_ui: Option<ModuleUiProvides>,
    #[serde(default)]
    storefront_ui: Option<ModuleUiProvides>,
}

#[derive(Debug, Deserialize, Default)]
struct ModuleUiProvides {
    #[serde(default)]
    leptos_crate: Option<String>,
}

#[derive(Debug, Serialize)]
struct ModulePublishDryRunPreview {
    slug: String,
    version: String,
    crate_name: String,
    module_name: String,
    module_description: String,
    ownership: String,
    trust_level: String,
    license: String,
    manifest_path: String,
    package_manifest_path: String,
    module_entry_type: Option<String>,
    marketplace: ModuleMarketplacePreview,
    ui_packages: ModuleUiPackagesPreview,
}

#[derive(Debug, Serialize)]
struct ModuleMarketplacePreview {
    category: Option<String>,
    tags: Vec<String>,
}

#[derive(Debug, Serialize)]
struct ModuleUiPackagesPreview {
    admin: Option<ModuleUiPackagePreview>,
    storefront: Option<ModuleUiPackagePreview>,
}

#[derive(Debug, Serialize)]
struct ModuleUiPackagePreview {
    crate_name: String,
    manifest_path: String,
}

#[derive(Debug, Serialize)]
struct RegistryPublishHttpRequest {
    schema_version: u32,
    dry_run: bool,
    module: RegistryPublishModuleHttpRequest,
}

#[derive(Debug, Serialize)]
struct RegistryPublishModuleHttpRequest {
    slug: String,
    version: String,
    crate_name: String,
    name: String,
    description: String,
    ownership: String,
    trust_level: String,
    license: String,
    entry_type: Option<String>,
    marketplace: RegistryPublishMarketplaceHttpRequest,
    ui_packages: RegistryPublishUiPackagesHttpRequest,
}

#[derive(Debug, Serialize)]
struct RegistryPublishMarketplaceHttpRequest {
    category: Option<String>,
    tags: Vec<String>,
}

#[derive(Debug, Serialize)]
struct RegistryPublishUiPackagesHttpRequest {
    admin: Option<RegistryPublishUiPackageHttpRequest>,
    storefront: Option<RegistryPublishUiPackageHttpRequest>,
}

#[derive(Debug, Serialize)]
struct RegistryPublishUiPackageHttpRequest {
    crate_name: String,
}

#[derive(Debug, Serialize)]
struct RegistryYankHttpRequest {
    schema_version: u32,
    dry_run: bool,
    slug: String,
    version: String,
    reason: Option<String>,
}

#[derive(Debug, Serialize)]
struct RegistryOwnerTransferHttpRequest {
    schema_version: u32,
    dry_run: bool,
    slug: String,
    new_owner_actor: String,
    reason: Option<String>,
}

#[derive(Debug, Serialize)]
struct RegistryValidationStageHttpRequest {
    schema_version: u32,
    dry_run: bool,
    stage: String,
    status: String,
    detail: Option<String>,
    requeue: bool,
}

#[derive(Debug, Deserialize, Serialize)]
struct RegistryMutationHttpResponse {
    accepted: bool,
    #[allow(dead_code)]
    action: Option<String>,
    #[allow(dead_code)]
    dry_run: Option<bool>,
    request_id: Option<String>,
    status: Option<String>,
    #[serde(default)]
    warnings: Vec<String>,
    #[serde(default)]
    errors: Vec<String>,
    next_step: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
struct RegistryPublishStatusHttpResponse {
    request_id: String,
    slug: String,
    version: String,
    status: String,
    accepted: bool,
    #[serde(default)]
    warnings: Vec<String>,
    #[serde(default)]
    errors: Vec<String>,
    next_step: Option<String>,
}

#[derive(Debug, Serialize)]
struct RegistryPublishValidationHttpRequest {
    schema_version: u32,
    dry_run: bool,
}

#[derive(Debug, Serialize)]
struct RegistryPublishDecisionHttpRequest {
    schema_version: u32,
    dry_run: bool,
    reason: Option<String>,
}

#[derive(Debug, Serialize)]
struct ModuleYankDryRunPreview {
    action: String,
    slug: String,
    version: String,
    crate_name: String,
    current_local_version: String,
    matches_local_version: bool,
    package_manifest_path: String,
}

#[derive(Debug, Serialize)]
struct ModuleOwnerTransferDryRunPreview {
    action: String,
    slug: String,
    crate_name: String,
    current_local_version: String,
    package_manifest_path: String,
    new_owner_actor: String,
    reason: Option<String>,
}

#[derive(Debug, Serialize)]
struct ModuleValidationStageDryRunPreview {
    action: String,
    request_id: String,
    stage: String,
    status: String,
    detail: Option<String>,
    requeue: bool,
}

#[derive(Debug, Serialize)]
struct ModuleTestPlanPreview {
    slug: String,
    version: String,
    commands: Vec<ModuleCommandPreview>,
}

#[derive(Debug, Serialize)]
struct ModuleCommandPreview {
    label: String,
    argv: Vec<String>,
}

#[derive(Debug)]
struct ResolvedCargoPackage {
    name: String,
    version: String,
    license: Option<String>,
    manifest_path: PathBuf,
}

const REGISTRY_MUTATION_SCHEMA_VERSION: u32 = 1;

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
        "module" => module_command(&args[2..])?,
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
    println!("  module validate     Validate module publish-readiness contracts");
    println!("  module test         Run or preview local module smoke checks");
    println!("  module publish      Create/preview a publish request and stop at review-ready unless --auto-approve is set");
    println!("  module stage        Record or requeue a follow-up validation stage");
    println!("  module owner-transfer Emit a dry-run owner transfer payload preview");
    println!("  module yank         Emit a dry-run yank payload preview");
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

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .map(PathBuf::from)
        .expect("xtask should live under the workspace root")
}

fn workspace_manifest_path() -> PathBuf {
    workspace_root().join("Cargo.toml")
}

fn load_workspace_manifest() -> Result<TomlValue> {
    let path = workspace_manifest_path();
    let content =
        fs::read_to_string(&path).with_context(|| format!("Failed to read {}", path.display()))?;
    toml::from_str(&content).with_context(|| format!("Failed to parse {}", path.display()))
}

fn load_toml_value(path: &Path) -> Result<TomlValue> {
    let content =
        fs::read_to_string(path).with_context(|| format!("Failed to read {}", path.display()))?;
    toml::from_str(&content).with_context(|| format!("Failed to parse {}", path.display()))
}

fn resolve_workspace_inherited_string(
    value: Option<&TomlValue>,
    workspace_manifest: &TomlValue,
    field: &str,
) -> Result<Option<String>> {
    let Some(value) = value else {
        return Ok(None);
    };

    if let Some(string) = value.as_str() {
        return Ok(Some(string.trim().to_string()));
    }

    if let Some(table) = value.as_table() {
        if table.get("workspace").and_then(TomlValue::as_bool) == Some(true) {
            let inherited = workspace_manifest
                .get("workspace")
                .and_then(TomlValue::as_table)
                .and_then(|workspace| workspace.get("package"))
                .and_then(TomlValue::as_table)
                .and_then(|package| package.get(field))
                .and_then(TomlValue::as_str)
                .map(|value| value.trim().to_string());
            return Ok(inherited);
        }
    }

    anyhow::bail!("Unsupported package.{field} declaration in Cargo manifest")
}

fn load_resolved_cargo_package(
    path: &Path,
    workspace_manifest: &TomlValue,
) -> Result<ResolvedCargoPackage> {
    let manifest = load_toml_value(path)?;
    let package = manifest
        .get("package")
        .and_then(TomlValue::as_table)
        .with_context(|| format!("{} is missing [package]", path.display()))?;

    let name = package
        .get("name")
        .and_then(TomlValue::as_str)
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .with_context(|| format!("{} is missing package.name", path.display()))?;
    let version =
        resolve_workspace_inherited_string(package.get("version"), workspace_manifest, "version")?
            .filter(|value| !value.is_empty())
            .with_context(|| format!("{} is missing package.version", path.display()))?;
    let license =
        resolve_workspace_inherited_string(package.get("license"), workspace_manifest, "license")?
            .filter(|value| !value.is_empty());

    Ok(ResolvedCargoPackage {
        name,
        version,
        license,
        manifest_path: path.to_path_buf(),
    })
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

fn module_command(args: &[String]) -> Result<()> {
    if args.is_empty() {
        print_module_usage();
        anyhow::bail!("Missing module subcommand");
    }

    match args[0].as_str() {
        "validate" => module_validate_command(&args[1..]),
        "test" => module_test_command(&args[1..]),
        "publish" => module_publish_command(&args[1..]),
        "stage" => module_stage_command(&args[1..]),
        "owner-transfer" => module_owner_transfer_command(&args[1..]),
        "yank" => module_yank_command(&args[1..]),
        other => {
            print_module_usage();
            anyhow::bail!("Unknown module subcommand: {other}");
        }
    }
}

fn print_module_usage() {
    println!("Usage:");
    println!("  cargo xtask module validate [slug]");
    println!("  cargo xtask module test <slug> [--dry-run]");
    println!(
        "  cargo xtask module publish <slug> [--dry-run] [--registry-url <url>] [--auto-approve]"
    );
    println!(
        "  cargo xtask module stage <request-id> <stage> <status> [--dry-run] [--detail <text>] [--requeue] [--registry-url <url>]"
    );
    println!(
        "  cargo xtask module owner-transfer <slug> <new-owner-actor> [--dry-run] [--reason <text>] [--registry-url <url>]"
    );
    println!(
        "  cargo xtask module yank <slug> <version> [--dry-run] [--reason <text>] [--registry-url <url>]"
    );
}

fn module_validate_command(args: &[String]) -> Result<()> {
    let manifest_path = manifest_path();
    let manifest = load_manifest_from(&manifest_path)?;
    let workspace_manifest = load_workspace_manifest()?;
    let explicit_slug = args.first().map(String::as_str);
    let targets = selected_modules(&manifest, explicit_slug)?;
    let mut skipped_without_package_manifest = 0usize;

    println!("Validating module publish-readiness contracts...");

    for (slug, spec) in targets {
        if explicit_slug.is_none() {
            let Some(package_manifest_path) = module_package_manifest_path(&manifest_path, spec)
            else {
                continue;
            };
            if !package_manifest_path.exists() {
                skipped_without_package_manifest += 1;
                continue;
            }
        }

        let preview =
            build_module_publish_preview(&manifest_path, slug, spec, &workspace_manifest)?;
        println!(
            "  PASS {slug} -> {} v{}",
            preview.crate_name, preview.version
        );
    }

    if skipped_without_package_manifest > 0 {
        println!(
            "  Skipped {skipped_without_package_manifest} path modules without rustok-module.toml publish contract"
        );
    }

    Ok(())
}

fn module_publish_command(args: &[String]) -> Result<()> {
    if args.is_empty() {
        print_module_usage();
        anyhow::bail!("module publish requires a module slug");
    }

    let slug = args[0].as_str();
    let dry_run = args.iter().skip(1).any(|arg| arg == "--dry-run");
    let auto_approve = auto_approve_argument(&args[1..]);

    let manifest_path = manifest_path();
    let manifest = load_manifest_from(&manifest_path)?;
    let workspace_manifest = load_workspace_manifest()?;
    let spec = manifest
        .modules
        .get(slug)
        .with_context(|| format!("Unknown module slug '{slug}'"))?;
    let preview = build_module_publish_preview(&manifest_path, slug, spec, &workspace_manifest)?;
    let registry_url = registry_url_argument(&args[1..]);
    if dry_run {
        if let Some(registry_url) = registry_url {
            let payload = publish_via_registry_dry_run(&registry_url, &preview)?;
            println!("{payload}");
        } else {
            let payload = serde_json::to_string_pretty(&preview)?;
            println!("{payload}");
        }
    } else {
        let registry_url = registry_url.with_context(|| {
            "Live module publish requires --registry-url or RUSTOK_MODULE_REGISTRY_URL"
        })?;
        let payload = publish_via_registry_live(&registry_url, &preview, auto_approve)?;
        println!("{payload}");
    }

    Ok(())
}

fn module_test_command(args: &[String]) -> Result<()> {
    if args.is_empty() {
        print_module_usage();
        anyhow::bail!("module test requires a module slug");
    }

    let slug = args[0].as_str();
    let dry_run = args.iter().skip(1).any(|arg| arg == "--dry-run");
    let manifest_path = manifest_path();
    let manifest = load_manifest_from(&manifest_path)?;
    let workspace_manifest = load_workspace_manifest()?;
    let spec = manifest
        .modules
        .get(slug)
        .with_context(|| format!("Unknown module slug '{slug}'"))?;
    let preview = build_module_publish_preview(&manifest_path, slug, spec, &workspace_manifest)?;
    let plan = build_module_test_plan(&preview);

    if dry_run {
        let payload = serde_json::to_string_pretty(&plan)?;
        println!("{payload}");
        return Ok(());
    }

    println!(
        "Running local module smoke checks for {slug} v{}...",
        preview.version
    );
    for command in &plan.commands {
        println!("  > {}", command.argv.join(" "));
        run_command(command)?;
    }

    Ok(())
}

fn module_yank_command(args: &[String]) -> Result<()> {
    if args.len() < 2 {
        print_module_usage();
        anyhow::bail!("module yank requires a module slug and version");
    }

    let slug = args[0].as_str();
    let version = args[1].trim();
    let dry_run = args.iter().skip(2).any(|arg| arg == "--dry-run");
    Version::parse(version)
        .with_context(|| format!("module yank version '{version}' is not valid semver"))?;

    let manifest_path = manifest_path();
    let manifest = load_manifest_from(&manifest_path)?;
    let workspace_manifest = load_workspace_manifest()?;
    let spec = manifest
        .modules
        .get(slug)
        .with_context(|| format!("Unknown module slug '{slug}'"))?;
    let preview = build_module_publish_preview(&manifest_path, slug, spec, &workspace_manifest)?;
    let payload = ModuleYankDryRunPreview {
        action: "yank".to_string(),
        slug: slug.to_string(),
        version: version.to_string(),
        crate_name: preview.crate_name.clone(),
        current_local_version: preview.version.clone(),
        matches_local_version: preview.version == version,
        package_manifest_path: preview.package_manifest_path,
    };
    let registry_url = registry_url_argument(&args[2..]);
    let reason = reason_argument(&args[2..]);
    if dry_run {
        if let Some(registry_url) = registry_url {
            let remote_payload = yank_via_registry_dry_run(&registry_url, &payload, reason)?;
            println!("{remote_payload}");
        } else {
            println!("{}", serde_json::to_string_pretty(&payload)?);
        }
    } else {
        let registry_url = registry_url.with_context(|| {
            "Live module yank requires --registry-url or RUSTOK_MODULE_REGISTRY_URL"
        })?;
        let reason = reason.with_context(|| "Live module yank requires --reason <text>")?;
        let remote_payload = yank_via_registry_live(&registry_url, &payload, reason)?;
        println!("{remote_payload}");
    }

    Ok(())
}

fn module_owner_transfer_command(args: &[String]) -> Result<()> {
    if args.len() < 2 {
        print_module_usage();
        anyhow::bail!("module owner-transfer requires a module slug and new owner actor");
    }

    let slug = args[0].as_str();
    let new_owner_actor = args[1].trim();
    if new_owner_actor.is_empty() {
        anyhow::bail!("module owner-transfer requires a non-empty new owner actor");
    }

    let dry_run = args.iter().skip(2).any(|arg| arg == "--dry-run");
    let manifest_path = manifest_path();
    let manifest = load_manifest_from(&manifest_path)?;
    let workspace_manifest = load_workspace_manifest()?;
    let spec = manifest
        .modules
        .get(slug)
        .with_context(|| format!("Unknown module slug '{slug}'"))?;
    let preview = build_module_publish_preview(&manifest_path, slug, spec, &workspace_manifest)?;
    let reason = reason_argument(&args[2..]);
    let payload = ModuleOwnerTransferDryRunPreview {
        action: "owner_transfer".to_string(),
        slug: slug.to_string(),
        crate_name: preview.crate_name.clone(),
        current_local_version: preview.version.clone(),
        package_manifest_path: preview.package_manifest_path,
        new_owner_actor: new_owner_actor.to_string(),
        reason: reason.clone(),
    };
    let registry_url = registry_url_argument(&args[2..]);

    if dry_run {
        if let Some(registry_url) = registry_url {
            let remote_payload =
                owner_transfer_via_registry_dry_run(&registry_url, &payload, reason)?;
            println!("{remote_payload}");
        } else {
            println!("{}", serde_json::to_string_pretty(&payload)?);
        }
    } else {
        let registry_url = registry_url.with_context(|| {
            "Live module owner-transfer requires --registry-url or RUSTOK_MODULE_REGISTRY_URL"
        })?;
        let reason =
            reason.with_context(|| "Live module owner-transfer requires --reason <text>")?;
        let remote_payload = owner_transfer_via_registry_live(&registry_url, &payload, reason)?;
        println!("{remote_payload}");
    }

    Ok(())
}

fn module_stage_command(args: &[String]) -> Result<()> {
    if args.len() < 3 {
        print_module_usage();
        anyhow::bail!("module stage requires a request id, stage key, and status");
    }

    let request_id = args[0].trim();
    let stage = args[1].trim();
    let status = args[2].trim().to_ascii_lowercase();
    if request_id.is_empty() {
        anyhow::bail!("module stage requires a non-empty request id");
    }
    if stage.is_empty() {
        anyhow::bail!("module stage requires a non-empty stage key");
    }

    let allowed_statuses = ["queued", "running", "passed", "failed", "blocked"];
    if !allowed_statuses
        .iter()
        .any(|candidate| *candidate == status)
    {
        anyhow::bail!(
            "module stage status '{}' is not supported; expected one of {}",
            status,
            allowed_statuses.join(", ")
        );
    }

    let dry_run = args.iter().skip(3).any(|arg| arg == "--dry-run");
    let requeue = args.iter().skip(3).any(|arg| arg == "--requeue");
    if requeue && status != "queued" {
        anyhow::bail!("module stage --requeue requires status 'queued'");
    }

    let preview = ModuleValidationStageDryRunPreview {
        action: "validation_stage".to_string(),
        request_id: request_id.to_string(),
        stage: stage.to_string(),
        status,
        detail: detail_argument(&args[3..]),
        requeue,
    };
    let registry_url = registry_url_argument(&args[3..]);

    if dry_run {
        if let Some(registry_url) = registry_url {
            let payload = validation_stage_via_registry_dry_run(&registry_url, &preview)?;
            println!("{payload}");
        } else {
            println!("{}", serde_json::to_string_pretty(&preview)?);
        }
    } else {
        let registry_url = registry_url.with_context(|| {
            "Live module stage update requires --registry-url or RUSTOK_MODULE_REGISTRY_URL"
        })?;
        let payload = validation_stage_via_registry_live(&registry_url, &preview)?;
        println!("{payload}");
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

fn registry_url_argument(args: &[String]) -> Option<String> {
    if let Some(index) = args.iter().position(|arg| arg == "--registry-url") {
        return args
            .get(index + 1)
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty());
    }

    std::env::var("RUSTOK_MODULE_REGISTRY_URL")
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn auto_approve_argument(args: &[String]) -> bool {
    args.iter().any(|arg| arg == "--auto-approve")
}

fn reason_argument(args: &[String]) -> Option<String> {
    if let Some(index) = args.iter().position(|arg| arg == "--reason") {
        return args
            .get(index + 1)
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty());
    }

    None
}

fn detail_argument(args: &[String]) -> Option<String> {
    if let Some(index) = args.iter().position(|arg| arg == "--detail") {
        return args
            .get(index + 1)
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty());
    }

    None
}

fn publish_via_registry_dry_run(
    registry_url: &str,
    preview: &ModulePublishDryRunPreview,
) -> Result<String> {
    let endpoint = format!("{}/v2/catalog/publish", registry_url.trim_end_matches('/'));
    let request = build_publish_registry_request(preview);

    post_registry_json(&endpoint, &request)
}

fn publish_via_registry_live(
    registry_url: &str,
    preview: &ModulePublishDryRunPreview,
    auto_approve: bool,
) -> Result<String> {
    let publisher = format!("publisher:{}", preview.slug);
    let create_endpoint = format!("{}/v2/catalog/publish", registry_url.trim_end_matches('/'));
    let create_request = build_live_publish_registry_request(preview);
    let create_response: RegistryMutationHttpResponse = post_registry_json_parsed(
        &create_endpoint,
        &create_request,
        Some("xtask:module-publish"),
        Some(&publisher),
    )?;
    if !create_response.accepted {
        anyhow::bail!(
            "Registry publish request was not accepted: {}",
            join_registry_errors(&create_response.errors)
        );
    }

    let request_id = create_response
        .request_id
        .as_deref()
        .filter(|value| !value.trim().is_empty())
        .with_context(|| {
            format!(
                "Registry publish did not return request_id; status={:?}, next_step={:?}",
                create_response.status, create_response.next_step
            )
        })?;

    let artifact_bytes = build_publish_artifact_bytes(preview)?;
    let upload_endpoint = format!(
        "{}/v2/catalog/publish/{request_id}/artifact",
        registry_url.trim_end_matches('/')
    );
    let upload_response: RegistryMutationHttpResponse = put_registry_bytes_parsed(
        &upload_endpoint,
        &artifact_bytes,
        "application/json",
        Some("xtask:module-publish"),
        Some(&publisher),
    )?;
    ensure_publish_step_accepted(
        "artifact upload",
        upload_response.accepted,
        upload_response.status.as_deref(),
        &upload_response.errors,
    )?;

    let validate_endpoint = format!(
        "{}/v2/catalog/publish/{request_id}/validate",
        registry_url.trim_end_matches('/')
    );
    let validate_request = RegistryPublishValidationHttpRequest {
        schema_version: REGISTRY_MUTATION_SCHEMA_VERSION,
        dry_run: false,
    };
    let validate_response: RegistryMutationHttpResponse = post_registry_json_parsed(
        &validate_endpoint,
        &validate_request,
        Some("xtask:module-publish"),
        Some(&publisher),
    )?;
    ensure_publish_step_accepted(
        "validation",
        validate_response.accepted,
        validate_response.status.as_deref(),
        &validate_response.errors,
    )?;

    let readiness_endpoint = format!(
        "{}/v2/catalog/publish/{request_id}",
        registry_url.trim_end_matches('/')
    );
    let readiness = poll_registry_publish_status_until(
        &readiness_endpoint,
        Some("xtask:module-publish"),
        &["approved", "published", "rejected"],
    )?;
    ensure_publish_status_not_rejected(&readiness)?;
    if readiness.status == "published" {
        return pretty_json(&readiness);
    }
    if readiness.status != "approved" {
        anyhow::bail!(
            "Registry publish request '{}' did not reach approved status after validation; current status is '{}'",
            readiness.request_id,
            readiness.status
        );
    }
    if !auto_approve {
        return pretty_json(&readiness);
    }

    let approve_endpoint = format!(
        "{}/v2/catalog/publish/{request_id}/approve",
        registry_url.trim_end_matches('/')
    );
    let approve_request = RegistryPublishDecisionHttpRequest {
        schema_version: REGISTRY_MUTATION_SCHEMA_VERSION,
        dry_run: false,
        reason: None,
    };
    let approve_response: RegistryMutationHttpResponse = post_registry_json_parsed(
        &approve_endpoint,
        &approve_request,
        Some("xtask:module-publish"),
        Some(&publisher),
    )?;
    ensure_publish_step_accepted(
        "approval",
        approve_response.accepted,
        approve_response.status.as_deref(),
        &approve_response.errors,
    )?;

    let status_endpoint = format!(
        "{}/v2/catalog/publish/{request_id}",
        registry_url.trim_end_matches('/')
    );
    let status = poll_registry_publish_status_until(
        &status_endpoint,
        Some("xtask:module-publish"),
        &["published", "rejected"],
    )?;
    ensure_publish_status_not_rejected(&status)?;
    pretty_json(&status)
}

fn yank_via_registry_dry_run(
    registry_url: &str,
    preview: &ModuleYankDryRunPreview,
    reason: Option<String>,
) -> Result<String> {
    let endpoint = format!("{}/v2/catalog/yank", registry_url.trim_end_matches('/'));
    let request = build_yank_registry_request(preview, reason);

    post_registry_json(&endpoint, &request)
}

fn yank_via_registry_live(
    registry_url: &str,
    preview: &ModuleYankDryRunPreview,
    reason: String,
) -> Result<String> {
    let endpoint = format!("{}/v2/catalog/yank", registry_url.trim_end_matches('/'));
    let request = build_live_yank_registry_request(preview, Some(reason));
    let publisher = format!("publisher:{}", preview.slug);
    let response: RegistryMutationHttpResponse = post_registry_json_parsed(
        &endpoint,
        &request,
        Some("xtask:module-yank"),
        Some(&publisher),
    )?;
    if !response.accepted {
        anyhow::bail!(
            "Registry yank request was not accepted: {}",
            join_registry_errors(&response.errors)
        );
    }

    pretty_json(&response)
}

fn owner_transfer_via_registry_dry_run(
    registry_url: &str,
    preview: &ModuleOwnerTransferDryRunPreview,
    reason: Option<String>,
) -> Result<String> {
    let endpoint = format!(
        "{}/v2/catalog/owner-transfer",
        registry_url.trim_end_matches('/')
    );
    let request = build_owner_transfer_registry_request(preview, reason);

    post_registry_json(&endpoint, &request)
}

fn owner_transfer_via_registry_live(
    registry_url: &str,
    preview: &ModuleOwnerTransferDryRunPreview,
    reason: String,
) -> Result<String> {
    let endpoint = format!(
        "{}/v2/catalog/owner-transfer",
        registry_url.trim_end_matches('/')
    );
    let request = build_live_owner_transfer_registry_request(preview, Some(reason));
    let response: RegistryMutationHttpResponse = post_registry_json_parsed(
        &endpoint,
        &request,
        Some("xtask:module-owner-transfer"),
        None,
    )?;
    if !response.accepted {
        anyhow::bail!(
            "Registry owner transfer request was not accepted: {}",
            join_registry_errors(&response.errors)
        );
    }

    pretty_json(&response)
}

fn validation_stage_via_registry_dry_run(
    registry_url: &str,
    preview: &ModuleValidationStageDryRunPreview,
) -> Result<String> {
    let endpoint = format!(
        "{}/v2/catalog/publish/{}/stages",
        registry_url.trim_end_matches('/'),
        preview.request_id
    );
    let request = build_validation_stage_registry_request(preview);

    post_registry_json(&endpoint, &request)
}

fn validation_stage_via_registry_live(
    registry_url: &str,
    preview: &ModuleValidationStageDryRunPreview,
) -> Result<String> {
    let endpoint = format!(
        "{}/v2/catalog/publish/{}/stages",
        registry_url.trim_end_matches('/'),
        preview.request_id
    );
    let request = build_live_validation_stage_registry_request(preview);
    let response: RegistryMutationHttpResponse =
        post_registry_json_parsed(&endpoint, &request, Some("xtask:module-stage"), None)?;
    if !response.accepted {
        anyhow::bail!(
            "Registry validation stage update was not accepted: {}",
            join_registry_errors(&response.errors)
        );
    }

    pretty_json(&response)
}

fn build_publish_registry_request(
    preview: &ModulePublishDryRunPreview,
) -> RegistryPublishHttpRequest {
    build_publish_registry_request_with_dry_run(preview, true)
}

fn build_live_publish_registry_request(
    preview: &ModulePublishDryRunPreview,
) -> RegistryPublishHttpRequest {
    build_publish_registry_request_with_dry_run(preview, false)
}

fn build_publish_registry_request_with_dry_run(
    preview: &ModulePublishDryRunPreview,
    dry_run: bool,
) -> RegistryPublishHttpRequest {
    RegistryPublishHttpRequest {
        schema_version: REGISTRY_MUTATION_SCHEMA_VERSION,
        dry_run,
        module: RegistryPublishModuleHttpRequest {
            slug: preview.slug.clone(),
            version: preview.version.clone(),
            crate_name: preview.crate_name.clone(),
            name: preview.module_name.clone(),
            description: preview.module_description.clone(),
            ownership: preview.ownership.clone(),
            trust_level: preview.trust_level.clone(),
            license: preview.license.clone(),
            entry_type: preview.module_entry_type.clone(),
            marketplace: RegistryPublishMarketplaceHttpRequest {
                category: preview.marketplace.category.clone(),
                tags: preview.marketplace.tags.clone(),
            },
            ui_packages: RegistryPublishUiPackagesHttpRequest {
                admin: preview.ui_packages.admin.as_ref().map(|ui| {
                    RegistryPublishUiPackageHttpRequest {
                        crate_name: ui.crate_name.clone(),
                    }
                }),
                storefront: preview.ui_packages.storefront.as_ref().map(|ui| {
                    RegistryPublishUiPackageHttpRequest {
                        crate_name: ui.crate_name.clone(),
                    }
                }),
            },
        },
    }
}

fn build_yank_registry_request(
    preview: &ModuleYankDryRunPreview,
    reason: Option<String>,
) -> RegistryYankHttpRequest {
    build_yank_registry_request_with_dry_run(preview, reason, true)
}

fn build_live_yank_registry_request(
    preview: &ModuleYankDryRunPreview,
    reason: Option<String>,
) -> RegistryYankHttpRequest {
    build_yank_registry_request_with_dry_run(preview, reason, false)
}

fn build_yank_registry_request_with_dry_run(
    preview: &ModuleYankDryRunPreview,
    reason: Option<String>,
    dry_run: bool,
) -> RegistryYankHttpRequest {
    RegistryYankHttpRequest {
        schema_version: REGISTRY_MUTATION_SCHEMA_VERSION,
        dry_run,
        slug: preview.slug.clone(),
        version: preview.version.clone(),
        reason,
    }
}

fn build_owner_transfer_registry_request(
    preview: &ModuleOwnerTransferDryRunPreview,
    reason: Option<String>,
) -> RegistryOwnerTransferHttpRequest {
    build_owner_transfer_registry_request_with_dry_run(preview, reason, true)
}

fn build_live_owner_transfer_registry_request(
    preview: &ModuleOwnerTransferDryRunPreview,
    reason: Option<String>,
) -> RegistryOwnerTransferHttpRequest {
    build_owner_transfer_registry_request_with_dry_run(preview, reason, false)
}

fn build_owner_transfer_registry_request_with_dry_run(
    preview: &ModuleOwnerTransferDryRunPreview,
    reason: Option<String>,
    dry_run: bool,
) -> RegistryOwnerTransferHttpRequest {
    RegistryOwnerTransferHttpRequest {
        schema_version: REGISTRY_MUTATION_SCHEMA_VERSION,
        dry_run,
        slug: preview.slug.clone(),
        new_owner_actor: preview.new_owner_actor.clone(),
        reason,
    }
}

fn build_validation_stage_registry_request(
    preview: &ModuleValidationStageDryRunPreview,
) -> RegistryValidationStageHttpRequest {
    build_validation_stage_registry_request_with_dry_run(preview, true)
}

fn build_live_validation_stage_registry_request(
    preview: &ModuleValidationStageDryRunPreview,
) -> RegistryValidationStageHttpRequest {
    build_validation_stage_registry_request_with_dry_run(preview, false)
}

fn build_validation_stage_registry_request_with_dry_run(
    preview: &ModuleValidationStageDryRunPreview,
    dry_run: bool,
) -> RegistryValidationStageHttpRequest {
    RegistryValidationStageHttpRequest {
        schema_version: REGISTRY_MUTATION_SCHEMA_VERSION,
        dry_run,
        stage: preview.stage.clone(),
        status: preview.status.clone(),
        detail: preview.detail.clone(),
        requeue: preview.requeue,
    }
}

fn post_registry_json<T>(endpoint: &str, payload: &T) -> Result<String>
where
    T: Serialize,
{
    let value: serde_json::Value = post_registry_json_parsed(endpoint, payload, None, None)?;
    pretty_json(&value)
}

fn post_registry_json_parsed<T, U>(
    endpoint: &str,
    payload: &T,
    actor: Option<&str>,
    publisher: Option<&str>,
) -> Result<U>
where
    T: Serialize,
    U: DeserializeOwned,
{
    let client = build_registry_http_client(endpoint)?;
    let mut request = client.post(endpoint).json(payload);
    if let Some(actor) = actor {
        request = request.header("x-rustok-actor", actor);
    }
    if let Some(publisher) = publisher {
        request = request.header("x-rustok-publisher", publisher);
    }
    let response = request
        .send()
        .with_context(|| format!("Failed to call registry endpoint {endpoint}"))?;
    parse_registry_json_response(endpoint, response)
}

fn put_registry_bytes_parsed<U>(
    endpoint: &str,
    payload: &[u8],
    content_type: &str,
    actor: Option<&str>,
    publisher: Option<&str>,
) -> Result<U>
where
    U: DeserializeOwned,
{
    let client = build_registry_http_client(endpoint)?;
    let mut request = client
        .put(endpoint)
        .header("content-type", content_type)
        .body(payload.to_vec());
    if let Some(actor) = actor {
        request = request.header("x-rustok-actor", actor);
    }
    if let Some(publisher) = publisher {
        request = request.header("x-rustok-publisher", publisher);
    }
    let response = request
        .send()
        .with_context(|| format!("Failed to call registry upload endpoint {endpoint}"))?;
    parse_registry_json_response(endpoint, response)
}

fn get_registry_json_parsed<U>(endpoint: &str, actor: Option<&str>) -> Result<U>
where
    U: DeserializeOwned,
{
    let client = build_registry_http_client(endpoint)?;
    let mut request = client.get(endpoint);
    if let Some(actor) = actor {
        request = request.header("x-rustok-actor", actor);
    }
    let response = request
        .send()
        .with_context(|| format!("Failed to call registry endpoint {endpoint}"))?;
    parse_registry_json_response(endpoint, response)
}

fn parse_registry_json_response<U>(
    endpoint: &str,
    response: reqwest::blocking::Response,
) -> Result<U>
where
    U: DeserializeOwned,
{
    let response = response
        .error_for_status()
        .with_context(|| format!("Registry endpoint {endpoint} returned an error status"))?;
    response
        .json::<U>()
        .with_context(|| format!("Failed to parse registry response from {endpoint}"))
}

fn build_publish_artifact_bytes(preview: &ModulePublishDryRunPreview) -> Result<Vec<u8>> {
    let package_manifest_path = workspace_root().join(&preview.package_manifest_path);
    let package_manifest = fs::read_to_string(&package_manifest_path).with_context(|| {
        format!(
            "Failed to read publish package manifest {}",
            package_manifest_path.display()
        )
    })?;
    let module_root = package_manifest_path.parent().with_context(|| {
        format!(
            "Failed to resolve module root for {}",
            package_manifest_path.display()
        )
    })?;
    let crate_manifest_path = module_root.join("Cargo.toml");
    let crate_manifest = fs::read_to_string(&crate_manifest_path).with_context(|| {
        format!(
            "Failed to read crate manifest {}",
            crate_manifest_path.display()
        )
    })?;

    let admin_manifest = preview
        .ui_packages
        .admin
        .as_ref()
        .map(|ui| {
            let path = workspace_root().join(&ui.manifest_path);
            let content = fs::read_to_string(&path)
                .with_context(|| format!("Failed to read admin UI manifest {}", path.display()))?;
            Ok::<String, anyhow::Error>(content)
        })
        .transpose()?;
    let storefront_manifest = preview
        .ui_packages
        .storefront
        .as_ref()
        .map(|ui| {
            let path = workspace_root().join(&ui.manifest_path);
            let content = fs::read_to_string(&path).with_context(|| {
                format!("Failed to read storefront UI manifest {}", path.display())
            })?;
            Ok::<String, anyhow::Error>(content)
        })
        .transpose()?;

    let payload = serde_json::json!({
        "schema_version": REGISTRY_MUTATION_SCHEMA_VERSION,
        "artifact_type": "rustok-module-publish-bundle",
        "module": preview,
        "files": {
            "rustok-module.toml": package_manifest,
            "Cargo.toml": crate_manifest,
            "admin/Cargo.toml": admin_manifest,
            "storefront/Cargo.toml": storefront_manifest,
        }
    });

    serde_json::to_vec_pretty(&payload).context("Failed to serialize publish artifact bundle")
}

fn poll_registry_publish_status_until(
    endpoint: &str,
    actor: Option<&str>,
    desired_statuses: &[&str],
) -> Result<RegistryPublishStatusHttpResponse> {
    let mut last_status = None;

    for attempt in 0..10 {
        let status: RegistryPublishStatusHttpResponse = get_registry_json_parsed(endpoint, actor)?;
        let terminal = desired_statuses
            .iter()
            .any(|candidate| status.status == *candidate);
        last_status = Some(status);
        if terminal {
            break;
        }
        if attempt < 9 {
            std::thread::sleep(std::time::Duration::from_millis(200));
        }
    }

    last_status.with_context(|| format!("Registry status endpoint {endpoint} returned no payload"))
}

fn ensure_publish_step_accepted(
    step: &str,
    accepted: bool,
    status: Option<&str>,
    errors: &[String],
) -> Result<()> {
    if accepted && status != Some("rejected") {
        return Ok(());
    }

    let status_label = status.unwrap_or("unknown");
    anyhow::bail!(
        "Registry publish {step} finished with status '{status_label}': {}",
        join_registry_errors(errors)
    );
}

fn ensure_publish_status_not_rejected(status: &RegistryPublishStatusHttpResponse) -> Result<()> {
    if status.accepted && status.status != "rejected" {
        return Ok(());
    }

    anyhow::bail!(
        "Registry publish request '{}' was rejected: {}",
        status.request_id,
        join_registry_errors(&status.errors)
    );
}

fn join_registry_errors(errors: &[String]) -> String {
    if errors.is_empty() {
        "no error details returned".to_string()
    } else {
        errors.join("; ")
    }
}

fn pretty_json<T>(value: &T) -> Result<String>
where
    T: Serialize,
{
    serde_json::to_string_pretty(value).context("Failed to pretty-print registry payload")
}

fn build_registry_http_client(endpoint: &str) -> Result<Client> {
    let mut builder = Client::builder().timeout(std::time::Duration::from_secs(15));
    if registry_endpoint_uses_loopback(endpoint) {
        builder = builder.no_proxy();
    }

    builder
        .build()
        .context("Failed to build registry HTTP client")
}

fn registry_endpoint_uses_loopback(endpoint: &str) -> bool {
    Url::parse(endpoint)
        .ok()
        .and_then(|url| url.host_str().map(|host| host.to_string()))
        .is_some_and(|host| {
            host.eq_ignore_ascii_case("localhost")
                || host
                    .parse::<IpAddr>()
                    .map(|address| address.is_loopback())
                    .unwrap_or(false)
        })
}

fn build_module_test_plan(preview: &ModulePublishDryRunPreview) -> ModuleTestPlanPreview {
    let mut commands = vec![ModuleCommandPreview {
        label: "module validate".to_string(),
        argv: vec![
            "cargo".to_string(),
            "run".to_string(),
            "-p".to_string(),
            "xtask".to_string(),
            "--".to_string(),
            "module".to_string(),
            "validate".to_string(),
            preview.slug.clone(),
        ],
    }];

    commands.push(package_check_command("module crate", &preview.crate_name));

    if let Some(admin) = preview.ui_packages.admin.as_ref() {
        commands.push(package_check_command("admin ui crate", &admin.crate_name));
    }

    if let Some(storefront) = preview.ui_packages.storefront.as_ref() {
        commands.push(package_check_command(
            "storefront ui crate",
            &storefront.crate_name,
        ));
    }

    ModuleTestPlanPreview {
        slug: preview.slug.clone(),
        version: preview.version.clone(),
        commands,
    }
}

fn package_check_command(label: &str, package: &str) -> ModuleCommandPreview {
    ModuleCommandPreview {
        label: label.to_string(),
        argv: vec![
            "cargo".to_string(),
            "check".to_string(),
            "-p".to_string(),
            package.to_string(),
        ],
    }
}

fn run_command(command: &ModuleCommandPreview) -> Result<()> {
    let program = command
        .argv
        .first()
        .with_context(|| format!("Command '{}' has empty argv", command.label))?;
    let status = Command::new(program)
        .args(&command.argv[1..])
        .current_dir(workspace_root())
        .status()
        .with_context(|| format!("Failed to launch '{}'", command.argv.join(" ")))?;
    if !status.success() {
        anyhow::bail!(
            "Command '{}' failed with status {}",
            command.argv.join(" "),
            status
        );
    }
    Ok(())
}

fn build_module_publish_preview(
    manifest_path: &Path,
    slug: &str,
    spec: &ModuleSpec,
    workspace_manifest: &TomlValue,
) -> Result<ModulePublishDryRunPreview> {
    if spec.source != "path" {
        anyhow::bail!(
            "Module '{slug}' uses source='{}'; publish dry-run currently supports only local path modules",
            spec.source
        );
    }

    let package_manifest_path = module_package_manifest_path(manifest_path, spec)
        .with_context(|| format!("Module '{slug}' has source='path' but no path specified"))?;
    if !package_manifest_path.exists() {
        anyhow::bail!(
            "Module '{slug}' requires rustok-module.toml at {}",
            package_manifest_path.display()
        );
    }

    let package_manifest = load_module_package_manifest(&package_manifest_path)?;
    validate_module_package_metadata(slug, &package_manifest.module)?;
    validate_module_publish_contract(slug, &package_manifest)?;

    let declared_slug = package_manifest.module.slug.trim();
    if declared_slug != slug {
        anyhow::bail!(
            "Module '{slug}' declares slug '{}' in rustok-module.toml",
            declared_slug
        );
    }

    let module_root = package_manifest_path
        .parent()
        .map(PathBuf::from)
        .with_context(|| {
            format!(
                "Failed to resolve module root for '{}'",
                package_manifest_path.display()
            )
        })?;
    let crate_manifest_path = module_root.join("Cargo.toml");
    let crate_package = load_resolved_cargo_package(&crate_manifest_path, workspace_manifest)?;

    if crate_package.name != spec.crate_name {
        anyhow::bail!(
            "Module '{slug}' points to crate '{}' in modules.toml, but Cargo.toml declares '{}'",
            spec.crate_name,
            crate_package.name
        );
    }

    if crate_package.version != package_manifest.module.version {
        anyhow::bail!(
            "Module '{slug}' version mismatch: rustok-module.toml has '{}', Cargo.toml resolves to '{}'",
            package_manifest.module.version,
            crate_package.version
        );
    }

    let license = crate_package.license.clone().with_context(|| {
        format!(
            "Module '{slug}' must resolve package.license via {}",
            crate_package.manifest_path.display()
        )
    })?;

    validate_module_ui_surface_contract(
        slug,
        &module_root,
        "admin",
        package_manifest
            .provides
            .admin_ui
            .as_ref()
            .and_then(|ui| ui.leptos_crate.as_deref()),
    )?;
    validate_module_ui_surface_contract(
        slug,
        &module_root,
        "storefront",
        package_manifest
            .provides
            .storefront_ui
            .as_ref()
            .and_then(|ui| ui.leptos_crate.as_deref()),
    )?;

    let admin_preview = validate_module_ui_package(
        slug,
        &module_root,
        "admin",
        package_manifest
            .provides
            .admin_ui
            .as_ref()
            .and_then(|ui| ui.leptos_crate.as_deref()),
        &package_manifest.module.version,
        workspace_manifest,
    )?;
    let storefront_preview = validate_module_ui_package(
        slug,
        &module_root,
        "storefront",
        package_manifest
            .provides
            .storefront_ui
            .as_ref()
            .and_then(|ui| ui.leptos_crate.as_deref()),
        &package_manifest.module.version,
        workspace_manifest,
    )?;

    Ok(ModulePublishDryRunPreview {
        slug: slug.to_string(),
        version: package_manifest.module.version.clone(),
        crate_name: crate_package.name,
        module_name: package_manifest.module.name.clone(),
        module_description: package_manifest.module.description.clone(),
        ownership: package_manifest.module.ownership.clone(),
        trust_level: package_manifest.module.trust_level.clone(),
        license,
        manifest_path: manifest_path.display().to_string(),
        package_manifest_path: package_manifest_path.display().to_string(),
        module_entry_type: package_manifest.crate_contract.entry_type.clone(),
        marketplace: ModuleMarketplacePreview {
            category: package_manifest.marketplace.category.clone(),
            tags: package_manifest.marketplace.tags.clone(),
        },
        ui_packages: ModuleUiPackagesPreview {
            admin: admin_preview,
            storefront: storefront_preview,
        },
    })
}

fn validate_module_publish_contract(slug: &str, manifest: &ModulePackageManifest) -> Result<()> {
    let module_slug = manifest.module.slug.trim();
    if module_slug.is_empty() {
        anyhow::bail!("Module '{slug}' is missing module.slug in rustok-module.toml");
    }

    if manifest.module.name.trim().is_empty() {
        anyhow::bail!("Module '{slug}' is missing module.name in rustok-module.toml");
    }

    let version = manifest.module.version.trim();
    if version.is_empty() {
        anyhow::bail!("Module '{slug}' is missing module.version in rustok-module.toml");
    }
    Version::parse(version)
        .with_context(|| format!("Module '{slug}' has non-semver module.version '{version}'"))?;

    if manifest.module.description.trim().len() < 20 {
        anyhow::bail!(
            "Module '{slug}' description must be at least 20 characters for publish readiness"
        );
    }

    Ok(())
}

fn validate_module_ui_surface_contract(
    slug: &str,
    module_root: &Path,
    surface: &str,
    crate_name: Option<&str>,
) -> Result<()> {
    let manifest_path = module_root.join(surface).join("Cargo.toml");
    let has_subcrate = manifest_path.exists();
    let crate_name = crate_name.map(str::trim).filter(|value| !value.is_empty());

    if has_subcrate && crate_name.is_none() {
        anyhow::bail!(
            "Module '{slug}' contains {}, but rustok-module.toml is missing [provides.{surface}_ui].leptos_crate",
            manifest_path.display()
        );
    }

    if !has_subcrate && crate_name.is_some() {
        anyhow::bail!(
            "Module '{slug}' declares [provides.{surface}_ui].leptos_crate, but {} is missing",
            manifest_path.display()
        );
    }

    Ok(())
}

fn validate_module_ui_package(
    slug: &str,
    module_root: &Path,
    surface: &str,
    crate_name: Option<&str>,
    expected_version: &str,
    workspace_manifest: &TomlValue,
) -> Result<Option<ModuleUiPackagePreview>> {
    let Some(crate_name) = crate_name else {
        return Ok(None);
    };

    let manifest_path = module_root.join(surface).join("Cargo.toml");
    if !manifest_path.exists() {
        anyhow::bail!(
            "Module '{slug}' declares provides.{surface}_ui.leptos_crate='{crate_name}', but {} is missing",
            manifest_path.display()
        );
    }

    let package = load_resolved_cargo_package(&manifest_path, workspace_manifest)?;
    if package.name != crate_name {
        anyhow::bail!(
            "Module '{slug}' declares provides.{surface}_ui.leptos_crate='{crate_name}', but {} declares '{}'",
            manifest_path.display(),
            package.name
        );
    }
    if package.version != expected_version {
        anyhow::bail!(
            "Module '{slug}' {surface} package version mismatch: expected '{expected_version}', got '{}'",
            package.version
        );
    }

    Ok(Some(ModuleUiPackagePreview {
        crate_name: package.name,
        manifest_path: manifest_path.display().to_string(),
    }))
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

#[cfg(test)]
mod tests {
    use super::{
        auto_approve_argument, build_module_test_plan, build_owner_transfer_registry_request,
        build_publish_registry_request, build_validation_stage_registry_request,
        build_yank_registry_request, registry_endpoint_uses_loopback,
        resolve_workspace_inherited_string, validate_module_publish_contract,
        validate_module_ui_surface_contract, ModuleMarketplacePreview,
        ModuleOwnerTransferDryRunPreview, ModulePackageManifest, ModulePublishDryRunPreview,
        ModuleUiPackagePreview, ModuleUiPackagesPreview, ModuleValidationStageDryRunPreview,
        ModuleYankDryRunPreview, REGISTRY_MUTATION_SCHEMA_VERSION,
    };
    use std::path::Path;

    #[test]
    fn resolve_workspace_inherited_string_uses_workspace_package_value() {
        let package_manifest: toml::Value = toml::from_str(
            r#"
                [package]
                version.workspace = true
            "#,
        )
        .expect("package manifest should parse");
        let workspace_manifest: toml::Value = toml::from_str(
            r#"
                [workspace.package]
                version = "1.2.3"
            "#,
        )
        .expect("workspace manifest should parse");

        let resolved = resolve_workspace_inherited_string(
            package_manifest
                .get("package")
                .and_then(toml::Value::as_table)
                .and_then(|table| table.get("version")),
            &workspace_manifest,
            "version",
        )
        .expect("version should resolve");

        assert_eq!(resolved.as_deref(), Some("1.2.3"));
    }

    #[test]
    fn validate_module_publish_contract_rejects_short_description() {
        let manifest: ModulePackageManifest = toml::from_str(
            r#"
                [module]
                slug = "blog"
                name = "Blog"
                version = "1.2.3"
                description = "Too short"
                ownership = "first_party"
                trust_level = "verified"
            "#,
        )
        .expect("module manifest should parse");

        let error = validate_module_publish_contract("blog", &manifest)
            .expect_err("short descriptions must fail");
        assert!(error.to_string().contains("at least 20 characters"));
    }

    #[test]
    fn validate_module_ui_surface_contract_rejects_declared_missing_subcrate() {
        let error = validate_module_ui_surface_contract(
            "blog",
            Path::new("crates/rustok-blog"),
            "preview",
            Some("rustok-blog-preview"),
        )
        .expect_err("declared surface without subcrate must fail");

        assert!(error
            .to_string()
            .contains("declares [provides.preview_ui].leptos_crate"));
    }

    #[test]
    fn validate_module_ui_surface_contract_accepts_wired_existing_admin_subcrate() {
        validate_module_ui_surface_contract(
            "blog",
            &super::workspace_root().join("crates").join("rustok-blog"),
            "admin",
            Some("rustok-blog-admin"),
        )
        .expect("wired admin subcrate should validate");
    }

    #[test]
    fn build_module_test_plan_includes_main_and_ui_crates() {
        let preview = ModulePublishDryRunPreview {
            slug: "blog".to_string(),
            version: "1.2.3".to_string(),
            crate_name: "rustok-blog".to_string(),
            module_name: "Blog".to_string(),
            module_description: "A blog module description long enough.".to_string(),
            ownership: "first_party".to_string(),
            trust_level: "verified".to_string(),
            license: "MIT".to_string(),
            manifest_path: "modules.toml".to_string(),
            package_manifest_path: "crates/rustok-blog/rustok-module.toml".to_string(),
            module_entry_type: Some("BlogModule".to_string()),
            marketplace: ModuleMarketplacePreview {
                category: Some("content".to_string()),
                tags: vec!["content".to_string()],
            },
            ui_packages: ModuleUiPackagesPreview {
                admin: Some(ModuleUiPackagePreview {
                    crate_name: "rustok-blog-admin".to_string(),
                    manifest_path: "crates/rustok-blog/admin/Cargo.toml".to_string(),
                }),
                storefront: Some(ModuleUiPackagePreview {
                    crate_name: "rustok-blog-storefront".to_string(),
                    manifest_path: "crates/rustok-blog/storefront/Cargo.toml".to_string(),
                }),
            },
        };

        let plan = build_module_test_plan(&preview);
        let commands = plan
            .commands
            .iter()
            .map(|command| command.argv.join(" "))
            .collect::<Vec<_>>();

        assert_eq!(commands.len(), 4);
        assert!(commands[0].contains("module validate blog"));
        assert!(commands[1].contains("cargo check -p rustok-blog"));
        assert!(commands[2].contains("cargo check -p rustok-blog-admin"));
        assert!(commands[3].contains("cargo check -p rustok-blog-storefront"));
    }

    #[test]
    fn build_publish_registry_request_serializes_v2_contract() {
        let preview = ModulePublishDryRunPreview {
            slug: "blog".to_string(),
            version: "1.2.3".to_string(),
            crate_name: "rustok-blog".to_string(),
            module_name: "Blog".to_string(),
            module_description: "A blog module description long enough.".to_string(),
            ownership: "first_party".to_string(),
            trust_level: "verified".to_string(),
            license: "MIT".to_string(),
            manifest_path: "modules.toml".to_string(),
            package_manifest_path: "crates/rustok-blog/rustok-module.toml".to_string(),
            module_entry_type: Some("BlogModule".to_string()),
            marketplace: ModuleMarketplacePreview {
                category: Some("content".to_string()),
                tags: vec!["content".to_string(), "editorial".to_string()],
            },
            ui_packages: ModuleUiPackagesPreview {
                admin: Some(ModuleUiPackagePreview {
                    crate_name: "rustok-blog-admin".to_string(),
                    manifest_path: "crates/rustok-blog/admin/Cargo.toml".to_string(),
                }),
                storefront: Some(ModuleUiPackagePreview {
                    crate_name: "rustok-blog-storefront".to_string(),
                    manifest_path: "crates/rustok-blog/storefront/Cargo.toml".to_string(),
                }),
            },
        };

        let request_body = serde_json::to_value(build_publish_registry_request(&preview))
            .expect("request should serialize");
        assert_eq!(
            request_body["schema_version"],
            REGISTRY_MUTATION_SCHEMA_VERSION
        );
        assert_eq!(request_body["dry_run"], true);
        assert_eq!(request_body["module"]["slug"], "blog");
        assert_eq!(request_body["module"]["version"], "1.2.3");
        assert_eq!(request_body["module"]["crate_name"], "rustok-blog");
        assert_eq!(request_body["module"]["name"], "Blog");
        assert_eq!(
            request_body["module"]["description"],
            "A blog module description long enough."
        );
        assert_eq!(request_body["module"]["ownership"], "first_party");
        assert_eq!(request_body["module"]["trust_level"], "verified");
        assert_eq!(request_body["module"]["license"], "MIT");
        assert_eq!(request_body["module"]["entry_type"], "BlogModule");
        assert_eq!(request_body["module"]["marketplace"]["category"], "content");
        assert_eq!(
            request_body["module"]["marketplace"]["tags"],
            serde_json::json!(["content", "editorial"])
        );
        assert_eq!(
            request_body["module"]["ui_packages"]["admin"]["crate_name"],
            "rustok-blog-admin"
        );
        assert_eq!(
            request_body["module"]["ui_packages"]["storefront"]["crate_name"],
            "rustok-blog-storefront"
        );
    }

    #[test]
    fn build_validation_stage_registry_request_serializes_stage_contract() {
        let preview = ModuleValidationStageDryRunPreview {
            action: "validation_stage".to_string(),
            request_id: "rpr_test".to_string(),
            stage: "compile_smoke".to_string(),
            status: "passed".to_string(),
            detail: Some("External CI recorded the result.".to_string()),
            requeue: false,
        };

        let request_body = serde_json::to_value(build_validation_stage_registry_request(&preview))
            .expect("stage request should serialize");

        assert_eq!(
            request_body
                .get("schema_version")
                .and_then(serde_json::Value::as_u64),
            Some(REGISTRY_MUTATION_SCHEMA_VERSION as u64)
        );
        assert_eq!(
            request_body
                .get("dry_run")
                .and_then(serde_json::Value::as_bool),
            Some(true)
        );
        assert_eq!(
            request_body
                .get("stage")
                .and_then(serde_json::Value::as_str),
            Some("compile_smoke")
        );
        assert_eq!(
            request_body
                .get("status")
                .and_then(serde_json::Value::as_str),
            Some("passed")
        );
        assert_eq!(
            request_body
                .get("requeue")
                .and_then(serde_json::Value::as_bool),
            Some(false)
        );
    }

    #[test]
    fn build_yank_registry_request_serializes_v2_contract() {
        let preview = ModuleYankDryRunPreview {
            action: "yank".to_string(),
            slug: "blog".to_string(),
            version: "1.2.3".to_string(),
            crate_name: "rustok-blog".to_string(),
            current_local_version: "1.2.3".to_string(),
            matches_local_version: true,
            package_manifest_path: "crates/rustok-blog/rustok-module.toml".to_string(),
        };

        let request_body = serde_json::to_value(build_yank_registry_request(
            &preview,
            Some("Accidental publish".to_string()),
        ))
        .expect("request should serialize");
        assert_eq!(
            request_body["schema_version"],
            REGISTRY_MUTATION_SCHEMA_VERSION
        );
        assert_eq!(request_body["dry_run"], true);
        assert_eq!(request_body["slug"], "blog");
        assert_eq!(request_body["version"], "1.2.3");
        assert_eq!(request_body["reason"], "Accidental publish");
    }

    #[test]
    fn build_owner_transfer_registry_request_serializes_v2_contract() {
        let preview = ModuleOwnerTransferDryRunPreview {
            action: "owner_transfer".to_string(),
            slug: "blog".to_string(),
            crate_name: "rustok-blog".to_string(),
            current_local_version: "1.2.3".to_string(),
            package_manifest_path: "crates/rustok-blog/rustok-module.toml".to_string(),
            new_owner_actor: "publisher:forum".to_string(),
            reason: Some("Ownership transferred to the forum publisher".to_string()),
        };

        let request_body = serde_json::to_value(build_owner_transfer_registry_request(
            &preview,
            preview.reason.clone(),
        ))
        .expect("request should serialize");
        assert_eq!(
            request_body["schema_version"],
            REGISTRY_MUTATION_SCHEMA_VERSION
        );
        assert_eq!(request_body["dry_run"], true);
        assert_eq!(request_body["slug"], "blog");
        assert_eq!(request_body["new_owner_actor"], "publisher:forum");
        assert_eq!(
            request_body["reason"],
            "Ownership transferred to the forum publisher"
        );
    }

    #[test]
    fn registry_endpoint_uses_loopback_for_local_urls() {
        assert!(registry_endpoint_uses_loopback(
            "http://127.0.0.1:5150/v2/catalog/publish"
        ));
        assert!(registry_endpoint_uses_loopback(
            "http://localhost:5150/v2/catalog/yank"
        ));
        assert!(!registry_endpoint_uses_loopback(
            "https://modules.rustok.dev/v2/catalog/publish"
        ));
    }

    #[test]
    fn auto_approve_argument_detects_flag() {
        assert!(auto_approve_argument(&[
            "--registry-url".to_string(),
            "http://127.0.0.1:5150".to_string(),
            "--auto-approve".to_string(),
        ]));
        assert!(!auto_approve_argument(&[
            "--registry-url".to_string(),
            "http://127.0.0.1:5150".to_string(),
        ]));
    }
}
