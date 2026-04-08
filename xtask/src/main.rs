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
    #[serde(default)]
    dependencies: HashMap<String, ModulePackageDependencyConstraint>,
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
    ui_classification: String,
    #[serde(default)]
    recommended_admin_surfaces: Vec<String>,
    #[serde(default)]
    showcase_admin_surfaces: Vec<String>,
}

#[derive(Debug, Deserialize, Default)]
struct ModulePackageDependencyConstraint {
    #[serde(default)]
    version_req: String,
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
    reason_code: Option<String>,
}

#[derive(Debug, Serialize)]
struct RegistryOwnerTransferHttpRequest {
    schema_version: u32,
    dry_run: bool,
    slug: String,
    new_owner_actor: String,
    reason: Option<String>,
    reason_code: Option<String>,
}

#[derive(Debug, Serialize)]
struct RegistryValidationStageHttpRequest {
    schema_version: u32,
    dry_run: bool,
    stage: String,
    status: String,
    detail: Option<String>,
    reason_code: Option<String>,
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
    #[serde(default, rename = "moderationPolicy")]
    moderation_policy: Option<RegistryModerationPolicyHttpResponse>,
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
    #[serde(default, rename = "followUpGates")]
    follow_up_gates: Vec<RegistryPublishStatusFollowUpGate>,
    #[serde(default, rename = "validationStages")]
    validation_stages: Vec<RegistryPublishStatusValidationStage>,
    #[serde(default, rename = "approvalOverrideRequired")]
    approval_override_required: bool,
    #[serde(default, rename = "approvalOverrideReasonCodes")]
    approval_override_reason_codes: Vec<String>,
    #[serde(default, rename = "governanceActions")]
    governance_actions: Vec<RegistryGovernanceActionHttpResponse>,
    next_step: Option<String>,
    #[serde(default, rename = "moderationPolicy")]
    moderation_policy: Option<RegistryModerationPolicyHttpResponse>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct RegistryGovernanceActionHttpResponse {
    key: String,
    enabled: bool,
    #[serde(default)]
    reason: Option<String>,
    #[serde(default, rename = "supportedReasonCodes")]
    supported_reason_codes: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct RegistryModerationPolicyHttpResponse {
    mode: String,
    #[serde(default, rename = "livePublishSupported")]
    live_publish_supported: bool,
    #[serde(default, rename = "liveGovernanceSupported")]
    live_governance_supported: bool,
    #[serde(default, rename = "manualReviewRequired")]
    manual_review_required: bool,
    #[serde(rename = "restrictionReasonCode")]
    restriction_reason_code: Option<String>,
    #[serde(default, rename = "restrictionReason")]
    restriction_reason: String,
}

#[derive(Debug, Deserialize, Serialize)]
struct RegistryPublishStatusFollowUpGate {
    key: String,
    status: String,
    detail: String,
    #[serde(rename = "updatedAt")]
    updated_at: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct RegistryPublishStatusValidationStage {
    key: String,
    status: String,
    detail: String,
    #[serde(rename = "attemptNumber")]
    attempt_number: i32,
    #[serde(rename = "updatedAt")]
    updated_at: String,
    #[serde(rename = "startedAt")]
    started_at: Option<String>,
    #[serde(rename = "finishedAt")]
    finished_at: Option<String>,
    #[serde(default, rename = "executionMode")]
    execution_mode: String,
    #[serde(default)]
    runnable: bool,
    #[serde(default, rename = "requiresManualConfirmation")]
    requires_manual_confirmation: bool,
    #[serde(default, rename = "allowedTerminalReasonCodes")]
    allowed_terminal_reason_codes: Vec<String>,
    #[serde(default, rename = "suggestedPassReasonCode")]
    suggested_pass_reason_code: Option<String>,
    #[serde(default, rename = "suggestedFailureReasonCode")]
    suggested_failure_reason_code: Option<String>,
    #[serde(default, rename = "suggestedBlockedReasonCode")]
    suggested_blocked_reason_code: Option<String>,
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
    reason_code: Option<String>,
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
    reason_code: Option<String>,
}

#[derive(Debug, Serialize)]
struct ModuleValidationStageDryRunPreview {
    action: String,
    request_id: String,
    stage: String,
    status: String,
    detail: Option<String>,
    reason_code: Option<String>,
    requeue: bool,
}

#[derive(Debug, Serialize)]
struct ModuleRegistryDecisionDryRunPreview {
    action: String,
    request_id: String,
    reason: Option<String>,
    reason_code: Option<String>,
}

#[derive(Debug, Serialize)]
struct ModuleValidationStageRunPreview {
    action: String,
    slug: String,
    request_id: String,
    stage: String,
    requires_manual_confirmation: bool,
    running_detail: String,
    success_detail: String,
    success_reason_code: String,
    failure_detail_prefix: String,
    failure_reason_code: String,
    commands: Vec<ModuleCommandPreview>,
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
const REGISTRY_OWNER_TRANSFER_REASON_CODES: &[&str] = &[
    "maintenance_handoff",
    "team_restructure",
    "publisher_rotation",
    "security_emergency",
    "governance_override",
    "other",
];
const REGISTRY_APPROVE_OVERRIDE_REASON_CODES: &[&str] = &[
    "manual_review_complete",
    "trusted_first_party",
    "expedited_release",
    "governance_override",
    "other",
];
const REGISTRY_REQUEST_CHANGES_REASON_CODES: &[&str] = &[
    "artifact_mismatch",
    "quality_gap",
    "policy_gap",
    "docs_gap",
    "other",
];
const REGISTRY_HOLD_REASON_CODES: &[&str] = &[
    "release_window",
    "incident",
    "legal_hold",
    "security_review",
    "other",
];
const REGISTRY_RESUME_REASON_CODES: &[&str] = &[
    "review_complete",
    "incident_closed",
    "legal_cleared",
    "other",
];
const REGISTRY_VALIDATION_STAGE_REASON_CODES: &[&str] = &[
    "local_runner_passed",
    "manual_review_complete",
    "build_failure",
    "test_failure",
    "policy_preflight_failed",
    "security_findings",
    "policy_exception",
    "license_issue",
    "manual_override",
    "other",
];
const REGISTRY_YANK_REASON_CODES: &[&str] = &[
    "security",
    "legal",
    "malware",
    "critical_regression",
    "rollback",
    "other",
];

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
    println!("  module stage-run    Execute a local follow-up validation stage and report it");
    println!("  module publish      Create/preview a publish request and stop at review-ready unless --auto-approve is set");
    println!("  module stage        Record or requeue a follow-up validation stage");
    println!(
        "  module request-changes Request a new artifact revision for an approved publish request"
    );
    println!("  module hold         Put a publish request on hold");
    println!("  module resume       Resume a held publish request");
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
        "stage-run" => module_stage_run_command(&args[1..]),
        "publish" => module_publish_command(&args[1..]),
        "stage" => module_stage_command(&args[1..]),
        "request-changes" => module_request_changes_command(&args[1..]),
        "hold" => module_hold_command(&args[1..]),
        "resume" => module_resume_command(&args[1..]),
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
        "  cargo xtask module stage-run <slug> <request-id> <stage> [--dry-run] [--registry-url <url>] [--detail <text>] [--confirm-manual-review]"
    );
    println!(
        "  cargo xtask module publish <slug> [--dry-run] [--registry-url <url>] [--auto-approve] [--approve-reason <text>] [--approve-reason-code <code>] [--confirm-manual-review]"
    );
    println!(
        "  cargo xtask module stage <request-id> <stage> <status> [--dry-run] [--detail <text>] [--reason-code <code>] [--requeue] [--registry-url <url>]"
    );
    println!(
        "  cargo xtask module request-changes <request-id> [--dry-run] [--reason <text>] [--reason-code <code>] [--registry-url <url>]"
    );
    println!(
        "  cargo xtask module hold <request-id> [--dry-run] [--reason <text>] [--reason-code <code>] [--registry-url <url>]"
    );
    println!(
        "  cargo xtask module resume <request-id> [--dry-run] [--reason <text>] [--reason-code <code>] [--registry-url <url>]"
    );
    println!(
        "  cargo xtask module owner-transfer <slug> <new-owner-actor> [--dry-run] [--reason <text>] [--reason-code <code>] [--registry-url <url>]"
    );
    println!(
        "  cargo xtask module yank <slug> <version> [--dry-run] [--reason <text>] [--reason-code <code>] [--registry-url <url>]"
    );
}

fn module_validate_command(args: &[String]) -> Result<()> {
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

fn module_publish_command(args: &[String]) -> Result<()> {
    if args.is_empty() {
        print_module_usage();
        anyhow::bail!("module publish requires a module slug");
    }

    let slug = args[0].as_str();
    let dry_run = args.iter().skip(1).any(|arg| arg == "--dry-run");
    let auto_approve = auto_approve_argument(&args[1..]);
    let approve_reason = approve_reason_argument(&args[1..]);
    let approve_reason_code = approve_reason_code_argument(&args[1..])?;
    let confirm_manual_review = manual_review_confirmation_argument(&args[1..]);

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
        let payload = publish_via_registry_live(
            &registry_url,
            &preview,
            auto_approve,
            approve_reason,
            approve_reason_code,
            confirm_manual_review,
        )?;
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

fn module_stage_run_command(args: &[String]) -> Result<()> {
    if args.len() < 3 {
        print_module_usage();
        anyhow::bail!("module stage-run requires a module slug, request id, and stage key");
    }

    let slug = args[0].trim();
    let request_id = args[1].trim();
    let stage = args[2].trim().to_ascii_lowercase();
    if slug.is_empty() {
        anyhow::bail!("module stage-run requires a non-empty module slug");
    }
    if request_id.is_empty() {
        anyhow::bail!("module stage-run requires a non-empty request id");
    }

    let dry_run = args.iter().skip(3).any(|arg| arg == "--dry-run");
    let manifest_path = manifest_path();
    let manifest = load_manifest_from(&manifest_path)?;
    let workspace_manifest = load_workspace_manifest()?;
    let spec = manifest
        .modules
        .get(slug)
        .with_context(|| format!("Unknown module slug '{slug}'"))?;
    let preview = build_module_publish_preview(&manifest_path, slug, spec, &workspace_manifest)?;
    let confirm_manual_review = manual_review_confirmation_argument(&args[3..]);
    let registry_url = registry_url_argument(&args[3..]);
    let live_stage_metadata = if dry_run {
        None
    } else {
        registry_url
            .as_deref()
            .map(|registry_url| {
                fetch_registry_validation_stage_metadata(registry_url, request_id, &stage)
            })
            .transpose()?
    };
    let plan = build_module_validation_stage_run_preview(
        &preview,
        request_id,
        &stage,
        detail_argument(&args[3..]),
        live_stage_metadata.as_ref(),
    )?;

    if dry_run {
        println!("{}", serde_json::to_string_pretty(&plan)?);
        return Ok(());
    }

    let registry_url = registry_url.with_context(|| {
        "Live module stage-run requires --registry-url or RUSTOK_MODULE_REGISTRY_URL"
    })?;
    if plan.requires_manual_confirmation && !confirm_manual_review {
        anyhow::bail!(
            "module stage-run '{}' requires --confirm-manual-review before persisting a passed security/policy review",
            plan.stage
        );
    }
    run_validation_stage_plan_via_registry(&registry_url, &plan)?;
    println!("{}", serde_json::to_string_pretty(&plan)?);
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
    let reason_code = reason_code_argument(&args[2..])?;
    if dry_run {
        if let Some(registry_url) = registry_url {
            let remote_payload =
                yank_via_registry_dry_run(&registry_url, &payload, reason, reason_code)?;
            println!("{remote_payload}");
        } else {
            println!("{}", serde_json::to_string_pretty(&payload)?);
        }
    } else {
        let registry_url = registry_url.with_context(|| {
            "Live module yank requires --registry-url or RUSTOK_MODULE_REGISTRY_URL"
        })?;
        let reason = reason.with_context(|| "Live module yank requires --reason <text>")?;
        let reason_code = reason_code.with_context(|| {
            format!(
                "Live module yank requires --reason-code <{}>",
                REGISTRY_YANK_REASON_CODES.join("|")
            )
        })?;
        let remote_payload = yank_via_registry_live(&registry_url, &payload, reason, reason_code)?;
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
    let reason_code = owner_transfer_reason_code_argument(&args[2..])?;
    let payload = ModuleOwnerTransferDryRunPreview {
        action: "owner_transfer".to_string(),
        slug: slug.to_string(),
        crate_name: preview.crate_name.clone(),
        current_local_version: preview.version.clone(),
        package_manifest_path: preview.package_manifest_path,
        new_owner_actor: new_owner_actor.to_string(),
        reason: reason.clone(),
        reason_code: reason_code.clone(),
    };
    let registry_url = registry_url_argument(&args[2..]);

    if dry_run {
        if let Some(registry_url) = registry_url {
            let remote_payload =
                owner_transfer_via_registry_dry_run(&registry_url, &payload, reason, reason_code)?;
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
        let reason_code = reason_code.with_context(|| {
            format!(
                "Live module owner-transfer requires --reason-code <{}>",
                REGISTRY_OWNER_TRANSFER_REASON_CODES.join("|")
            )
        })?;
        let remote_payload =
            owner_transfer_via_registry_live(&registry_url, &payload, reason, reason_code)?;
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
    let mut reason_code = validation_stage_reason_code_argument(&args[3..])?;
    if requeue && status != "queued" {
        anyhow::bail!("module stage --requeue requires status 'queued'");
    }

    let registry_url = registry_url_argument(&args[3..]);
    if !dry_run && validation_stage_status_requires_reason_code(&status) && reason_code.is_none() {
        let registry_url = registry_url.as_deref().with_context(|| {
            "Live module stage update requires --registry-url or RUSTOK_MODULE_REGISTRY_URL"
        })?;
        reason_code = fetch_registry_validation_stage_metadata(registry_url, request_id, stage)
            .ok()
            .and_then(|stage| suggested_reason_code_for_stage_status(&stage, &status));
    }

    let preview = ModuleValidationStageDryRunPreview {
        action: "validation_stage".to_string(),
        request_id: request_id.to_string(),
        stage: stage.to_string(),
        status,
        detail: detail_argument(&args[3..]),
        reason_code,
        requeue,
    };

    if dry_run {
        if let Some(registry_url) = registry_url {
            let payload = validation_stage_via_registry_dry_run(&registry_url, &preview)?;
            println!("{payload}");
        } else {
            println!("{}", serde_json::to_string_pretty(&preview)?);
        }
    } else {
        if validation_stage_status_requires_reason_code(&preview.status)
            && preview.reason_code.is_none()
        {
            anyhow::bail!(
                "Live module stage status '{}' requires --reason-code <{}>",
                preview.status,
                REGISTRY_VALIDATION_STAGE_REASON_CODES.join("|")
            );
        }
        let registry_url = registry_url.with_context(|| {
            "Live module stage update requires --registry-url or RUSTOK_MODULE_REGISTRY_URL"
        })?;
        let payload = validation_stage_via_registry_live(&registry_url, &preview)?;
        println!("{payload}");
    }

    Ok(())
}

fn module_request_changes_command(args: &[String]) -> Result<()> {
    module_publish_decision_command(
        args,
        "request_changes",
        "request-changes",
        REGISTRY_REQUEST_CHANGES_REASON_CODES,
        "Registry request-changes is not enabled for this request.",
    )
}

fn module_hold_command(args: &[String]) -> Result<()> {
    module_publish_decision_command(
        args,
        "hold",
        "hold",
        REGISTRY_HOLD_REASON_CODES,
        "Registry hold is not enabled for this request.",
    )
}

fn module_resume_command(args: &[String]) -> Result<()> {
    module_publish_decision_command(
        args,
        "resume",
        "resume",
        REGISTRY_RESUME_REASON_CODES,
        "Registry resume is not enabled for this request.",
    )
}

fn module_publish_decision_command(
    args: &[String],
    action_key: &str,
    cli_label: &str,
    supported_reason_codes: &[&str],
    disabled_message: &str,
) -> Result<()> {
    if args.is_empty() {
        print_module_usage();
        anyhow::bail!("module {cli_label} requires a request id");
    }

    let request_id = args[0].trim();
    if request_id.is_empty() {
        anyhow::bail!("module {cli_label} requires a non-empty request id");
    }

    let dry_run = args.iter().skip(1).any(|arg| arg == "--dry-run");
    let reason = reason_argument(&args[1..]);
    let reason_code = decision_reason_code_argument(&args[1..], cli_label, supported_reason_codes)?;
    let preview = ModuleRegistryDecisionDryRunPreview {
        action: action_key.to_string(),
        request_id: request_id.to_string(),
        reason: reason.clone(),
        reason_code: reason_code.clone(),
    };
    let registry_url = registry_url_argument(&args[1..]);

    if dry_run {
        if let Some(registry_url) = registry_url {
            let payload = publish_decision_via_registry_dry_run(
                &registry_url,
                request_id,
                action_key,
                reason,
                reason_code,
            )?;
            println!("{payload}");
        } else {
            println!("{}", serde_json::to_string_pretty(&preview)?);
        }
        return Ok(());
    }

    let registry_url = registry_url.with_context(|| {
        format!("Live module {cli_label} requires --registry-url or RUSTOK_MODULE_REGISTRY_URL")
    })?;
    let reason =
        reason.with_context(|| format!("Live module {cli_label} requires --reason <text>"))?;
    let reason_code = reason_code.with_context(|| {
        format!(
            "Live module {cli_label} requires --reason-code <{}>",
            supported_reason_codes.join("|")
        )
    })?;
    let payload = publish_decision_via_registry_live(
        &registry_url,
        request_id,
        action_key,
        reason,
        reason_code,
        disabled_message,
    )?;
    println!("{payload}");
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

fn approve_reason_argument(args: &[String]) -> Option<String> {
    if let Some(index) = args.iter().position(|arg| arg == "--approve-reason") {
        return args
            .get(index + 1)
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty());
    }

    None
}

fn manual_review_confirmation_argument(args: &[String]) -> bool {
    args.iter().any(|arg| arg == "--confirm-manual-review")
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

fn reason_code_argument(args: &[String]) -> Result<Option<String>> {
    normalized_reason_code_argument(
        args,
        REGISTRY_YANK_REASON_CODES,
        "module yank",
        "--reason-code",
    )
}

fn owner_transfer_reason_code_argument(args: &[String]) -> Result<Option<String>> {
    normalized_reason_code_argument(
        args,
        REGISTRY_OWNER_TRANSFER_REASON_CODES,
        "module owner-transfer",
        "--reason-code",
    )
}

fn approve_reason_code_argument(args: &[String]) -> Result<Option<String>> {
    normalized_reason_code_argument(
        args,
        REGISTRY_APPROVE_OVERRIDE_REASON_CODES,
        "module publish approve override",
        "--approve-reason-code",
    )
}

fn validation_stage_reason_code_argument(args: &[String]) -> Result<Option<String>> {
    normalized_reason_code_argument(
        args,
        REGISTRY_VALIDATION_STAGE_REASON_CODES,
        "module stage",
        "--reason-code",
    )
}

fn decision_reason_code_argument(
    args: &[String],
    command_label: &str,
    allowed: &[&str],
) -> Result<Option<String>> {
    normalized_reason_code_argument(
        args,
        allowed,
        &format!("module {command_label}"),
        "--reason-code",
    )
}

fn normalized_reason_code_argument(
    args: &[String],
    allowed: &[&str],
    command_label: &str,
    flag: &str,
) -> Result<Option<String>> {
    let Some(index) = args.iter().position(|arg| arg == flag) else {
        return Ok(None);
    };

    let value = args
        .get(index + 1)
        .map(|value| value.trim().to_ascii_lowercase())
        .filter(|value| !value.is_empty());

    let Some(value) = value else {
        return Ok(None);
    };

    if !allowed.iter().any(|candidate| *candidate == value) {
        anyhow::bail!(
            "{} reason code '{}' is not supported; expected one of {}",
            command_label,
            value,
            allowed.join(", ")
        );
    }

    Ok(Some(value))
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
    approve_reason: Option<String>,
    approve_reason_code: Option<String>,
    confirm_manual_review: bool,
) -> Result<String> {
    let publisher = format!("publisher:{}", preview.slug);
    let preflight = publish_via_registry_preflight(registry_url, preview)?;
    ensure_live_publish_supported(preflight.moderation_policy.as_ref())?;
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
    ensure_live_publish_supported(create_response.moderation_policy.as_ref())?;

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
    let upload_status = fetch_registry_publish_status(registry_url, request_id)?;
    ensure_status_governance_action_enabled(
        &upload_status,
        "validate",
        "Registry publish validation is not enabled for this request.",
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
    let readiness = run_publish_follow_up_stages_if_needed(
        registry_url,
        preview,
        readiness,
        confirm_manual_review,
    )?;
    if readiness.approval_override_required
        && (approve_reason.as_deref().is_none_or(str::is_empty)
            || approve_reason_code.as_deref().is_none_or(str::is_empty))
    {
        let pending_stages = readiness
            .validation_stages
            .iter()
            .filter(|stage| !stage.status.eq_ignore_ascii_case("passed"))
            .map(|stage| format!("{} ({})", stage.key, stage.status))
            .collect::<Vec<_>>();
        let security_policy_hint = if readiness.validation_stages.iter().any(|stage| {
            !stage.status.eq_ignore_ascii_case("passed") && stage.requires_manual_confirmation
        }) {
            "; rerun with --confirm-manual-review to complete the operator-assisted security/policy review path, or provide explicit approve override fields"
        } else {
            ""
        };
        anyhow::bail!(
            "Registry publish request '{}' still has non-passed follow-up stages [{}]; rerun with --approve-reason and --approve-reason-code <{}> or mark the remaining stages as passed first{}",
            readiness.request_id,
            pending_stages.join(", "),
            if readiness.approval_override_reason_codes.is_empty() {
                REGISTRY_APPROVE_OVERRIDE_REASON_CODES.join("|")
            } else {
                readiness.approval_override_reason_codes.join("|")
            },
            security_policy_hint
        );
    }
    ensure_status_governance_action_enabled(
        &readiness,
        "approve",
        "Registry publish approval is not enabled for this request.",
    )?;

    let approve_endpoint = format!(
        "{}/v2/catalog/publish/{request_id}/approve",
        registry_url.trim_end_matches('/')
    );
    let approve_request = RegistryPublishDecisionHttpRequest {
        schema_version: REGISTRY_MUTATION_SCHEMA_VERSION,
        dry_run: false,
        reason: approve_reason,
        reason_code: approve_reason_code,
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
    reason_code: Option<String>,
) -> Result<String> {
    let endpoint = format!("{}/v2/catalog/yank", registry_url.trim_end_matches('/'));
    let request = build_yank_registry_request(preview, reason, reason_code);

    post_registry_json(&endpoint, &request)
}

fn yank_via_registry_live(
    registry_url: &str,
    preview: &ModuleYankDryRunPreview,
    reason: String,
    reason_code: String,
) -> Result<String> {
    let endpoint = format!("{}/v2/catalog/yank", registry_url.trim_end_matches('/'));
    let request = build_live_yank_registry_request(preview, Some(reason), Some(reason_code));
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
    reason_code: Option<String>,
) -> Result<String> {
    let endpoint = format!(
        "{}/v2/catalog/owner-transfer",
        registry_url.trim_end_matches('/')
    );
    let request = build_owner_transfer_registry_request(preview, reason, reason_code);

    post_registry_json(&endpoint, &request)
}

fn owner_transfer_via_registry_live(
    registry_url: &str,
    preview: &ModuleOwnerTransferDryRunPreview,
    reason: String,
    reason_code: String,
) -> Result<String> {
    let endpoint = format!(
        "{}/v2/catalog/owner-transfer",
        registry_url.trim_end_matches('/')
    );
    let request =
        build_live_owner_transfer_registry_request(preview, Some(reason), Some(reason_code));
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

fn publish_decision_via_registry_dry_run(
    registry_url: &str,
    request_id: &str,
    action_key: &str,
    reason: Option<String>,
    reason_code: Option<String>,
) -> Result<String> {
    let endpoint = format!(
        "{}/v2/catalog/publish/{request_id}/{}",
        registry_url.trim_end_matches('/'),
        action_key.replace('_', "-")
    );
    let request = RegistryPublishDecisionHttpRequest {
        schema_version: REGISTRY_MUTATION_SCHEMA_VERSION,
        dry_run: true,
        reason,
        reason_code,
    };
    post_registry_json(&endpoint, &request)
}

fn publish_decision_via_registry_live(
    registry_url: &str,
    request_id: &str,
    action_key: &str,
    reason: String,
    reason_code: String,
    disabled_message: &str,
) -> Result<String> {
    let status = fetch_registry_publish_status(registry_url, request_id)?;
    ensure_status_governance_action_enabled(&status, action_key, disabled_message)?;
    let endpoint = format!(
        "{}/v2/catalog/publish/{request_id}/{}",
        registry_url.trim_end_matches('/'),
        action_key.replace('_', "-")
    );
    let request = RegistryPublishDecisionHttpRequest {
        schema_version: REGISTRY_MUTATION_SCHEMA_VERSION,
        dry_run: false,
        reason: Some(reason),
        reason_code: Some(reason_code),
    };
    let actor = format!("xtask:module-{}", action_key.replace('_', "-"));
    let response: RegistryMutationHttpResponse =
        post_registry_json_parsed(&endpoint, &request, Some(&actor), None)?;
    if !response.accepted {
        anyhow::bail!(
            "Registry {} request was not accepted: {}",
            action_key,
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
    let status = fetch_registry_publish_status(registry_url, &preview.request_id)?;
    ensure_status_governance_action_enabled(
        &status,
        "stage_report",
        "Registry validation stage reporting is not enabled for this request.",
    )?;
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

fn publish_via_registry_preflight(
    registry_url: &str,
    preview: &ModulePublishDryRunPreview,
) -> Result<RegistryMutationHttpResponse> {
    let endpoint = format!("{}/v2/catalog/publish", registry_url.trim_end_matches('/'));
    let request = build_publish_registry_request(preview);
    post_registry_json_parsed(&endpoint, &request, Some("xtask:module-publish"), None)
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
    reason_code: Option<String>,
) -> RegistryYankHttpRequest {
    build_yank_registry_request_with_dry_run(preview, reason, reason_code, true)
}

fn build_live_yank_registry_request(
    preview: &ModuleYankDryRunPreview,
    reason: Option<String>,
    reason_code: Option<String>,
) -> RegistryYankHttpRequest {
    build_yank_registry_request_with_dry_run(preview, reason, reason_code, false)
}

fn build_yank_registry_request_with_dry_run(
    preview: &ModuleYankDryRunPreview,
    reason: Option<String>,
    reason_code: Option<String>,
    dry_run: bool,
) -> RegistryYankHttpRequest {
    RegistryYankHttpRequest {
        schema_version: REGISTRY_MUTATION_SCHEMA_VERSION,
        dry_run,
        slug: preview.slug.clone(),
        version: preview.version.clone(),
        reason,
        reason_code,
    }
}

fn build_owner_transfer_registry_request(
    preview: &ModuleOwnerTransferDryRunPreview,
    reason: Option<String>,
    reason_code: Option<String>,
) -> RegistryOwnerTransferHttpRequest {
    build_owner_transfer_registry_request_with_dry_run(preview, reason, reason_code, true)
}

fn build_live_owner_transfer_registry_request(
    preview: &ModuleOwnerTransferDryRunPreview,
    reason: Option<String>,
    reason_code: Option<String>,
) -> RegistryOwnerTransferHttpRequest {
    build_owner_transfer_registry_request_with_dry_run(preview, reason, reason_code, false)
}

fn build_owner_transfer_registry_request_with_dry_run(
    preview: &ModuleOwnerTransferDryRunPreview,
    reason: Option<String>,
    reason_code: Option<String>,
    dry_run: bool,
) -> RegistryOwnerTransferHttpRequest {
    RegistryOwnerTransferHttpRequest {
        schema_version: REGISTRY_MUTATION_SCHEMA_VERSION,
        dry_run,
        slug: preview.slug.clone(),
        new_owner_actor: preview.new_owner_actor.clone(),
        reason,
        reason_code,
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
        reason_code: preview.reason_code.clone(),
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

fn run_publish_follow_up_stages_if_needed(
    registry_url: &str,
    preview: &ModulePublishDryRunPreview,
    mut status: RegistryPublishStatusHttpResponse,
    confirm_manual_review: bool,
) -> Result<RegistryPublishStatusHttpResponse> {
    loop {
        let next_stage = status
            .validation_stages
            .iter()
            .find(|stage| publish_status_stage_should_auto_run(stage, confirm_manual_review))
            .cloned();
        let Some(next_stage) = next_stage else {
            break;
        };

        let plan = build_module_validation_stage_run_preview(
            preview,
            &status.request_id,
            &next_stage.key,
            None,
            Some(&next_stage),
        )?;
        run_validation_stage_plan_via_registry(registry_url, &plan)?;
        status = fetch_registry_publish_status(registry_url, &status.request_id)?;
        ensure_publish_status_not_rejected(&status)?;
    }

    Ok(status)
}

fn fetch_registry_publish_status(
    registry_url: &str,
    request_id: &str,
) -> Result<RegistryPublishStatusHttpResponse> {
    let endpoint = format!(
        "{}/v2/catalog/publish/{request_id}",
        registry_url.trim_end_matches('/')
    );
    get_registry_json_parsed(&endpoint, Some("xtask:module-publish"))
}

fn ensure_live_publish_supported(
    moderation_policy: Option<&RegistryModerationPolicyHttpResponse>,
) -> Result<()> {
    let Some(moderation_policy) = moderation_policy else {
        return Ok(());
    };

    if moderation_policy.live_publish_supported {
        return Ok(());
    }

    let reason_code = moderation_policy
        .restriction_reason_code
        .as_deref()
        .map(|code| format!(" ({code})"))
        .unwrap_or_default();
    anyhow::bail!(
        "{}{}",
        moderation_policy.restriction_reason.trim(),
        reason_code
    );
}

fn status_governance_action<'a>(
    status: &'a RegistryPublishStatusHttpResponse,
    key: &str,
) -> Option<&'a RegistryGovernanceActionHttpResponse> {
    status
        .governance_actions
        .iter()
        .find(|action| action.key.eq_ignore_ascii_case(key))
}

fn ensure_status_governance_action_enabled(
    status: &RegistryPublishStatusHttpResponse,
    key: &str,
    fallback_message: &str,
) -> Result<()> {
    let Some(action) = status_governance_action(status, key) else {
        return Ok(());
    };

    if action.enabled {
        return Ok(());
    }

    anyhow::bail!("{}", action.reason.as_deref().unwrap_or(fallback_message));
}

fn fetch_registry_validation_stage_metadata(
    registry_url: &str,
    request_id: &str,
    stage_key: &str,
) -> Result<RegistryPublishStatusValidationStage> {
    let status = fetch_registry_publish_status(registry_url, request_id)?;
    status
        .validation_stages
        .into_iter()
        .find(|stage| stage.key.eq_ignore_ascii_case(stage_key))
        .with_context(|| {
            format!(
                "Registry publish request '{}' does not expose validation stage '{}'",
                request_id, stage_key
            )
        })
}

fn suggested_reason_code_for_stage_status(
    stage: &RegistryPublishStatusValidationStage,
    status: &str,
) -> Option<String> {
    match status.trim().to_ascii_lowercase().as_str() {
        "passed" => stage.suggested_pass_reason_code.clone(),
        "failed" => stage.suggested_failure_reason_code.clone(),
        "blocked" => stage.suggested_blocked_reason_code.clone(),
        _ => None,
    }
}

fn publish_status_stage_should_auto_run(
    stage: &RegistryPublishStatusValidationStage,
    confirm_manual_review: bool,
) -> bool {
    if !stage.runnable || stage.status.eq_ignore_ascii_case("passed") {
        return false;
    }

    if stage.requires_manual_confirmation && !confirm_manual_review {
        return false;
    }

    matches!(
        stage.execution_mode.as_str(),
        "local_runner" | "operator_assisted"
    )
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
                    .trim_matches(|ch| ch == '[' || ch == ']')
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
            "--target-dir".to_string(),
            "target_xtask_validate".to_string(),
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

fn build_module_validation_stage_run_preview(
    preview: &ModulePublishDryRunPreview,
    request_id: &str,
    stage: &str,
    detail_override: Option<String>,
    stage_metadata: Option<&RegistryPublishStatusValidationStage>,
) -> Result<ModuleValidationStageRunPreview> {
    let normalized_stage = normalize_executable_validation_stage(stage)?;
    let (commands, fallback_requires_manual_confirmation) = match normalized_stage {
        "compile_smoke" => (build_compile_smoke_commands(preview), false),
        "targeted_tests" => (build_module_test_plan(preview).commands, false),
        "security_policy_review" => (build_security_policy_review_commands(preview), true),
        _ => unreachable!(),
    };
    let requires_manual_confirmation = stage_metadata
        .map(|stage| stage.requires_manual_confirmation)
        .unwrap_or(fallback_requires_manual_confirmation);
    let running_detail = detail_override.unwrap_or_else(|| {
        executable_validation_stage_running_detail(normalized_stage, &preview.slug)
    });
    let success_reason_code = stage_metadata
        .and_then(|stage| suggested_reason_code_for_stage_status(stage, "passed"))
        .unwrap_or_else(|| validation_stage_success_reason_code(normalized_stage).to_string());
    let failure_reason_code = stage_metadata
        .and_then(|stage| suggested_reason_code_for_stage_status(stage, "failed"))
        .unwrap_or_else(|| validation_stage_failure_reason_code(normalized_stage).to_string());

    Ok(ModuleValidationStageRunPreview {
        action: "validation_stage_run".to_string(),
        slug: preview.slug.clone(),
        request_id: request_id.to_string(),
        stage: normalized_stage.to_string(),
        requires_manual_confirmation,
        running_detail,
        success_detail: executable_validation_stage_success_detail(normalized_stage, &preview.slug),
        success_reason_code,
        failure_detail_prefix: executable_validation_stage_failure_prefix(normalized_stage),
        failure_reason_code,
        commands,
    })
}

fn normalize_executable_validation_stage(stage: &str) -> Result<&'static str> {
    let stage = stage.trim().to_ascii_lowercase();
    match stage.as_str() {
        "compile_smoke" => Ok("compile_smoke"),
        "targeted_tests" => Ok("targeted_tests"),
        "security_policy_review" => Ok("security_policy_review"),
        _ => anyhow::bail!(
            "module stage-run stage '{}' is not supported; expected one of compile_smoke, targeted_tests, or security_policy_review",
            stage
        ),
    }
}

fn build_compile_smoke_commands(preview: &ModulePublishDryRunPreview) -> Vec<ModuleCommandPreview> {
    let mut commands = vec![package_check_command("module crate", &preview.crate_name)];

    if let Some(admin) = preview.ui_packages.admin.as_ref() {
        commands.push(package_check_command("admin ui crate", &admin.crate_name));
    }

    if let Some(storefront) = preview.ui_packages.storefront.as_ref() {
        commands.push(package_check_command(
            "storefront ui crate",
            &storefront.crate_name,
        ));
    }

    commands
}

fn build_security_policy_review_commands(
    preview: &ModulePublishDryRunPreview,
) -> Vec<ModuleCommandPreview> {
    vec![
        ModuleCommandPreview {
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
        },
        ModuleCommandPreview {
            label: "module test dry-run".to_string(),
            argv: vec![
                "cargo".to_string(),
                "run".to_string(),
                "-p".to_string(),
                "xtask".to_string(),
                "--".to_string(),
                "module".to_string(),
                "test".to_string(),
                preview.slug.clone(),
                "--dry-run".to_string(),
            ],
        },
    ]
}

fn executable_validation_stage_running_detail(stage: &str, slug: &str) -> String {
    match stage {
        "compile_smoke" => format!("Running local compile smoke checks for module '{slug}'."),
        "targeted_tests" => format!("Running local targeted tests for module '{slug}'."),
        "security_policy_review" => {
            format!("Running local security/policy review preflight for module '{slug}'.")
        }
        _ => format!("Running local validation stage '{stage}' for module '{slug}'."),
    }
}

fn executable_validation_stage_success_detail(stage: &str, slug: &str) -> String {
    match stage {
        "compile_smoke" => {
            format!("Local compile smoke checks passed for module '{slug}'.")
        }
        "targeted_tests" => {
            format!("Local targeted tests completed successfully for module '{slug}'.")
        }
        "security_policy_review" => format!(
            "Local security/policy preflight completed and manual review was confirmed for module '{slug}'."
        ),
        _ => format!("Local validation stage '{stage}' completed successfully for '{slug}'."),
    }
}

fn executable_validation_stage_failure_prefix(stage: &str) -> String {
    match stage {
        "compile_smoke" => "Local compile smoke failed".to_string(),
        "targeted_tests" => "Local targeted tests failed".to_string(),
        "security_policy_review" => "Local security/policy review preflight failed".to_string(),
        _ => format!("Local validation stage '{stage}' failed"),
    }
}

fn validation_stage_success_reason_code(stage: &str) -> &'static str {
    match stage {
        "security_policy_review" => "manual_review_complete",
        _ => "local_runner_passed",
    }
}

fn validation_stage_failure_reason_code(stage: &str) -> &'static str {
    match stage {
        "compile_smoke" => "build_failure",
        "targeted_tests" => "test_failure",
        "security_policy_review" => "policy_preflight_failed",
        _ => "other",
    }
}

fn validation_stage_status_requires_reason_code(status: &str) -> bool {
    matches!(status, "passed" | "failed" | "blocked")
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

fn run_validation_stage_plan_via_registry(
    registry_url: &str,
    plan: &ModuleValidationStageRunPreview,
) -> Result<()> {
    let running_preview = ModuleValidationStageDryRunPreview {
        action: "validation_stage".to_string(),
        request_id: plan.request_id.clone(),
        stage: plan.stage.clone(),
        status: "running".to_string(),
        detail: Some(plan.running_detail.clone()),
        reason_code: None,
        requeue: false,
    };
    validation_stage_via_registry_live(registry_url, &running_preview)?;

    for command in &plan.commands {
        println!("  > {}", command.argv.join(" "));
        if let Err(error) = run_command(command) {
            let failed_preview = ModuleValidationStageDryRunPreview {
                action: "validation_stage".to_string(),
                request_id: plan.request_id.clone(),
                stage: plan.stage.clone(),
                status: "failed".to_string(),
                detail: Some(format!("{}: {error}", plan.failure_detail_prefix)),
                reason_code: Some(plan.failure_reason_code.clone()),
                requeue: false,
            };
            let report_error = validation_stage_via_registry_live(registry_url, &failed_preview)
                .err()
                .map(|report_error| {
                    format!("; failed to persist failed stage status: {report_error}")
                })
                .unwrap_or_default();
            anyhow::bail!("{error}{report_error}");
        }
    }

    let passed_preview = ModuleValidationStageDryRunPreview {
        action: "validation_stage".to_string(),
        request_id: plan.request_id.clone(),
        stage: plan.stage.clone(),
        status: "passed".to_string(),
        detail: Some(plan.success_detail.clone()),
        reason_code: Some(plan.success_reason_code.clone()),
        requeue: false,
    };
    validation_stage_via_registry_live(registry_url, &passed_preview)?;
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
    validate_module_dependency_alignment(slug, spec, &package_manifest)?;

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
    validate_module_ui_classification(slug, &package_manifest)?;
    validate_module_documentation_contract(slug, &module_root)?;
    validate_runtime_dependency_contract(slug, spec, &module_root)?;

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

fn validate_module_dependency_alignment(
    slug: &str,
    spec: &ModuleSpec,
    manifest: &ModulePackageManifest,
) -> Result<()> {
    let expected = spec
        .depends_on
        .clone()
        .unwrap_or_default()
        .into_iter()
        .collect::<HashSet<_>>();
    let declared = manifest
        .dependencies
        .keys()
        .cloned()
        .collect::<HashSet<_>>();

    let missing = expected.difference(&declared).cloned().collect::<Vec<_>>();
    let extra = declared.difference(&expected).cloned().collect::<Vec<_>>();

    if !missing.is_empty() || !extra.is_empty() {
        let mut problems = Vec::new();
        if !missing.is_empty() {
            problems.push(format!(
                "missing in rustok-module.toml [dependencies]: {}",
                missing.join(", ")
            ));
        }
        if !extra.is_empty() {
            problems.push(format!(
                "extra in rustok-module.toml [dependencies]: {}",
                extra.join(", ")
            ));
        }
        anyhow::bail!(
            "Module '{slug}' dependency drift between modules.toml and rustok-module.toml: {}",
            problems.join("; ")
        );
    }

    for (dependency, constraint) in &manifest.dependencies {
        if constraint.version_req.trim().is_empty() {
            anyhow::bail!(
                "Module '{slug}' dependency '{}' is missing version_req in rustok-module.toml",
                dependency
            );
        }
    }

    Ok(())
}

fn validate_module_ui_classification(slug: &str, manifest: &ModulePackageManifest) -> Result<()> {
    let has_admin_ui = manifest.provides.admin_ui.is_some();
    let has_storefront_ui = manifest.provides.storefront_ui.is_some();
    let classification = manifest.module.ui_classification.trim();

    if classification.is_empty() {
        if !has_admin_ui && !has_storefront_ui {
            anyhow::bail!(
                "Module '{slug}' has no UI wiring and must declare module.ui_classification in rustok-module.toml"
            );
        }
        return Ok(());
    }

    match classification {
        "dual_surface" => {
            if !has_admin_ui || !has_storefront_ui {
                anyhow::bail!(
                    "Module '{slug}' declares ui_classification='dual_surface' but manifest wiring is incomplete"
                );
            }
        }
        "admin_only" => {
            if !has_admin_ui || has_storefront_ui {
                anyhow::bail!(
                    "Module '{slug}' declares ui_classification='admin_only' but manifest wiring does not match"
                );
            }
        }
        "storefront_only" => {
            if has_admin_ui || !has_storefront_ui {
                anyhow::bail!(
                    "Module '{slug}' declares ui_classification='storefront_only' but manifest wiring does not match"
                );
            }
        }
        "no_ui" | "capability_only" | "future_ui" => {
            if has_admin_ui || has_storefront_ui {
                anyhow::bail!(
                    "Module '{slug}' declares ui_classification='{classification}' but also wires UI surfaces"
                );
            }
        }
        _ => {
            anyhow::bail!(
                "Module '{slug}' declares unsupported ui_classification '{}'",
                classification
            );
        }
    }

    Ok(())
}

fn validate_module_documentation_contract(slug: &str, module_root: &Path) -> Result<()> {
    let workspace = workspace_root();
    let module_root = if module_root.is_absolute() {
        module_root.to_path_buf()
    } else {
        workspace.join(module_root)
    };

    let readme_path = module_root.join("README.md");
    if !readme_path.exists() {
        anyhow::bail!(
            "Module '{slug}' requires README.md at {}",
            readme_path.display()
        );
    }
    let root_readme = fs::read_to_string(&readme_path)
        .with_context(|| format!("Failed to read module README at {}", readme_path.display()))?;
    let normalized_root_readme = root_readme.to_ascii_lowercase();
    for (section, display_name) in [
        ("## purpose", "## Purpose"),
        ("## responsibilities", "## Responsibilities"),
        ("## entry points", "## Entry points"),
        ("## interactions", "## Interactions"),
    ] {
        if !normalized_root_readme.contains(section) {
            anyhow::bail!(
                "Module '{slug}' README.md must contain a `{}` section",
                display_name
            );
        }
    }
    if !normalized_root_readme.contains("docs/readme.md") {
        anyhow::bail!(
            "Module '{slug}' README.md must link to `docs/README.md`"
        );
    }

    let docs_readme_path = module_root.join("docs").join("README.md");
    if !docs_readme_path.exists() {
        anyhow::bail!(
            "Module '{slug}' requires docs/README.md at {}",
            docs_readme_path.display()
        );
    }

    let implementation_plan_path = module_root.join("docs").join("implementation-plan.md");
    if !implementation_plan_path.exists() {
        anyhow::bail!(
            "Module '{slug}' requires docs/implementation-plan.md at {}",
            implementation_plan_path.display()
        );
    }

    let docs_index_path = workspace.join("docs").join("modules").join("_index.md");
    let docs_index = fs::read_to_string(&docs_index_path).with_context(|| {
        format!(
            "Failed to read module documentation index at {}",
            docs_index_path.display()
        )
    })?;
    let relative_module_root = module_root
        .strip_prefix(&workspace)
        .with_context(|| {
            format!(
                "Failed to resolve module root '{}' relative to workspace '{}'",
                module_root.display(),
                workspace.display()
            )
        })?
        .to_string_lossy()
        .replace('\\', "/");
    let expected_docs_link = format!("../../{relative_module_root}/docs/README.md");
    if !docs_index.contains(&expected_docs_link) {
        anyhow::bail!(
            "Module '{slug}' is missing docs/README.md link in {}",
            docs_index_path.display()
        );
    }
    let expected_plan_link = format!("../../{relative_module_root}/docs/implementation-plan.md");
    if !docs_index.contains(&expected_plan_link) {
        anyhow::bail!(
            "Module '{slug}' is missing docs/implementation-plan.md link in {}",
            docs_index_path.display()
        );
    }

    Ok(())
}

fn validate_runtime_dependency_contract(
    slug: &str,
    spec: &ModuleSpec,
    module_root: &Path,
) -> Result<()> {
    let workspace = workspace_root();
    let module_root = if module_root.is_absolute() {
        module_root.to_path_buf()
    } else {
        workspace.join(module_root)
    };
    let lib_rs_path = module_root.join("src").join("lib.rs");
    if !lib_rs_path.exists() {
        return Ok(());
    }

    let runtime_dependencies =
        parse_runtime_dependencies_from_lib_rs(&lib_rs_path)?.unwrap_or_default();
    let manifest_dependencies = spec.depends_on.clone().unwrap_or_default();
    if runtime_dependencies != manifest_dependencies {
        anyhow::bail!(
            "Module '{slug}' dependency drift between modules.toml and RusToKModule::dependencies(): runtime declares {:?}, manifest declares {:?}",
            runtime_dependencies,
            manifest_dependencies
        );
    }

    Ok(())
}

fn parse_runtime_dependencies_from_lib_rs(lib_rs_path: &Path) -> Result<Option<Vec<String>>> {
    let lib_rs = fs::read_to_string(lib_rs_path)
        .with_context(|| format!("Failed to read {}", lib_rs_path.display()))?;
    let Some(fn_pos) = lib_rs.find("fn dependencies(&self)") else {
        return Ok(None);
    };

    let after_fn = &lib_rs[fn_pos..];
    let body_start = after_fn.find('{').with_context(|| {
        format!(
            "Failed to locate RusToKModule::dependencies() body in {}",
            lib_rs_path.display()
        )
    })?;
    let body = &after_fn[body_start..];
    let slice_start = body.find("&[").with_context(|| {
        format!(
            "Failed to parse RusToKModule::dependencies() slice in {}",
            lib_rs_path.display()
        )
    })?;
    let slice_body = &body[slice_start + 2..];
    let slice_end = slice_body.find(']').with_context(|| {
        format!(
            "Failed to locate closing dependency slice in {}",
            lib_rs_path.display()
        )
    })?;

    Ok(Some(parse_quoted_string_literals(&slice_body[..slice_end])))
}

fn parse_quoted_string_literals(input: &str) -> Vec<String> {
    let mut values = Vec::new();
    let mut current = String::new();
    let mut in_string = false;
    let mut escaped = false;

    for ch in input.chars() {
        if !in_string {
            if ch == '"' {
                in_string = true;
                current.clear();
            }
            continue;
        }

        if escaped {
            current.push(ch);
            escaped = false;
            continue;
        }

        match ch {
            '\\' => escaped = true,
            '"' => {
                values.push(current.clone());
                current.clear();
                in_string = false;
            }
            _ => current.push(ch),
        }
    }

    values
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
        auto_approve_argument, build_live_owner_transfer_registry_request,
        build_live_publish_registry_request, build_live_validation_stage_registry_request,
        build_live_yank_registry_request, build_module_test_plan,
        build_owner_transfer_registry_request, build_publish_registry_request,
        build_validation_stage_registry_request, build_yank_registry_request, detail_argument,
        module_owner_transfer_command, module_publish_command, module_stage_command,
        module_test_command, module_yank_command, reason_argument, reason_code_argument,
        registry_endpoint_uses_loopback, registry_url_argument, resolve_workspace_inherited_string,
        validate_module_publish_contract, validate_module_ui_surface_contract, workspace_root,
        ModuleMarketplacePreview, ModuleOwnerTransferDryRunPreview, ModulePackageManifest,
        ModulePublishDryRunPreview, ModuleUiPackagePreview, ModuleUiPackagesPreview,
        ModuleValidationStageDryRunPreview, ModuleYankDryRunPreview,
        REGISTRY_MUTATION_SCHEMA_VERSION, REGISTRY_YANK_REASON_CODES,
    };
    use std::{
        env,
        path::{Path, PathBuf},
        sync::{Mutex, MutexGuard, OnceLock},
    };

    struct WorkspaceRootGuard {
        previous_dir: PathBuf,
        _lock: MutexGuard<'static, ()>,
    }

    impl WorkspaceRootGuard {
        fn enter() -> Self {
            static WORKSPACE_CWD_GUARD: OnceLock<Mutex<()>> = OnceLock::new();

            let lock = WORKSPACE_CWD_GUARD
                .get_or_init(|| Mutex::new(()))
                .lock()
                .expect("workspace cwd guard should lock");
            let previous_dir = env::current_dir().expect("current dir should resolve");
            env::set_current_dir(workspace_root()).expect("workspace root should be accessible");

            Self {
                previous_dir,
                _lock: lock,
            }
        }
    }

    impl Drop for WorkspaceRootGuard {
        fn drop(&mut self) {
            env::set_current_dir(&self.previous_dir)
                .expect("current dir should restore after xtask test");
        }
    }

    struct EnvVarGuard {
        key: &'static str,
        previous_value: Option<String>,
        _lock: MutexGuard<'static, ()>,
    }

    impl EnvVarGuard {
        fn set(key: &'static str, value: Option<&str>) -> Self {
            static ENV_VAR_GUARD: OnceLock<Mutex<()>> = OnceLock::new();

            let lock = ENV_VAR_GUARD
                .get_or_init(|| Mutex::new(()))
                .lock()
                .expect("env var guard should lock");
            let previous_value = env::var(key).ok();
            match value {
                Some(value) => env::set_var(key, value),
                None => env::remove_var(key),
            }

            Self {
                key,
                previous_value,
                _lock: lock,
            }
        }
    }

    impl Drop for EnvVarGuard {
        fn drop(&mut self) {
            match &self.previous_value {
                Some(value) => env::set_var(self.key, value),
                None => env::remove_var(self.key),
            }
        }
    }

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
    fn build_live_publish_registry_request_turns_off_dry_run() {
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

        let request_body = serde_json::to_value(build_live_publish_registry_request(&preview))
            .expect("live publish request should serialize");

        assert_eq!(request_body["dry_run"], false);
        assert_eq!(request_body["module"]["slug"], "blog");
    }

    #[test]
    fn module_publish_command_requires_slug() {
        let error = module_publish_command(&[])
            .expect_err("publish command without slug should fail immediately");

        assert!(error
            .to_string()
            .contains("module publish requires a module slug"));
    }

    #[test]
    fn module_publish_command_live_requires_registry_url() {
        let _cwd_guard = WorkspaceRootGuard::enter();
        let _env_guard = EnvVarGuard::set("RUSTOK_MODULE_REGISTRY_URL", None);
        let args = vec!["blog".to_string()];

        let error = module_publish_command(&args)
            .expect_err("live publish should require registry url before any network call");

        assert!(error
            .to_string()
            .contains("Live module publish requires --registry-url"));
    }

    #[test]
    fn module_publish_command_rejects_unknown_slug() {
        let _cwd_guard = WorkspaceRootGuard::enter();
        let args = vec!["missing-module".to_string(), "--dry-run".to_string()];

        let error = module_publish_command(&args)
            .expect_err("unknown slug should fail before any network call");

        assert!(error
            .to_string()
            .contains("Unknown module slug 'missing-module'"));
    }

    #[test]
    fn module_test_command_requires_slug() {
        let error =
            module_test_command(&[]).expect_err("module test without slug should fail immediately");

        assert!(error
            .to_string()
            .contains("module test requires a module slug"));
    }

    #[test]
    fn build_validation_stage_registry_request_serializes_stage_contract() {
        let preview = ModuleValidationStageDryRunPreview {
            action: "validation_stage".to_string(),
            request_id: "rpr_test".to_string(),
            stage: "compile_smoke".to_string(),
            status: "passed".to_string(),
            detail: Some("External CI recorded the result.".to_string()),
            reason_code: None,
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
    fn build_validation_stage_registry_request_serializes_requeue_contract() {
        let preview = ModuleValidationStageDryRunPreview {
            action: "validation_stage".to_string(),
            request_id: "rpr_retry".to_string(),
            stage: "targeted_tests".to_string(),
            status: "queued".to_string(),
            detail: Some("Waiting for external rerun.".to_string()),
            reason_code: None,
            requeue: true,
        };

        let request_body = serde_json::to_value(build_validation_stage_registry_request(&preview))
            .expect("stage request should serialize");

        assert_eq!(
            request_body
                .get("stage")
                .and_then(serde_json::Value::as_str),
            Some("targeted_tests")
        );
        assert_eq!(
            request_body
                .get("status")
                .and_then(serde_json::Value::as_str),
            Some("queued")
        );
        assert_eq!(
            request_body
                .get("requeue")
                .and_then(serde_json::Value::as_bool),
            Some(true)
        );
        assert_eq!(
            request_body
                .get("detail")
                .and_then(serde_json::Value::as_str),
            Some("Waiting for external rerun.")
        );
        assert_eq!(
            request_body
                .get("dry_run")
                .and_then(serde_json::Value::as_bool),
            Some(true)
        );
    }

    #[test]
    fn build_live_validation_stage_registry_request_turns_off_dry_run() {
        let preview = ModuleValidationStageDryRunPreview {
            action: "validation_stage".to_string(),
            request_id: "rpr_live".to_string(),
            stage: "security_policy_review".to_string(),
            status: "blocked".to_string(),
            detail: Some("Waiting for manual policy sign-off.".to_string()),
            reason_code: None,
            requeue: false,
        };

        let request_body =
            serde_json::to_value(build_live_validation_stage_registry_request(&preview))
                .expect("live stage request should serialize");

        assert_eq!(
            request_body
                .get("dry_run")
                .and_then(serde_json::Value::as_bool),
            Some(false)
        );
        assert_eq!(
            request_body
                .get("status")
                .and_then(serde_json::Value::as_str),
            Some("blocked")
        );
    }

    #[test]
    fn build_validation_stage_registry_request_preserves_null_detail() {
        let preview = ModuleValidationStageDryRunPreview {
            action: "validation_stage".to_string(),
            request_id: "rpr_null".to_string(),
            stage: "compile_smoke".to_string(),
            status: "running".to_string(),
            detail: None,
            reason_code: None,
            requeue: false,
        };

        let request_body = serde_json::to_value(build_validation_stage_registry_request(&preview))
            .expect("stage request should serialize");

        assert!(
            request_body.get("detail").is_some(),
            "stage request keeps explicit detail field for API contract stability"
        );
        assert!(
            request_body
                .get("detail")
                .is_some_and(serde_json::Value::is_null),
            "detail should serialize as null when operator omitted it"
        );
    }

    #[test]
    fn module_stage_command_rejects_unknown_status() {
        let args = vec![
            "rpr_test".to_string(),
            "compile_smoke".to_string(),
            "skipped".to_string(),
        ];

        let error = module_stage_command(&args).expect_err("unsupported stage status must fail");

        assert!(error
            .to_string()
            .contains("module stage status 'skipped' is not supported"));
    }

    #[test]
    fn module_stage_command_requires_request_stage_and_status() {
        let args = vec!["rpr_test".to_string(), "compile_smoke".to_string()];

        let error = module_stage_command(&args)
            .expect_err("stage command without full argument set should fail immediately");

        assert!(error
            .to_string()
            .contains("module stage requires a request id, stage key, and status"));
    }

    #[test]
    fn module_stage_command_rejects_requeue_without_queued_status() {
        let args = vec![
            "rpr_test".to_string(),
            "compile_smoke".to_string(),
            "passed".to_string(),
            "--requeue".to_string(),
        ];

        let error = module_stage_command(&args)
            .expect_err("requeue should only be accepted for queued status");

        assert!(error
            .to_string()
            .contains("module stage --requeue requires status 'queued'"));
    }

    #[test]
    fn module_stage_command_rejects_empty_request_id() {
        let args = vec![
            "   ".to_string(),
            "compile_smoke".to_string(),
            "queued".to_string(),
        ];

        let error = module_stage_command(&args)
            .expect_err("empty request id should fail before any registry lookup");

        assert!(error
            .to_string()
            .contains("module stage requires a non-empty request id"));
    }

    #[test]
    fn module_stage_command_rejects_empty_stage_key() {
        let args = vec![
            "rpr_test".to_string(),
            "   ".to_string(),
            "queued".to_string(),
        ];

        let error = module_stage_command(&args)
            .expect_err("empty stage key should fail before any registry lookup");

        assert!(error
            .to_string()
            .contains("module stage requires a non-empty stage key"));
    }

    #[test]
    fn module_stage_command_live_requires_registry_url() {
        let args = vec![
            "rpr_test".to_string(),
            "compile_smoke".to_string(),
            "running".to_string(),
        ];

        let error = module_stage_command(&args)
            .expect_err("live stage update should require registry url before any network call");

        assert!(error
            .to_string()
            .contains("Live module stage update requires --registry-url"));
    }

    #[test]
    fn registry_url_argument_prefers_cli_value_over_env() {
        let _guard = EnvVarGuard::set("RUSTOK_MODULE_REGISTRY_URL", Some("http://env.example"));

        assert_eq!(
            registry_url_argument(&[
                "--registry-url".to_string(),
                "  http://cli.example  ".to_string(),
            ]),
            Some("http://cli.example".to_string())
        );
    }

    #[test]
    fn registry_url_argument_uses_trimmed_env_and_ignores_blank_values() {
        let _guard = EnvVarGuard::set("RUSTOK_MODULE_REGISTRY_URL", Some("  http://env.example  "));
        assert_eq!(
            registry_url_argument(&[]),
            Some("http://env.example".to_string())
        );

        drop(_guard);

        let _guard = EnvVarGuard::set("RUSTOK_MODULE_REGISTRY_URL", Some("   "));
        assert_eq!(registry_url_argument(&[]), None);
    }

    #[test]
    fn reason_argument_trims_and_ignores_blank_values() {
        assert_eq!(
            reason_argument(&["--reason".to_string(), "  ownership move  ".to_string()]),
            Some("ownership move".to_string())
        );
        assert_eq!(
            reason_argument(&["--reason".to_string(), "   ".to_string()]),
            None
        );
    }

    #[test]
    fn detail_argument_trims_and_ignores_blank_values() {
        assert_eq!(
            detail_argument(&["--detail".to_string(), "  external runner  ".to_string()]),
            Some("external runner".to_string())
        );
        assert_eq!(
            detail_argument(&["--detail".to_string(), "   ".to_string()]),
            None
        );
    }

    #[test]
    fn reason_code_argument_trims_and_normalizes_values() {
        assert_eq!(
            reason_code_argument(&[
                "--reason-code".to_string(),
                "  Critical_Regression  ".to_string(),
            ])
            .expect("supported reason code should parse"),
            Some("critical_regression".to_string())
        );
    }

    #[test]
    fn reason_code_argument_rejects_unknown_values() {
        let error = reason_code_argument(&["--reason-code".to_string(), "surprise".to_string()])
            .expect_err("unknown yank reason code must fail");

        assert!(error
            .to_string()
            .contains("module yank reason code 'surprise' is not supported"));
    }

    #[test]
    fn module_owner_transfer_command_rejects_empty_new_owner_actor() {
        let args = vec!["blog".to_string(), "   ".to_string()];

        let error = module_owner_transfer_command(&args)
            .expect_err("empty new owner actor must fail before manifest lookup");

        assert!(error
            .to_string()
            .contains("module owner-transfer requires a non-empty new owner actor"));
    }

    #[test]
    fn module_owner_transfer_command_requires_slug_and_new_owner_actor() {
        let args = vec!["blog".to_string()];

        let error = module_owner_transfer_command(&args)
            .expect_err("owner-transfer command without actor should fail immediately");

        assert!(error
            .to_string()
            .contains("module owner-transfer requires a module slug and new owner actor"));
    }

    #[test]
    fn module_owner_transfer_command_live_requires_reason() {
        let _guard = WorkspaceRootGuard::enter();
        let args = vec![
            "blog".to_string(),
            "publisher:forum".to_string(),
            "--registry-url".to_string(),
            "http://127.0.0.1:5150".to_string(),
        ];

        let error = module_owner_transfer_command(&args)
            .expect_err("live owner-transfer should require reason before any network call");

        assert!(error
            .to_string()
            .contains("Live module owner-transfer requires --reason <text>"));
    }

    #[test]
    fn module_owner_transfer_command_live_requires_registry_url() {
        let _cwd_guard = WorkspaceRootGuard::enter();
        let _env_guard = EnvVarGuard::set("RUSTOK_MODULE_REGISTRY_URL", None);
        let args = vec![
            "blog".to_string(),
            "publisher:forum".to_string(),
            "--reason".to_string(),
            "ownership move".to_string(),
        ];

        let error = module_owner_transfer_command(&args)
            .expect_err("live owner-transfer should require registry url before any network call");

        assert!(error
            .to_string()
            .contains("Live module owner-transfer requires --registry-url"));
    }

    #[test]
    fn module_yank_command_live_requires_reason() {
        let _guard = WorkspaceRootGuard::enter();
        let args = vec![
            "blog".to_string(),
            "1.2.3".to_string(),
            "--reason-code".to_string(),
            "rollback".to_string(),
            "--registry-url".to_string(),
            "http://127.0.0.1:5150".to_string(),
        ];

        let error = module_yank_command(&args)
            .expect_err("live yank should require reason before any network call");

        assert!(error
            .to_string()
            .contains("Live module yank requires --reason <text>"));
    }

    #[test]
    fn module_yank_command_requires_slug_and_version() {
        let args = vec!["blog".to_string()];

        let error = module_yank_command(&args)
            .expect_err("yank command without version should fail immediately");

        assert!(error
            .to_string()
            .contains("module yank requires a module slug and version"));
    }

    #[test]
    fn module_yank_command_live_requires_registry_url() {
        let _cwd_guard = WorkspaceRootGuard::enter();
        let _env_guard = EnvVarGuard::set("RUSTOK_MODULE_REGISTRY_URL", None);
        let args = vec![
            "blog".to_string(),
            "1.2.3".to_string(),
            "--reason".to_string(),
            "policy rollback".to_string(),
            "--reason-code".to_string(),
            "rollback".to_string(),
        ];

        let error = module_yank_command(&args)
            .expect_err("live yank should require registry url before any network call");

        assert!(error
            .to_string()
            .contains("Live module yank requires --registry-url"));
    }

    #[test]
    fn module_yank_command_live_requires_reason_code() {
        let _cwd_guard = WorkspaceRootGuard::enter();
        let _env_guard = EnvVarGuard::set("RUSTOK_MODULE_REGISTRY_URL", None);
        let args = vec![
            "blog".to_string(),
            "1.2.3".to_string(),
            "--reason".to_string(),
            "critical regression in production".to_string(),
            "--registry-url".to_string(),
            "http://127.0.0.1:5150".to_string(),
        ];

        let error = module_yank_command(&args)
            .expect_err("live yank should require reason code before any network call");

        assert!(error
            .to_string()
            .contains("Live module yank requires --reason-code"));
        assert!(error
            .to_string()
            .contains(&REGISTRY_YANK_REASON_CODES.join("|")));
    }

    #[test]
    fn module_yank_command_rejects_invalid_semver() {
        let args = vec!["blog".to_string(), "not-a-version".to_string()];

        let error = module_yank_command(&args)
            .expect_err("invalid semver should fail before any manifest lookup");

        assert!(error
            .to_string()
            .contains("module yank version 'not-a-version' is not valid semver"));
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
            Some("rollback".to_string()),
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
        assert_eq!(request_body["reason_code"], "rollback");
    }

    #[test]
    fn build_live_yank_registry_request_turns_off_dry_run_and_keeps_reason_code() {
        let preview = ModuleYankDryRunPreview {
            action: "yank".to_string(),
            slug: "blog".to_string(),
            version: "1.2.3".to_string(),
            crate_name: "rustok-blog".to_string(),
            current_local_version: "1.2.3".to_string(),
            matches_local_version: true,
            package_manifest_path: "crates/rustok-blog/rustok-module.toml".to_string(),
        };

        let request_body = serde_json::to_value(build_live_yank_registry_request(
            &preview,
            Some("Security takedown".to_string()),
            Some("security".to_string()),
        ))
        .expect("live yank request should serialize");

        assert_eq!(request_body["dry_run"], false);
        assert_eq!(request_body["reason_code"], "security");
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
            reason_code: Some("maintenance_handoff".to_string()),
        };

        let request_body = serde_json::to_value(build_owner_transfer_registry_request(
            &preview,
            preview.reason.clone(),
            preview.reason_code.clone(),
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
        assert_eq!(request_body["reason_code"], "maintenance_handoff");
    }

    #[test]
    fn build_live_owner_transfer_registry_request_turns_off_dry_run() {
        let preview = ModuleOwnerTransferDryRunPreview {
            action: "owner_transfer".to_string(),
            slug: "blog".to_string(),
            crate_name: "rustok-blog".to_string(),
            current_local_version: "1.2.3".to_string(),
            package_manifest_path: "crates/rustok-blog/rustok-module.toml".to_string(),
            new_owner_actor: "publisher:comments".to_string(),
            reason: Some("Transfer to the comments publisher".to_string()),
            reason_code: Some("publisher_rotation".to_string()),
        };

        let request_body = serde_json::to_value(build_live_owner_transfer_registry_request(
            &preview,
            preview.reason.clone(),
            preview.reason_code.clone(),
        ))
        .expect("live owner transfer request should serialize");

        assert_eq!(request_body["dry_run"], false);
        assert_eq!(request_body["new_owner_actor"], "publisher:comments");
        assert_eq!(request_body["reason_code"], "publisher_rotation");
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
        assert!(registry_endpoint_uses_loopback(
            "http://[::1]:5150/v2/catalog/publish"
        ));
        assert!(!registry_endpoint_uses_loopback(
            "http://0.0.0.0:5150/v2/catalog/publish"
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
