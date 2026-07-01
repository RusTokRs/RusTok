use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ScaffoldModuleRequest {
    /// Module slug, e.g. `newsletter` or `customer-feedback`
    pub slug: String,
    /// Human-readable module name, e.g. `Newsletter`
    pub name: String,
    /// Short purpose statement used in README/docs/lib comments
    pub description: String,
    /// Runtime module dependencies by slug, e.g. `["content"]`
    #[serde(default)]
    pub dependencies: Vec<String>,
    /// Generate GraphQL placeholder entry points
    #[serde(default = "default_true")]
    pub with_graphql: bool,
    /// Generate REST placeholder entry points
    #[serde(default = "default_true")]
    pub with_rest: bool,
    /// Deprecated. Drafts must now be applied via `alloy_apply_module_scaffold`.
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
    /// Explicit human or orchestrator confirmation that the reviewed draft should be written.
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
            "Direct write during alloy_scaffold_module is no longer allowed. Stage the draft, review it, then use alloy_apply_module_scaffold with confirm=true.".to_string(),
        );
    }

    let slug = request.slug.trim().to_string();
    let crate_name = format!("rustok-{}", slug);
    let module_type = format!("{}Module", pascal_case(&slug));
    let query_type = format!("{}Query", pascal_case(&slug));
    let mutation_type = format!("{}Mutation", pascal_case(&slug));
    let route_prefix = format!("api/{}", slug);
    let crate_path = format!("crates/{}", crate_name);

    let file_map = build_file_map(
        &slug,
        &crate_name,
        &module_type,
        &query_type,
        &mutation_type,
        &route_prefix,
        request,
    );

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
        "Review the draft crate before applying it into the workspace.".to_string(),
        "Define module-specific Resource/Permission constants in rustok-core before registering this module in the runtime registry.".to_string(),
        "Wire the new module into apps/server composition root and update docs/modules/registry.md plus docs/index.md.".to_string(),
        "Replace placeholder GraphQL/REST health endpoints with real service boundaries and transport adapters.".to_string(),
    ]
}

fn build_file_map(
    slug: &str,
    crate_name: &str,
    module_type: &str,
    query_type: &str,
    mutation_type: &str,
    route_prefix: &str,
    request: &ScaffoldModuleRequest,
) -> BTreeMap<String, String> {
    let mut files = BTreeMap::new();

    files.insert(
        "Cargo.toml".to_string(),
        render_cargo_toml(crate_name, request.with_graphql, request.with_rest),
    );
    files.insert(
        "README.md".to_string(),
        render_readme(crate_name, module_type, &request.description, request),
    );
    files.insert(
        "CRATE_API.md".to_string(),
        render_crate_api(crate_name, request.with_graphql, request.with_rest),
    );
    files.insert("docs/README.md".to_string(), render_docs_readme(crate_name));
    files.insert(
        "docs/implementation-plan.md".to_string(),
        render_docs_plan(crate_name),
    );
    files.insert(
        "src/lib.rs".to_string(),
        render_lib_rs(
            slug,
            module_type,
            query_type,
            mutation_type,
            &request.name,
            &request.description,
            &request.dependencies,
            request.with_graphql,
            request.with_rest,
        ),
    );
    files.insert("src/contract_tests.rs".to_string(), render_contract_tests());

    if request.with_graphql {
        files.insert(
            "src/graphql/mod.rs".to_string(),
            render_graphql_mod(query_type, mutation_type),
        );
        files.insert(
            "src/graphql/query.rs".to_string(),
            render_graphql_query(slug, query_type),
        );
        files.insert(
            "src/graphql/mutation.rs".to_string(),
            render_graphql_mutation(slug, mutation_type),
        );
    }

    if request.with_rest {
        files.insert(
            "src/controllers/mod.rs".to_string(),
            render_controllers_mod(route_prefix),
        );
    }

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

fn render_cargo_toml(crate_name: &str, with_graphql: bool, with_rest: bool) -> String {
    let mut dependencies = vec![
        "async-trait.workspace = true".to_string(),
        "rustok-core.workspace = true".to_string(),
        "sea-orm-migration.workspace = true".to_string(),
    ];

    if with_graphql {
        dependencies.push("async-graphql.workspace = true".to_string());
    }
    if with_rest {
        dependencies.push("axum.workspace = true".to_string());
        dependencies.push("loco-rs.workspace = true".to_string());
    }

    format!(
        "[package]\nname = \"{crate_name}\"\nversion.workspace = true\nedition.workspace = true\nlicense.workspace = true\ndescription.workspace = true\n\n[dependencies]\n{}\n\n[dev-dependencies]\ntokio.workspace = true\n",
        dependencies.join("\n")
    )
}

fn render_readme(
    crate_name: &str,
    module_type: &str,
    description: &str,
    request: &ScaffoldModuleRequest,
) -> String {
    let mut interactions = vec![
        "- Depends on `rustok-core` for module contracts, permissions, and migration source."
            .to_string(),
    ];

    if request.with_graphql {
        interactions.push(
            "- Exposes module-owned GraphQL placeholders that should be replaced by real adapters."
                .to_string(),
        );
    }
    if request.with_rest {
        interactions.push(
            "- Exposes module-owned REST placeholders that should be replaced by real handlers."
                .to_string(),
        );
    }
    if !request.dependencies.is_empty() {
        interactions.push(format!(
            "- Declares runtime dependencies on: {}.",
            request
                .dependencies
                .iter()
                .map(|dependency| format!("`{dependency}`"))
                .collect::<Vec<_>>()
                .join(", ")
        ));
    }

    let mut entry_points = vec![format!("- `{module_type}`")];
    if request.with_graphql {
        entry_points.push("- `graphql::*`".to_string());
    }
    if request.with_rest {
        entry_points.push("- `controllers::routes`".to_string());
    }

    format!(
        "# {crate_name}\n\n## Purpose\n\n`{crate_name}` owns {description}.\n\n## Responsibilities\n\n- Provide `{module_type}` metadata for the runtime registry.\n- Provide a draft crate skeleton aligned with RusToK module conventions.\n- Reserve the public entry points that future domain services and adapters will fill.\n\n## Interactions\n\n{}\n\n## Entry points\n\n{}\n",
        interactions.join("\n"),
        entry_points.join("\n")
    )
}

fn render_crate_api(crate_name: &str, with_graphql: bool, with_rest: bool) -> String {
    let mut public_modules =
        vec!["`server`-agnostic module core is defined in `src/lib.rs`.".to_string()];
    if with_graphql {
        public_modules.push("`graphql`".to_string());
    }
    if with_rest {
        public_modules.push("`controllers`".to_string());
    }

    format!(
        "# {crate_name} / CRATE_API\n\n## РџСѓР±Р»РёС‡РЅС‹Рµ РјРѕРґСѓР»Рё\n{}\n\n## РћСЃРЅРѕРІРЅС‹Рµ РїСѓР±Р»РёС‡РЅС‹Рµ С‚РёРїС‹ Рё СЃРёРіРЅР°С‚СѓСЂС‹\n- `pub struct ...Module`\n- `impl RusToKModule for ...Module`\n- Draft transport entry points exported from the crate.\n\n## РЎРѕР±С‹С‚РёСЏ\n- РџСѓР±Р»РёРєСѓРµС‚: TBD.\n- РџРѕС‚СЂРµР±Р»СЏРµС‚: TBD.\n\n## Р—Р°РІРёСЃРёРјРѕСЃС‚Рё РѕС‚ РґСЂСѓРіРёС… rustok-РєСЂРµР№С‚РѕРІ\n- `rustok-core`\n\n## Р§Р°СЃС‚С‹Рµ РѕС€РёР±РєРё РР\n- РЎС‡РёС‚Р°С‚СЊ scaffold РіРѕС‚РѕРІС‹Рј production-РјРѕРґСѓР»РµРј.\n- Р РµРіРёСЃС‚СЂРёСЂРѕРІР°С‚СЊ РјРѕРґСѓР»СЊ Р±РµР· СЃРѕР±СЃС‚РІРµРЅРЅРѕРіРѕ permission surface РІ `rustok-core`.\n- РћСЃС‚Р°РІР»СЏС‚СЊ placeholder transport endpoints РєР°Рє С„РёРЅР°Р»СЊРЅСѓСЋ СЂРµР°Р»РёР·Р°С†РёСЋ.\n\n## РњРёРЅРёРјР°Р»СЊРЅС‹Р№ РЅР°Р±РѕСЂ РєРѕРЅС‚СЂР°РєС‚РѕРІ\n\n### Р’С…РѕРґРЅС‹Рµ DTO/РєРѕРјР°РЅРґС‹\n- Р’СЃРµ РІС…РѕРґРЅС‹Рµ DTO РґРѕР»Р¶РЅС‹ С„РёРєСЃРёСЂРѕРІР°С‚СЊСЃСЏ РІ РїСѓР±Р»РёС‡РЅРѕРј crate API РїРѕ РјРµСЂРµ РїРѕСЏРІР»РµРЅРёСЏ СЂРµР°Р»СЊРЅРѕР№ РґРѕРјРµРЅРЅРѕР№ Р»РѕРіРёРєРё.\n- Breaking-change РІ РїСѓР±Р»РёС‡РЅС‹С… DTO С‚СЂРµР±СѓРµС‚ СЃРёРЅС…СЂРѕРЅРЅРѕРіРѕ РѕР±РЅРѕРІР»РµРЅРёСЏ transport-Р°РґР°РїС‚РµСЂРѕРІ.\n\n### Р”РѕРјРµРЅРЅС‹Рµ РёРЅРІР°СЂРёР°РЅС‚С‹\n- РРЅРІР°СЂРёР°РЅС‚С‹ tenant boundary, permission checks Рё service boundaries РѕР±СЏР·Р°С‚РµР»СЊРЅС‹ РґРѕ runtime registration.\n- Placeholder scaffold РЅРµ СЏРІР»СЏРµС‚СЃСЏ Р·Р°РјРµРЅРѕР№ СЂРµР°Р»СЊРЅС‹С… РґРѕРјРµРЅРЅС‹С… РёРЅРІР°СЂРёР°РЅС‚РѕРІ.\n\n### РЎРѕР±С‹С‚РёСЏ / outbox-РїРѕР±РѕС‡РЅС‹Рµ СЌС„С„РµРєС‚С‹\n- Р’СЃРµ РјРµР¶РјРѕРґСѓР»СЊРЅС‹Рµ СЃРѕР±С‹С‚РёСЏ РґРѕР»Р¶РЅС‹ РёРґС‚Рё С‡РµСЂРµР· platform event contracts.\n- Event payloads РґРѕР»Р¶РЅС‹ РѕСЃС‚Р°РІР°С‚СЊСЃСЏ РѕР±СЂР°С‚РЅРѕ-СЃРѕРІРјРµСЃС‚РёРјС‹РјРё РґР»СЏ РїРѕС‚СЂРµР±РёС‚РµР»РµР№.\n\n### РћС€РёР±РєРё / РєРѕРґС‹ РѕС‚РєР°Р·РѕРІ\n- РџСѓР±Р»РёС‡РЅС‹Рµ РѕС€РёР±РєРё РґРѕР»Р¶РЅС‹ СЃРѕС…СЂР°РЅСЏС‚СЊ СЃРµРјР°РЅС‚РёРєСѓ validation/auth/conflict/not-found СЃС†РµРЅР°СЂРёРµРІ.\n- Placeholder handlers РЅРµ РґРѕР»Р¶РЅС‹ СЃС‡РёС‚Р°С‚СЊСЃСЏ С„РёРЅР°Р»СЊРЅС‹Рј error contract.\n",
        public_modules.join(", ")
    )
}

fn render_docs_readme(crate_name: &str) -> String {
    format!(
        "# Р”РѕРєСѓРјРµРЅС‚Р°С†РёСЏ `{crate_name}`\n\nР’ СЌС‚РѕР№ РїР°РїРєРµ С…СЂР°РЅРёС‚СЃСЏ Р»РѕРєР°Р»СЊРЅР°СЏ РґРѕРєСѓРјРµРЅС‚Р°С†РёСЏ draft-РјРѕРґСѓР»СЏ.\n\n## РЎРѕРґРµСЂР¶РёРјРѕРµ\n\n- [РџР»Р°РЅ СЂРµР°Р»РёР·Р°С†РёРё](./implementation-plan.md)\n"
    )
}

fn render_docs_plan(crate_name: &str) -> String {
    format!(
        "# РџР»Р°РЅ СЂРµР°Р»РёР·Р°С†РёРё `{crate_name}`\n\n## Scope\n\nР­С‚РѕС‚ scaffold С„РёРєСЃРёСЂСѓРµС‚ С‚РѕР»СЊРєРѕ СЃС‚Р°СЂС‚РѕРІС‹Р№ РјРѕРґСѓР»СЊРЅС‹Р№ РєР°СЂРєР°СЃ.\n\n## РЎР»РµРґСѓСЋС‰РёРµ С€Р°РіРё\n\n1. РћРїСЂРµРґРµР»РёС‚СЊ РґРѕРјРµРЅРЅСѓСЋ РјРѕРґРµР»СЊ Рё service boundaries.\n2. Р”РѕР±Р°РІРёС‚СЊ СЃРѕР±СЃС‚РІРµРЅРЅС‹Р№ permission surface РІ `rustok-core`.\n3. РџРѕРґРјРµРЅРёС‚СЊ placeholder GraphQL/REST entry points СЂРµР°Р»СЊРЅРѕР№ Р±РёР·РЅРµСЃ-Р»РѕРіРёРєРѕР№.\n4. РћР±РЅРѕРІРёС‚СЊ `docs/modules/registry.md` Рё `docs/index.md` РїРѕСЃР»Рµ runtime wiring.\n"
    )
}

#[allow(clippy::too_many_arguments)]
fn render_lib_rs(
    slug: &str,
    module_type: &str,
    query_type: &str,
    mutation_type: &str,
    name: &str,
    description: &str,
    dependencies: &[String],
    with_graphql: bool,
    with_rest: bool,
) -> String {
    let dependency_list = if dependencies.is_empty() {
        "&[]".to_string()
    } else {
        format!(
            "&[{}]",
            dependencies
                .iter()
                .map(|dependency| format!("\"{dependency}\""))
                .collect::<Vec<_>>()
                .join(", ")
        )
    };

    let mut mod_decls = Vec::new();
    let mut re_exports = Vec::new();
    if with_rest {
        mod_decls.push("pub mod controllers;".to_string());
        re_exports.push("pub use controllers::routes;".to_string());
    }
    if with_graphql {
        mod_decls.push("pub mod graphql;".to_string());
        re_exports.push(format!(
            "pub use graphql::{{{query_type}, {mutation_type}}};"
        ));
    }

    format!(
        "//! Draft RusToK module scaffold for `{slug}`.\n//!\n//! Purpose: {description}\n\n{}\n{}\n\nuse async_trait::async_trait;\nuse rustok_core::{{MigrationSource, RusToKModule}};\nuse rustok_api::Permission;\nuse sea_orm_migration::MigrationTrait;\n\npub struct {module_type};\n\n#[async_trait]\nimpl RusToKModule for {module_type} {{\n    fn slug(&self) -> &'static str {{\n        \"{slug}\"\n    }}\n\n    fn name(&self) -> &'static str {{\n        \"{name}\"\n    }}\n\n    fn description(&self) -> &'static str {{\n        \"{description}\"\n    }}\n\n    fn version(&self) -> &'static str {{\n        env!(\"CARGO_PKG_VERSION\")\n    }}\n\n    fn dependencies(&self) -> &[&'static str] {{\n        {dependency_list}\n    }}\n\n    fn permissions(&self) -> Vec<Permission> {{\n        // TODO: add dedicated module permissions before runtime registration.\n        Vec::new()\n    }}\n}}\n\nimpl MigrationSource for {module_type} {{\n    fn migrations(&self) -> Vec<Box<dyn MigrationTrait>> {{\n        Vec::new()\n    }}\n}}\n\n#[cfg(test)]\nmod tests {{\n    use super::*;\n\n    #[test]\n    fn module_metadata() {{\n        let module = {module_type};\n        assert_eq!(module.slug(), \"{slug}\");\n        assert_eq!(module.name(), \"{name}\");\n        assert_eq!(module.description(), \"{description}\");\n    }}\n}}\n\n#[cfg(test)]\nmod contract_tests;\n",
        if mod_decls.is_empty() {
            String::new()
        } else {
            mod_decls.join("\n")
        },
        if re_exports.is_empty() {
            String::new()
        } else {
            format!("{}\n", re_exports.join("\n"))
        }
    )
}

fn render_contract_tests() -> String {
    "#[test]\nfn crate_api_defines_minimal_contract_sections() {\n    let api = include_str!(\"../CRATE_API.md\");\n    for marker in [\n        \"## РњРёРЅРёРјР°Р»СЊРЅС‹Р№ РЅР°Р±РѕСЂ РєРѕРЅС‚СЂР°РєС‚РѕРІ\",\n        \"### Р’С…РѕРґРЅС‹Рµ DTO/РєРѕРјР°РЅРґС‹\",\n        \"### Р”РѕРјРµРЅРЅС‹Рµ РёРЅРІР°СЂРёР°РЅС‚С‹\",\n        \"### РЎРѕР±С‹С‚РёСЏ / outbox-РїРѕР±РѕС‡РЅС‹Рµ СЌС„С„РµРєС‚С‹\",\n        \"### РћС€РёР±РєРё / РєРѕРґС‹ РѕС‚РєР°Р·РѕРІ\",\n    ] {\n        assert!(api.contains(marker), \"CRATE_API.md must contain section: {marker}\");\n    }\n}\n".to_string()
}

fn render_graphql_mod(query_type: &str, mutation_type: &str) -> String {
    format!(
        "mod mutation;\nmod query;\n\npub use mutation::{mutation_type};\npub use query::{query_type};\n"
    )
}

fn render_graphql_query(slug: &str, query_type: &str) -> String {
    let field_name = snake_case(&format!("{slug}_module_health"));
    format!(
        "use async_graphql::Object;\n\npub struct {query_type};\n\n#[Object]\nimpl {query_type} {{\n    async fn {field_name}(&self) -> &'static str {{\n        \"draft\"\n    }}\n}}\n"
    )
}

fn render_graphql_mutation(slug: &str, mutation_type: &str) -> String {
    let field_name = snake_case(&format!("{slug}_module_ping"));
    format!(
        "use async_graphql::Object;\n\npub struct {mutation_type};\n\n#[Object]\nimpl {mutation_type} {{\n    async fn {field_name}(&self) -> &'static str {{\n        \"draft\"\n    }}\n}}\n"
    )
}

fn render_controllers_mod(route_prefix: &str) -> String {
    format!(
        "use axum::routing::get;\nuse loco_rs::controller::Routes;\n\nasync fn health() -> &'static str {{\n    \"draft\"\n}}\n\npub fn routes() -> Routes {{\n    Routes::new()\n        .prefix(\"{route_prefix}\")\n        .add(\"/health\", get(health))\n}}\n"
    )
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
    if name.trim().is_empty() {
        return Err("name must not be empty".to_string());
    }
    Ok(())
}

fn validate_description(description: &str) -> Result<(), String> {
    if description.trim().is_empty() {
        return Err("description must not be empty".to_string());
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

fn snake_case(value: &str) -> String {
    value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '_' {
                ch.to_ascii_lowercase()
            } else {
                '_'
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    #[test]
    fn preview_includes_expected_core_files() {
        let response = generate_module_scaffold(&ScaffoldModuleRequest {
            slug: "newsletter".to_string(),
            name: "Newsletter".to_string(),
            description: "newsletter campaigns and subscriptions".to_string(),
            dependencies: vec!["content".to_string()],
            with_graphql: true,
            with_rest: true,
            write_files: false,
        })
        .expect("preview scaffold should succeed");

        let paths = response
            .files
            .iter()
            .map(|file| file.path.as_str())
            .collect::<Vec<_>>();

        assert!(paths.contains(&"Cargo.toml"));
        assert!(paths.contains(&"src/lib.rs"));
        assert!(paths.contains(&"src/graphql/query.rs"));
        assert!(paths.contains(&"src/controllers/mod.rs"));
    }

    #[test]
    fn write_flag_is_rejected_during_stage() {
        let error = generate_module_scaffold(&ScaffoldModuleRequest {
            slug: "sample".to_string(),
            name: "Sample".to_string(),
            description: "sample module".to_string(),
            dependencies: Vec::new(),
            with_graphql: false,
            with_rest: false,
            write_files: true,
        })
        .expect_err("staging must reject direct writes");

        assert!(error.contains("alloy_apply_module_scaffold"));
    }

    #[test]
    fn apply_writes_files_to_disk() {
        let workspace_root = std::env::temp_dir().join(format!("rustok-mcp-{}", Uuid::new_v4()));
        fs::create_dir_all(workspace_root.join("crates")).expect("workspace crates dir");
        fs::write(workspace_root.join("Cargo.toml"), "[workspace]\n").expect("workspace cargo");

        let preview = generate_module_scaffold(&ScaffoldModuleRequest {
            slug: "sample".to_string(),
            name: "Sample".to_string(),
            description: "sample module".to_string(),
            dependencies: Vec::new(),
            with_graphql: false,
            with_rest: false,
            write_files: false,
        })
        .expect("preview scaffold should succeed");

        let draft = StagedModuleScaffold {
            draft_id: Uuid::new_v4().to_string(),
            request: ScaffoldModuleRequest {
                slug: "sample".to_string(),
                name: "Sample".to_string(),
                description: "sample module".to_string(),
                dependencies: Vec::new(),
                with_graphql: false,
                with_rest: false,
                write_files: false,
            },
            preview: preview.clone(),
            status: ModuleScaffoldDraftStatus::Staged,
        };

        let response = apply_staged_scaffold(&draft, &workspace_root.to_string_lossy())
            .expect("apply should succeed");

        let crate_root = workspace_root.join(response.crate_path);
        assert!(crate_root.join("Cargo.toml").exists());
        assert!(crate_root.join("README.md").exists());

        let _ = fs::remove_dir_all(workspace_root);
    }
}
