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
    graphql_subscription_type: Option<String>,
    #[serde(default)]
    graphql_runtime_data_factory: Option<String>,
    #[serde(default)]
    http_axum_router_fn: Option<String>,
    #[serde(default)]
    http_axum_webhook_router_fn: Option<String>,
}

#[derive(Debug)]
struct OptionalModuleEntry {
    feature: String,
    module_expr: Option<String>,
    graphql_query_expr: Option<String>,
    graphql_mutation_expr: Option<String>,
    graphql_subscription_expr: Option<String>,
    graphql_runtime_data_factory_expr: Option<String>,
    axum_router_expr: Option<String>,
    axum_webhook_router_expr: Option<String>,
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
    #[serde(default)]
    subscription: Option<String>,
    #[serde(default)]
    runtime_data_factory: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
struct ModulePackageHttpProvides {
    #[serde(default)]
    axum_router: Option<String>,
    #[serde(default)]
    axum_webhook_router: Option<String>,
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
    let modules: ModulesManifest = toml::from_str(&fs::read_to_string(&manifest_path)?)?;
    let mut optional_modules = Vec::new();
    for (slug, spec) in modules.modules {
        if let Some(entry) = build_optional_module_entry(&workspace_root, slug, spec)? {
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
            .filter(|root| {
                !has_package_manifest && has_any(root, &["src/graphql/mod.rs", "src/graphql.rs"])
            })
            .map(|_| format!("{crate_ident}::graphql::{type_stem}Query"))
    });
    let graphql_mutation_expr = spec.graphql_mutation_type.clone().or_else(|| {
        crate_root
            .as_ref()
            .filter(|root| {
                !has_package_manifest && has_any(root, &["src/graphql/mod.rs", "src/graphql.rs"])
            })
            .map(|_| format!("{crate_ident}::graphql::{type_stem}Mutation"))
    });
    let graphql_subscription_expr = spec.graphql_subscription_type.clone();
    let graphql_runtime_data_factory_expr = spec.graphql_runtime_data_factory.clone();

    let axum_router_expr = spec
        .http_axum_router_fn
        .clone()
        .map(|value| format!("{value}(runtime)"));
    let axum_webhook_router_expr = spec
        .http_axum_webhook_router_fn
        .clone()
        .map(|value| format!("{value}(runtime)"));

    Ok(Some(OptionalModuleEntry {
        feature,
        module_expr,
        graphql_query_expr,
        graphql_mutation_expr,
        graphql_subscription_expr,
        graphql_runtime_data_factory_expr,
        axum_router_expr,
        axum_webhook_router_expr,
    }))
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
        if let Some(subscription_type) =
            qualify_package_type_path(&spec.crate_name, graphql.subscription.as_deref())
        {
            spec.graphql_subscription_type = Some(subscription_type);
        }
        if let Some(factory) =
            qualify_package_type_path(&spec.crate_name, graphql.runtime_data_factory.as_deref())
        {
            spec.graphql_runtime_data_factory = Some(factory);
        }
    }
    if let Some(http) = package_manifest.provides.http {
        if let Some(axum_router_fn) =
            qualify_package_type_path(&spec.crate_name, http.axum_router.as_deref())
        {
            spec.http_axum_router_fn = Some(axum_router_fn);
        }
        if let Some(axum_webhook_router_fn) =
            qualify_package_type_path(&spec.crate_name, http.axum_webhook_router.as_deref())
        {
            spec.http_axum_webhook_router_fn = Some(axum_webhook_router_fn);
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
    let mut out = String::from(
        "#[allow(unused_mut)]\npub fn register_optional_modules(mut registry: rustok_core::ModuleRegistry) -> rustok_core::ModuleRegistry {\n",
    );
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
    let subscription_entries = entries
        .iter()
        .filter_map(|entry| {
            entry
                .graphql_subscription_expr
                .as_ref()
                .map(|expr| (&entry.feature, expr))
        })
        .collect::<Vec<_>>();
    let runtime_data_factories = entries
        .iter()
        .filter_map(|entry| entry.graphql_runtime_data_factory_expr.as_ref())
        .collect::<Vec<_>>();
    let mut out = String::new();
    out.push_str("use async_graphql::{MergedObject, MergedSubscription};\n\n");

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
    if subscription_entries.is_empty() {
        out.push_str(
            "\n#[derive(MergedSubscription, Default)]\npub struct OptionalModuleSubscription();\n",
        );
    } else {
        out.push_str(
            "\n#[derive(MergedSubscription, Default)]\npub struct OptionalModuleSubscription(\n",
        );
        for (feature, expr) in &subscription_entries {
            out.push_str(&format!(
                "    #[cfg(feature = \"{feature}\")] {expr},\n",
                feature = feature,
                expr = expr,
            ));
        }
        out.push_str(");\n");
    }
    out.push_str("\n/// Runtime-data factories declared by installed capability manifests.\n");
    out.push_str("pub const MODULE_GRAPHQL_RUNTIME_DATA_FACTORIES: &[&str] = &[\n");
    for factory in runtime_data_factories {
        out.push_str(&format!("    \"{factory}\",\n"));
    }
    out.push_str("];\n");
    out.push_str("\npub fn attach_module_graphql_data(\n    mut builder: async_graphql::SchemaBuilder<super::Query, super::Mutation, super::Subscription>,\n    inputs: &rustok_api::graphql::GraphqlRuntimeInputs,\n) -> Result<async_graphql::SchemaBuilder<super::Query, super::Mutation, super::Subscription>, String> {\n");
    for entry in entries {
        if let Some(factory) = &entry.graphql_runtime_data_factory_expr {
            out.push_str(&format!(
                "    #[cfg(feature = \"{feature}\")]\n    {{\n        builder = builder.data({factory}(inputs)?);\n    }}\n",
                feature = entry.feature,
                factory = factory,
            ));
        }
    }
    out.push_str("    Ok(builder)\n}\n");
    out
}

fn render_routes_codegen(entries: &[OptionalModuleEntry]) -> String {
    let mut out = String::from(
        "#[allow(unused_mut, unused_variables)]\npub fn append_optional_module_axum_routers(\n    mut router: axum::Router,\n    runtime: &rustok_api::HostRuntimeContext,\n) -> anyhow::Result<axum::Router> {\n",
    );
    for entry in entries {
        if let Some(axum_router_expr) = &entry.axum_router_expr {
            out.push_str(&format!(
                "    #[cfg(feature = \"{feature}\")]\n    {{\n        router = router.merge({axum_router_expr}?);\n    }}\n",
                feature = entry.feature,
                axum_router_expr = axum_router_expr,
            ));
        }
        if let Some(axum_webhook_router_expr) = &entry.axum_webhook_router_expr {
            out.push_str(&format!(
                "    #[cfg(feature = \"{feature}\")]\n    {{\n        router = router.merge({axum_webhook_router_expr}?);\n    }}\n",
                feature = entry.feature,
                axum_webhook_router_expr = axum_webhook_router_expr,
            ));
        }
    }
    out.push_str("    Ok(router)\n}\n");
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
