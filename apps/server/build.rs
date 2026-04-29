use serde::Deserialize;
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Deserialize)]
struct ModulesManifest {
    #[serde(default)]
    modules: BTreeMap<String, ModuleSpec>,
}

#[derive(Debug, Deserialize)]
struct ModuleSpec {
    #[serde(rename = "crate")]
    crate_name: String,
    #[serde(default)]
    path: Option<String>,
    #[serde(default)]
    required: bool,
    #[serde(default)]
    entry_type: Option<String>,
    #[serde(default)]
    graphql_query_type: Option<String>,
    #[serde(default)]
    graphql_mutation_type: Option<String>,
    #[serde(default)]
    http_routes_fn: Option<String>,
    #[serde(default)]
    http_webhook_routes_fn: Option<String>,
}

#[derive(Debug)]
struct OptionalModuleEntry {
    feature: String,
    module_expr: Option<String>,
    graphql_query_expr: Option<String>,
    graphql_mutation_expr: Option<String>,
    routes_expr: Option<String>,
    extra_route_exprs: Vec<String>,
}

#[derive(Debug, Deserialize, Default)]
struct ModulePackageManifest {
    #[serde(rename = "crate", default)]
    crate_contract: ModulePackageCrateContract,
    #[serde(default)]
    provides: ModulePackageProvides,
}

#[derive(Debug, Deserialize, Default)]
struct ModulePackageCrateContract {
    #[serde(default)]
    entry_type: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
struct ModulePackageProvides {
    #[serde(default)]
    graphql: Option<ModulePackageGraphqlProvides>,
    #[serde(default)]
    http: Option<ModulePackageHttpProvides>,
}

#[derive(Debug, Deserialize, Default)]
struct ModulePackageGraphqlProvides {
    #[serde(default)]
    query: Option<String>,
    #[serde(default)]
    mutation: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
struct ModulePackageHttpProvides {
    #[serde(default)]
    routes: Option<String>,
    #[serde(default)]
    webhook_routes: Option<String>,
}

fn main() {
    if let Err(error) = generate_module_code() {
        panic!("failed to generate server module code: {error}");
    }
}

fn generate_module_code() -> Result<(), Box<dyn std::error::Error>> {
    let manifest_path = manifest_path();
    println!("cargo:rerun-if-env-changed=RUSTOK_MODULES_MANIFEST");
    println!("cargo:rerun-if-changed={}", manifest_path.display());

    let workspace_root = workspace_root();
    let server_src_root = workspace_root.join("apps").join("server").join("src");
    let modules: ModulesManifest = toml::from_str(&fs::read_to_string(&manifest_path)?)?;
    let mut optional_modules = Vec::new();
    for (slug, spec) in modules.modules {
        if let Some(entry) =
            build_optional_module_entry(&workspace_root, &server_src_root, slug, spec)?
        {
            optional_modules.push(entry);
        }
    }

    let out_dir = PathBuf::from(std::env::var("OUT_DIR")?);
    fs::write(
        out_dir.join("modules_registry_codegen.rs"),
        render_registry_codegen(&optional_modules),
    )?;
    fs::write(
        out_dir.join("graphql_schema_codegen.rs"),
        render_graphql_codegen(&optional_modules),
    )?;
    fs::write(
        out_dir.join("app_routes_codegen.rs"),
        render_routes_codegen(&optional_modules),
    )?;

    Ok(())
}

fn manifest_path() -> PathBuf {
    if let Ok(path) = std::env::var("RUSTOK_MODULES_MANIFEST") {
        let raw = PathBuf::from(path);
        if raw.is_absolute() {
            return raw;
        }
        return workspace_root().join(raw);
    }

    workspace_root().join("modules.toml")
}

fn workspace_root() -> PathBuf {
    PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .map(PathBuf::from)
        .expect("workspace root should be resolvable from apps/server")
}

fn build_optional_module_entry(
    workspace_root: &Path,
    server_src_root: &Path,
    slug: String,
    spec: ModuleSpec,
) -> Result<Option<OptionalModuleEntry>, Box<dyn std::error::Error>> {
    if spec.required {
        return Ok(None);
    }

    let spec = apply_module_package_manifest(workspace_root, spec)?;
    let crate_ident = spec.crate_name.replace('-', "_");
    let type_stem = pascal_case(&slug);
    let feature = format!("mod-{slug}");
    let crate_root = spec.path.as_ref().map(|value| workspace_root.join(value));
    let has_package_manifest = crate_root
        .as_ref()
        .map(|root| root.join("rustok-module.toml").exists())
        .unwrap_or(false);
    let lib_path = crate_root
        .as_ref()
        .map(|root| root.join("src").join("lib.rs"));
    if let Some(path) = lib_path.as_ref().filter(|path| path.exists()) {
        println!("cargo:rerun-if-changed={}", path.display());
    }

    let module_expr = spec
        .entry_type
        .clone()
        .or_else(|| infer_runtime_module_expr(lib_path.as_deref(), &spec.crate_name))
        .or_else(|| {
            if has_package_manifest {
                None
            } else {
                Some(format!("{crate_ident}::{type_stem}Module"))
            }
        });

    let graphql_query_expr = spec.graphql_query_type.clone().or_else(|| {
        crate_root
            .as_ref()
            .filter(|root| has_any(root, &["src/graphql/mod.rs", "src/graphql.rs"]))
            .map(|_| format!("{crate_ident}::graphql::{type_stem}Query"))
    });
    let graphql_mutation_expr = spec.graphql_mutation_type.clone().or_else(|| {
        crate_root
            .as_ref()
            .filter(|root| has_any(root, &["src/graphql/mod.rs", "src/graphql.rs"]))
            .map(|_| format!("{crate_ident}::graphql::{type_stem}Mutation"))
    });

    let routes_expr = spec
        .http_routes_fn
        .clone()
        .map(|value| format!("{value}()"))
        .or_else(|| {
            crate_root
                .as_ref()
                .filter(|root| has_any(root, &["src/controllers/mod.rs", "src/controllers.rs"]))
                .map(|_| route_expression(server_src_root, &crate_ident, &slug))
        });

    let mut extra_route_exprs = Vec::new();
    if let Some(webhook_routes_fn) = spec.http_webhook_routes_fn.clone() {
        extra_route_exprs.push(format!("{webhook_routes_fn}()"));
    } else if let Some(root) = crate_root.as_ref() {
        let crate_controller_mod =
            first_existing(root, &["src/controllers/mod.rs", "src/controllers.rs"]);
        let server_controller_mod = first_existing(
            server_src_root,
            &[
                &format!("controllers/{slug}/mod.rs"),
                &format!("controllers/{slug}.rs"),
            ],
        );

        if file_contains_any(&crate_controller_mod, "pub fn webhook_routes")
            || file_contains_any(&server_controller_mod, "pub fn webhook_routes")
        {
            extra_route_exprs.push(webhook_route_expression(
                server_src_root,
                &crate_ident,
                &slug,
            ));
        }
    }

    Ok(Some(OptionalModuleEntry {
        feature,
        module_expr,
        graphql_query_expr,
        graphql_mutation_expr,
        routes_expr,
        extra_route_exprs,
    }))
}

fn route_expression(server_src_root: &Path, crate_ident: &str, slug: &str) -> String {
    let server_mod = first_existing(
        server_src_root,
        &[
            &format!("controllers/{slug}/mod.rs"),
            &format!("controllers/{slug}.rs"),
        ],
    );
    if server_mod.is_some() {
        format!("crate::controllers::{slug}::routes()")
    } else {
        format!("{crate_ident}::controllers::routes()")
    }
}

fn webhook_route_expression(server_src_root: &Path, crate_ident: &str, slug: &str) -> String {
    let server_mod = first_existing(
        server_src_root,
        &[
            &format!("controllers/{slug}/mod.rs"),
            &format!("controllers/{slug}.rs"),
        ],
    );
    if server_mod.is_some() {
        format!("crate::controllers::{slug}::webhook_routes()")
    } else {
        format!("{crate_ident}::controllers::webhook_routes()")
    }
}

fn apply_module_package_manifest(
    workspace_root: &Path,
    mut spec: ModuleSpec,
) -> Result<ModuleSpec, Box<dyn std::error::Error>> {
    let Some(module_path) = spec.path.as_ref() else {
        return Ok(spec);
    };
    let package_manifest_path = workspace_root.join(module_path).join("rustok-module.toml");
    if !package_manifest_path.exists() {
        return Ok(spec);
    }

    println!("cargo:rerun-if-changed={}", package_manifest_path.display());
    let raw = fs::read_to_string(&package_manifest_path)?;
    let package_manifest = toml::from_str::<ModulePackageManifest>(&raw)?;

    if let Some(entry_type) = qualify_package_type_path(
        &spec.crate_name,
        package_manifest.crate_contract.entry_type.as_deref(),
    ) {
        spec.entry_type = Some(entry_type);
    }
    if let Some(graphql) = package_manifest.provides.graphql {
        if let Some(query_type) =
            qualify_package_type_path(&spec.crate_name, graphql.query.as_deref())
        {
            spec.graphql_query_type = Some(query_type);
        }
        if let Some(mutation_type) =
            qualify_package_type_path(&spec.crate_name, graphql.mutation.as_deref())
        {
            spec.graphql_mutation_type = Some(mutation_type);
        }
    }
    if let Some(http) = package_manifest.provides.http {
        if let Some(routes_fn) = qualify_package_type_path(&spec.crate_name, http.routes.as_deref())
        {
            spec.http_routes_fn = Some(routes_fn);
        }
        if let Some(webhook_routes_fn) =
            qualify_package_type_path(&spec.crate_name, http.webhook_routes.as_deref())
        {
            spec.http_webhook_routes_fn = Some(webhook_routes_fn);
        }
    }

    Ok(spec)
}

fn qualify_package_type_path(crate_name: &str, value: Option<&str>) -> Option<String> {
    let value = value?.trim();
    if value.is_empty() {
        return None;
    }

    let crate_ident = crate_name.replace('-', "_");
    let relative = value.strip_prefix("crate::").unwrap_or(value);
    Some(format!("{crate_ident}::{relative}"))
}

fn infer_runtime_module_expr(lib_path: Option<&Path>, crate_name: &str) -> Option<String> {
    let lib_path = lib_path?;
    let content = fs::read_to_string(lib_path).ok()?;
    let marker = "impl RusToKModule for ";
    let start = content.find(marker)? + marker.len();
    let ident: String = content[start..]
        .chars()
        .take_while(|ch| ch.is_ascii_alphanumeric() || *ch == '_')
        .collect();
    if ident.is_empty() {
        return None;
    }

    let crate_ident = crate_name.replace('-', "_");
    Some(format!("{crate_ident}::{ident}"))
}

fn render_registry_codegen(entries: &[OptionalModuleEntry]) -> String {
    let mut out = String::from("#[allow(unused_mut)]\npub fn register_optional_modules(mut registry: rustok_core::ModuleRegistry) -> rustok_core::ModuleRegistry {\n");
    for entry in entries {
        if let Some(module_expr) = &entry.module_expr {
            out.push_str(&format!(
                "    #[cfg(feature = \"{feature}\")]\n    {{\n        registry = registry.register({module_expr});\n    }}\n",
                feature = entry.feature,
                module_expr = module_expr,
            ));
        }
    }
    out.push_str("    registry\n}\n");
    out
}

fn render_graphql_codegen(entries: &[OptionalModuleEntry]) -> String {
    let query_entries = entries
        .iter()
        .filter_map(|entry| {
            entry
                .graphql_query_expr
                .as_ref()
                .map(|expr| (&entry.feature, expr))
        })
        .collect::<Vec<_>>();
    let mutation_entries = entries
        .iter()
        .filter_map(|entry| {
            entry
                .graphql_mutation_expr
                .as_ref()
                .map(|expr| (&entry.feature, expr))
        })
        .collect::<Vec<_>>();
    let mut out = String::new();
    out.push_str("use async_graphql::MergedObject;\n\n");

    if query_entries.is_empty() {
        out.push_str("#[derive(MergedObject, Default)]\npub struct OptionalModuleQuery();\n\n");
    } else {
        out.push_str("#[derive(MergedObject, Default)]\npub struct OptionalModuleQuery(\n");
        for (feature, expr) in &query_entries {
            out.push_str(&format!(
                "    #[cfg(feature = \"{feature}\")] {expr},\n",
                feature = feature,
                expr = expr,
            ));
        }
        out.push_str(");\n\n");
    }

    if mutation_entries.is_empty() {
        out.push_str("#[derive(MergedObject, Default)]\npub struct OptionalModuleMutation();\n");
    } else {
        out.push_str("#[derive(MergedObject, Default)]\npub struct OptionalModuleMutation(\n");
        for (feature, expr) in &mutation_entries {
            out.push_str(&format!(
                "    #[cfg(feature = \"{feature}\")] {expr},\n",
                feature = feature,
                expr = expr,
            ));
        }
        out.push_str(");\n");
    }
    out
}

fn render_routes_codegen(entries: &[OptionalModuleEntry]) -> String {
    let mut out = String::from(
        "#[allow(unused_mut)]\npub fn append_optional_module_routes(mut routes: loco_rs::controller::AppRoutes) -> loco_rs::controller::AppRoutes {\n",
    );
    for entry in entries {
        if let Some(routes_expr) = &entry.routes_expr {
            out.push_str(&format!(
                "    #[cfg(feature = \"{feature}\")]\n    {{\n        routes = routes.add_route({routes_expr});\n    }}\n",
                feature = entry.feature,
                routes_expr = routes_expr,
            ));
        }
        for extra_route_expr in &entry.extra_route_exprs {
            out.push_str(&format!(
                "    #[cfg(feature = \"{feature}\")]\n    {{\n        routes = routes.add_route({route_expr});\n    }}\n",
                feature = entry.feature,
                route_expr = extra_route_expr,
            ));
        }
    }
    out.push_str("    routes\n}\n");
    out
}

fn pascal_case(value: &str) -> String {
    value
        .split(['-', '_'])
        .filter(|part| !part.is_empty())
        .map(|part| {
            let mut chars = part.chars();
            match chars.next() {
                Some(first) => format!("{}{}", first.to_ascii_uppercase(), chars.as_str()),
                None => String::new(),
            }
        })
        .collect()
}

fn has_any(root: &Path, candidates: &[&str]) -> bool {
    candidates
        .iter()
        .any(|candidate| root.join(candidate).exists())
}

fn first_existing(root: &Path, candidates: &[&str]) -> Option<PathBuf> {
    candidates
        .iter()
        .map(|candidate| root.join(candidate))
        .find(|path| path.exists())
}

fn file_contains_any(path: &Option<PathBuf>, needle: &str) -> bool {
    path.as_ref()
        .and_then(|path| fs::read_to_string(path).ok())
        .map(|content| content.contains(needle))
        .unwrap_or(false)
}
