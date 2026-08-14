use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

fn default_true() -> bool {
    true
}

/// A reviewed design request for a new RusToK module crate.
///
/// This request deliberately creates no domain behavior or transport handler.
/// Requested transports are recorded in the generated design documents and
/// must be implemented through a real owner service before registration.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ScaffoldModuleRequest {
    /// Module slug, for example `newsletter` or `customer-feedback`.
    pub slug: String,
    /// Human-readable module name, for example `Newsletter`.
    pub name: String,
    /// Short purpose statement used in the generated documentation.
    pub description: String,
    /// Runtime module dependencies by slug, for example `["content"]`.
    #[serde(default)]
    pub dependencies: Vec<String>,
    /// Record GraphQL as a required future adapter in the generated design docs.
    #[serde(default = "default_true")]
    pub with_graphql: bool,
    /// Record REST as a required future adapter in the generated design docs.
    #[serde(default = "default_true")]
    pub with_rest: bool,
    /// Direct writes are forbidden; use the reviewed apply operation instead.
    #[serde(default)]
    pub write_files: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ScaffoldModuleFile {
    pub path: String,
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ScaffoldModulePreview {
    pub crate_name: String,
    pub crate_path: String,
    pub files: Vec<ScaffoldModuleFile>,
    pub next_steps: Vec<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ModuleScaffoldDraftStatus {
    Staged,
    Applied,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct StagedModuleScaffold {
    pub draft_id: String,
    pub request: ScaffoldModuleRequest,
    pub preview: ScaffoldModulePreview,
    pub status: ModuleScaffoldDraftStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct StageModuleScaffoldResponse {
    pub draft_id: String,
    pub preview: ScaffoldModulePreview,
    pub status: ModuleScaffoldDraftStatus,
    pub review_required: bool,
    pub apply_tool: String,
    pub next_steps: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ReviewModuleScaffoldRequest {
    pub draft_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ReviewModuleScaffoldResponse {
    pub draft: StagedModuleScaffold,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ApplyModuleScaffoldRequest {
    pub draft_id: String,
    /// Absolute workspace root where `crates/rustok-<slug>` should be written.
    pub workspace_root: String,
    /// Explicit confirmation that the reviewed design scaffold should be written.
    pub confirm: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ApplyModuleScaffoldResponse {
    pub draft_id: String,
    pub crate_name: String,
    pub crate_path: String,
    pub wrote_files: bool,
    pub status: ModuleScaffoldDraftStatus,
    pub next_steps: Vec<String>,
}

pub fn generate_module_scaffold(
    request: &ScaffoldModuleRequest,
) -> Result<ScaffoldModulePreview, String> {
    validate_slug(&request.slug)?;
    validate_name(&request.name)?;
    validate_description(&request.description)?;
    for dependency in &request.dependencies {
        validate_slug(dependency)?;
    }

    if request.write_files {
        return Err(
            "Direct write during alloy_scaffold_module is forbidden. Stage the design, review it, then use alloy_apply_module_scaffold with confirm=true."
                .to_string(),
        );
    }

    let slug = request.slug.trim().to_string();
    let crate_name = format!("rustok-{slug}");
    let module_type = format!("{}Module", pascal_case(&slug));
    let crate_path = format!("crates/{crate_name}");
    let file_map = build_file_map(&slug, &crate_name, &module_type, request);

    Ok(ScaffoldModulePreview {
        crate_name,
        crate_path,
        files: file_map
            .into_iter()
            .map(|(path, content)| ScaffoldModuleFile { path, content })
            .collect(),
        next_steps: preview_next_steps(),
    })
}

pub fn apply_staged_scaffold(
    draft: &StagedModuleScaffold,
    workspace_root: &str,
) -> Result<ApplyModuleScaffoldResponse, String> {
    let target_root = PathBuf::from(workspace_root).join(&draft.preview.crate_path);
    let file_map = draft
        .preview
        .files
        .iter()
        .map(|file| (file.path.clone(), file.content.clone()))
        .collect::<BTreeMap<_, _>>();
    write_scaffold_files(&target_root, &file_map)?;

    Ok(ApplyModuleScaffoldResponse {
        draft_id: draft.draft_id.clone(),
        crate_name: draft.preview.crate_name.clone(),
        crate_path: draft.preview.crate_path.clone(),
        wrote_files: true,
        status: ModuleScaffoldDraftStatus::Applied,
        next_steps: preview_next_steps(),
    })
}

fn preview_next_steps() -> Vec<String> {
    vec![
        "Review the design scaffold before writing it into the workspace.".to_string(),
        "Define module-specific resource and permission constants in rustok-core before runtime registration.".to_string(),
        "Implement the domain model, persistence, migrations, lifecycle, and event contracts through an owner service.".to_string(),
        "Implement requested GraphQL or REST adapters over that owner service before registration.".to_string(),
        "Wire the completed module into apps/server and update docs/modules/registry.md plus docs/index.md.".to_string(),
    ]
}

fn build_file_map(
    slug: &str,
    crate_name: &str,
    module_type: &str,
    request: &ScaffoldModuleRequest,
) -> BTreeMap<String, String> {
    let mut files = BTreeMap::new();
    files.insert("Cargo.toml".to_string(), render_cargo_toml(crate_name));
    files.insert(
        "README.md".to_string(),
        render_readme(crate_name, module_type, request),
    );
    files.insert(
        "CRATE_API.md".to_string(),
        render_crate_api(crate_name, request.with_graphql, request.with_rest),
    );
    files.insert("docs/README.md".to_string(), render_docs_readme(crate_name));
    files.insert(
        "docs/implementation-plan.md".to_string(),
        render_docs_plan(crate_name, request.with_graphql, request.with_rest),
    );
    files.insert(
        "src/lib.rs".to_string(),
        render_lib_rs(
            slug,
            module_type,
            &request.name,
            &request.description,
            &request.dependencies,
        ),
    );
    files.insert("src/contract_tests.rs".to_string(), render_contract_tests());
    files
}

fn write_scaffold_files(root: &Path, file_map: &BTreeMap<String, String>) -> Result<(), String> {
    if root.exists() {
        return Err(format!(
            "Target crate directory already exists: {}",
            root.display()
        ));
    }

    let workspace_root = root
        .parent()
        .and_then(Path::parent)
        .ok_or_else(|| "Failed to resolve workspace root from target path".to_string())?;
    if !workspace_root.join("Cargo.toml").exists() {
        return Err(format!(
            "workspace_root does not look like a RusToK workspace: {}",
            workspace_root.display()
        ));
    }

    for (relative_path, content) in file_map {
        let target_path = root.join(relative_path);
        if let Some(parent) = target_path.parent() {
            fs::create_dir_all(parent).map_err(|error| {
                format!("Failed to create directory {}: {error}", parent.display())
            })?;
        }
        fs::write(&target_path, content)
            .map_err(|error| format!("Failed to write {}: {error}", target_path.display()))?;
    }

    Ok(())
}

fn render_cargo_toml(crate_name: &str) -> String {
    let dependencies = [
        "async-trait.workspace = true",
        "rustok-api.workspace = true",
        "rustok-core.workspace = true",
        "sea-orm-migration.workspace = true",
    ];
    format!(
        "[package]\nname = \"{crate_name}\"\nversion.workspace = true\nedition.workspace = true\nlicense.workspace = true\ndescription.workspace = true\n\n[dependencies]\n{}\n\n[dev-dependencies]\ntokio.workspace = true\n",
        dependencies.join("\n")
    )
}

fn render_readme(crate_name: &str, module_type: &str, request: &ScaffoldModuleRequest) -> String {
    let mut requested_adapters = Vec::new();
    if request.with_graphql {
        requested_adapters.push("GraphQL");
    }
    if request.with_rest {
        requested_adapters.push("REST");
    }
    let adapter_summary = if requested_adapters.is_empty() {
        "No transport adapter is requested yet.".to_string()
    } else {
        format!(
            "The completed module must expose {} through its owner service.",
            requested_adapters.join(" and ")
        )
    };
    let dependency_summary = if request.dependencies.is_empty() {
        "No runtime dependencies are requested.".to_string()
    } else {
        format!(
            "Requested runtime dependencies: {}.",
            request
                .dependencies
                .iter()
                .map(|dependency| format!("`{dependency}`"))
                .collect::<Vec<_>>()
                .join(", ")
        )
    };

    format!(
        "# {crate_name}\n\n## Purpose\n\nThis design scaffold proposes a module that will own {}.\n\n## Current state\n\nThe crate contains metadata and design documentation only. It is not a registered or deployable module.\n\n## Required implementation\n\n- Define the domain model, tenant boundary, and owner-service contract.\n- Add module-specific resources and permissions in `rustok-core`.\n- Implement persistence, migrations, lifecycle/error semantics, and required event effects.\n- {adapter_summary}\n- {dependency_summary}\n\n## Entry point\n\n- `{module_type}` metadata only.\n",
        request.description.trim()
    )
}

fn render_crate_api(crate_name: &str, with_graphql: bool, with_rest: bool) -> String {
    let planned_adapters = match (with_graphql, with_rest) {
        (true, true) => "GraphQL and REST adapters are required before registration.",
        (true, false) => "A GraphQL adapter is required before registration.",
        (false, true) => "A REST adapter is required before registration.",
        (false, false) => "No transport adapter is requested in this design draft.",
    };

    format!(
        "# {crate_name} / CRATE_API\n\n## Public modules\n\n- The design scaffold defines only the module metadata in `src/lib.rs`.\n\n## Core public types and signatures\n\n- `pub struct ...Module`\n- `impl RusToKModule for ...Module`\n\n## Events\n\nNo events are defined by this scaffold.\n\n## Dependencies\n\n- `rustok-core`\n- `rustok-api`\n\n## Design constraints\n\n- This scaffold is not a production module and must not be registered.\n- Tenant boundaries, permission checks, owner services, and persistence contracts must be implemented before composition.\n- {planned_adapters}\n\n## Minimum contract checklist\n\n### Input DTOs and commands\n\nDefine public inputs only with the owner service and its transport adapters.\n\n### Domain invariants\n\nDocument tenant, permission, idempotency, and lifecycle invariants with the domain model.\n\n### Events and outbox effects\n\nPublish cross-module effects through the platform event contract only when the domain behavior exists.\n\n### Errors and refusal codes\n\nDefine stable validation, authorization, conflict, and not-found semantics with the owner transport.\n"
    )
}

fn render_docs_readme(crate_name: &str) -> String {
    format!(
        "# `{crate_name}` Documentation\n\nThis folder records the approved design scaffold. It does not describe a registered or deployable module.\n\n## Contents\n\n- [Implementation plan](./implementation-plan.md)\n"
    )
}

fn render_docs_plan(crate_name: &str, with_graphql: bool, with_rest: bool) -> String {
    let transport_step = match (with_graphql, with_rest) {
        (true, true) => "Implement the GraphQL and REST adapters over the same owner service.",
        (true, false) => "Implement the GraphQL adapter over the owner service.",
        (false, true) => "Implement the REST adapter over the owner service.",
        (false, false) => {
            "Choose and document an external transport only after the owner service exists."
        }
    };
    format!(
        "# `{crate_name}` Implementation Plan\n\n## Scope\n\nThis scaffold records only a proposed module boundary and crate layout. It intentionally contains no domain behavior, persistence, event, permission, or transport implementation.\n\n## Required work before registration\n\n1. Define the domain model, tenant boundary, and owner-service contract.\n2. Add the module-specific permission surface in `rustok-core`.\n3. Implement persistence, migrations, lifecycle/error semantics, and any outbox effects.\n4. {transport_step}\n5. Update `docs/modules/registry.md` and `docs/index.md` with the real runtime wiring.\n"
    )
}

fn render_lib_rs(
    slug: &str,
    module_type: &str,
    name: &str,
    description: &str,
    dependencies: &[String],
) -> String {
    let dependency_list = if dependencies.is_empty() {
        "&[]".to_string()
    } else {
        format!(
            "&[{}]",
            dependencies
                .iter()
                .map(|dependency| rust_string_literal(dependency))
                .collect::<Vec<_>>()
                .join(", ")
        )
    };
    let name_literal = rust_string_literal(name);
    let description_literal = rust_string_literal(description);

    format!(
        "//! Design scaffold for the proposed `{slug}` module.\n//!\n//! This crate cannot be registered until its owner service, permissions,\n//! migrations, and requested transport contracts are implemented.\n\nuse async_trait::async_trait;\nuse rustok_api::Permission;\nuse rustok_core::{{MigrationSource, RusToKModule}};\nuse sea_orm_migration::MigrationTrait;\n\npub struct {module_type};\n\n#[async_trait]\nimpl RusToKModule for {module_type} {{\n    fn slug(&self) -> &'static str {{\n        \"{slug}\"\n    }}\n\n    fn name(&self) -> &'static str {{\n        {name_literal}\n    }}\n\n    fn description(&self) -> &'static str {{\n        {description_literal}\n    }}\n\n    fn version(&self) -> &'static str {{\n        env!(\"CARGO_PKG_VERSION\")\n    }}\n\n    fn dependencies(&self) -> &[&'static str] {{\n        {dependency_list}\n    }}\n\n    fn permissions(&self) -> Vec<Permission> {{\n        Vec::new()\n    }}\n}}\n\nimpl MigrationSource for {module_type} {{\n    fn migrations(&self) -> Vec<Box<dyn MigrationTrait>> {{\n        Vec::new()\n    }}\n}}\n\n#[cfg(test)]\nmod tests {{\n    use super::*;\n\n    #[test]\n    fn module_metadata() {{\n        let module = {module_type};\n        assert_eq!(module.slug(), \"{slug}\");\n        assert_eq!(module.name(), {name_literal});\n        assert_eq!(module.description(), {description_literal});\n    }}\n}}\n\n#[cfg(test)]\nmod contract_tests;\n"
    )
}

fn rust_string_literal(value: &str) -> String {
    serde_json::to_string(value).expect("serializing a Rust string literal cannot fail")
}

fn render_contract_tests() -> String {
    "#[test]\nfn crate_api_defines_minimal_contract_sections() {\n    let api = include_str!(\"../CRATE_API.md\");\n    for marker in [\n        \"## Minimum contract checklist\",\n        \"### Input DTOs and commands\",\n        \"### Domain invariants\",\n        \"### Events and outbox effects\",\n        \"### Errors and refusal codes\",\n    ] {\n        assert!(api.contains(marker), \"CRATE_API.md must contain section: {marker}\");\n    }\n}\n".to_string()
}

fn validate_slug(slug: &str) -> Result<(), String> {
    let slug = slug.trim();
    if slug.is_empty() {
        return Err("slug must not be empty".to_string());
    }
    if slug.len() > 64 {
        return Err("slug must be 64 characters or fewer".to_string());
    }
    if !slug
        .chars()
        .all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '-' || ch == '_')
    {
        return Err(
            "slug must contain only lowercase ASCII letters, digits, '-' or '_'".to_string(),
        );
    }
    Ok(())
}

fn validate_name(name: &str) -> Result<(), String> {
    validate_text(name, "name", 128)
}

fn validate_description(description: &str) -> Result<(), String> {
    validate_text(description, "description", 4_096)
}

fn validate_text(value: &str, field: &str, maximum_len: usize) -> Result<(), String> {
    if value.trim().is_empty() {
        return Err(format!("{field} must not be empty"));
    }
    if value.len() > maximum_len {
        return Err(format!("{field} must be {maximum_len} characters or fewer"));
    }
    if value.chars().any(char::is_control) {
        return Err(format!("{field} must not contain control characters"));
    }
    Ok(())
}

fn pascal_case(value: &str) -> String {
    value
        .split(['-', '_'])
        .filter(|segment| !segment.is_empty())
        .map(|segment| {
            let mut chars = segment.chars();
            match chars.next() {
                Some(first) => first.to_ascii_uppercase().to_string() + chars.as_str(),
                None => String::new(),
            }
        })
        .collect::<String>()
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    fn request(with_graphql: bool, with_rest: bool) -> ScaffoldModuleRequest {
        ScaffoldModuleRequest {
            slug: "newsletter".to_string(),
            name: "Newsletter \"Core\"".to_string(),
            description: "newsletter campaigns and subscriptions".to_string(),
            dependencies: vec!["content".to_string()],
            with_graphql,
            with_rest,
            write_files: false,
        }
    }

    #[test]
    fn preview_records_requested_transports_without_fake_handlers() {
        let response = generate_module_scaffold(&request(true, true))
            .expect("design scaffold should be generated");
        let paths = response
            .files
            .iter()
            .map(|file| file.path.as_str())
            .collect::<Vec<_>>();

        assert!(paths.contains(&"Cargo.toml"));
        assert!(paths.contains(&"src/lib.rs"));
        assert!(paths.contains(&"docs/implementation-plan.md"));
        assert!(!paths.iter().any(|path| path.starts_with("src/graphql/")));
        assert!(
            !paths
                .iter()
                .any(|path| path.starts_with("src/controllers/"))
        );

        let plan = response
            .files
            .iter()
            .find(|file| file.path == "docs/implementation-plan.md")
            .expect("implementation plan should be present");
        assert!(plan.content.contains("GraphQL and REST adapters"));

        let source = response
            .files
            .iter()
            .find(|file| file.path == "src/lib.rs")
            .expect("module source should be present");
        assert!(source.content.contains("Newsletter \\\"Core\\\""));
    }

    #[test]
    fn direct_write_flag_is_rejected_during_staging() {
        let mut request = request(false, false);
        request.write_files = true;
        let error =
            generate_module_scaffold(&request).expect_err("staging must reject direct writes");
        assert!(error.contains("alloy_apply_module_scaffold"));
    }

    #[test]
    fn apply_writes_reviewed_design_files_to_disk() {
        let workspace_root = std::env::temp_dir().join(format!("rustok-mcp-{}", Uuid::new_v4()));
        fs::create_dir_all(workspace_root.join("crates")).expect("workspace crates directory");
        fs::write(workspace_root.join("Cargo.toml"), "[workspace]\n")
            .expect("workspace Cargo.toml");

        let preview = generate_module_scaffold(&request(false, false))
            .expect("design scaffold should be generated");
        let draft = StagedModuleScaffold {
            draft_id: Uuid::new_v4().to_string(),
            request: request(false, false),
            preview: preview.clone(),
            status: ModuleScaffoldDraftStatus::Staged,
        };

        let response = apply_staged_scaffold(&draft, &workspace_root.to_string_lossy())
            .expect("apply should write the reviewed scaffold");
        let crate_root = workspace_root.join(response.crate_path);
        assert!(crate_root.join("Cargo.toml").exists());
        assert!(crate_root.join("README.md").exists());
        assert!(!crate_root.join("src/graphql").exists());
        assert!(!crate_root.join("src/controllers").exists());

        fs::remove_dir_all(workspace_root).expect("temporary workspace should be removable");
    }
}
