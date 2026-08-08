use serde::Deserialize;
use serde_json::json;
use std::collections::{BTreeMap, BTreeSet};
use std::env;
use std::fs;
use std::path::PathBuf;

const MODULE_MANIFEST_RELATIVE_PATH: &str = "../rustok-module.toml";
const GENERATED_FILE: &str = "pages_contribution_manifest.rs";
const OWNER_PROVIDER_METADATA_KEY: &str = "ownerProvider";
const PROVIDER_VERSION_METADATA_KEY: &str = "providerVersion";

#[derive(Debug, Deserialize)]
struct ModuleManifestRoot {
    module: ModuleMetadata,
    fba: FbaMetadata,
}

#[derive(Debug, Deserialize)]
struct ModuleMetadata {
    slug: String,
    version: String,
}

#[derive(Debug, Deserialize)]
struct FbaMetadata {
    builder_consumer: BuilderConsumerMetadata,
}

#[derive(Debug, Deserialize)]
struct BuilderConsumerMetadata {
    capabilities: Vec<String>,
    contribution_manifest: ContributionManifestSource,
}

#[derive(Debug, Deserialize)]
struct ContributionManifestSource {
    owner_provider: String,
    #[serde(default)]
    target_providers: BTreeMap<String, String>,
    #[serde(default)]
    dependencies: BTreeSet<String>,
    #[serde(default)]
    required_permissions: BTreeSet<String>,
    #[serde(default)]
    admin: Vec<toml::Value>,
    #[serde(default)]
    storefront: Vec<toml::Value>,
}

#[derive(Debug, Clone)]
struct ContributionExport {
    id: String,
    provider: String,
    required_capabilities: Vec<String>,
    blocks: Vec<String>,
    property_editor_id: Option<String>,
    property_editor_component_type: Option<String>,
}

fn main() {
    println!("cargo:rerun-if-changed={MODULE_MANIFEST_RELATIVE_PATH}");

    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR"));
    let manifest_path = manifest_dir.join(MODULE_MANIFEST_RELATIVE_PATH);
    let source = fs::read_to_string(&manifest_path)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", manifest_path.display()));
    let root: ModuleManifestRoot = toml::from_str(&source)
        .unwrap_or_else(|error| panic!("invalid {}: {error}", manifest_path.display()));

    let module_id = required(&root.module.slug, "module.slug");
    if module_id != "pages" {
        panic!("Pages admin contribution generator requires module.slug='pages', got '{module_id}'");
    }
    let owner_version = required(&root.module.version, "module.version");
    let cargo_version = env::var("CARGO_PKG_VERSION").expect("CARGO_PKG_VERSION");
    if owner_version != cargo_version {
        panic!(
            "Pages module.version '{owner_version}' must match rustok-pages-admin package version '{cargo_version}'"
        );
    }

    let builder_capabilities = normalize_strings(
        root.fba.builder_consumer.capabilities,
        "fba.builder_consumer.capabilities",
    );
    if builder_capabilities.is_empty() {
        panic!("fba.builder_consumer.capabilities must not be empty");
    }

    let ContributionManifestSource {
        owner_provider,
        target_providers,
        dependencies,
        required_permissions,
        admin,
        storefront,
    } = root.fba.builder_consumer.contribution_manifest;
    let owner_provider = required(&owner_provider, "contribution_manifest.owner_provider");
    let target_providers = normalize_targets(target_providers, &owner_provider, &owner_version);

    let mut exports = BTreeMap::new();
    let admin = normalize_contributions(
        admin,
        "admin",
        &owner_provider,
        &owner_version,
        &target_providers,
        &mut exports,
    );
    let storefront = normalize_contributions(
        storefront,
        "storefront",
        &owner_provider,
        &owner_version,
        &target_providers,
        &mut exports,
    );

    let landing = exports
        .get("landing_blocks")
        .unwrap_or_else(|| panic!("contribution_manifest.admin must declare role='landing_blocks'"));
    let metadata = exports
        .get("metadata")
        .unwrap_or_else(|| panic!("contribution_manifest.admin must declare role='metadata'"));
    if landing.provider == owner_provider {
        panic!("landing_blocks contribution must target a declared external provider");
    }
    if metadata.provider != owner_provider {
        panic!("metadata contribution must target owner provider '{owner_provider}'");
    }
    let fly_builtin_version = target_providers
        .get(&landing.provider)
        .unwrap_or_else(|| panic!("landing_blocks provider '{}' is not version-pinned", landing.provider));
    let metadata_property_editor_id = metadata
        .property_editor_id
        .as_deref()
        .unwrap_or_else(|| panic!("metadata contribution must declare exactly one property editor"));
    let metadata_component_type = metadata
        .property_editor_component_type
        .as_deref()
        .unwrap_or_else(|| panic!("metadata property editor must declare component_type"));

    let manifest_json = json!({
        "module_id": module_id,
        "owner_provider": owner_provider,
        "owner_version": owner_version,
        "target_providers": target_providers,
        "dependencies": dependencies,
        "required_permissions": required_permissions,
        "admin": admin,
        "storefront": storefront,
    });
    let manifest_json = serde_json::to_string(&manifest_json)
        .expect("generated Pages contribution manifest must serialize");

    let mut generated = String::from(
        "// @generated by rustok-pages-admin/build.rs from ../rustok-module.toml.\n// Do not edit by hand.\n\n",
    );
    push_str_const(&mut generated, "PAGES_MODULE_ID", &module_id);
    push_str_const(&mut generated, "PAGES_OWNER_PROVIDER", &owner_provider);
    push_str_const(
        &mut generated,
        "PAGES_OWNER_PROVIDER_VERSION",
        &owner_version,
    );
    push_str_const(&mut generated, "FLY_BUILTIN_PROVIDER", &landing.provider);
    push_str_const(
        &mut generated,
        "FLY_BUILTIN_PROVIDER_VERSION",
        fly_builtin_version,
    );
    push_str_const(
        &mut generated,
        "PAGES_LANDING_BLOCKS_CONTRIBUTION_ID",
        &landing.id,
    );
    push_str_const(
        &mut generated,
        "PAGES_METADATA_CONTRIBUTION_ID",
        &metadata.id,
    );
    push_str_const(
        &mut generated,
        "PAGES_METADATA_PROPERTY_EDITOR_ID",
        metadata_property_editor_id,
    );
    push_str_const(
        &mut generated,
        "PAGES_METADATA_COMPONENT_TYPE",
        metadata_component_type,
    );
    push_str_slice_const(
        &mut generated,
        "PAGES_BUILDER_CAPABILITIES",
        &builder_capabilities,
    );
    push_str_slice_const(
        &mut generated,
        "PAGES_LANDING_BLOCK_CAPABILITIES",
        &landing.required_capabilities,
    );
    push_str_slice_const(
        &mut generated,
        "PAGES_METADATA_CAPABILITIES",
        &metadata.required_capabilities,
    );
    push_str_slice_const(
        &mut generated,
        "PAGES_LANDING_BLOCK_IDS",
        &landing.blocks,
    );
    generated.push_str(&format!(
        "pub const GENERATED_PAGES_CONTRIBUTION_MANIFEST_JSON: &str = {manifest_json:?};\n"
    ));

    let out_dir = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR"));
    fs::write(out_dir.join(GENERATED_FILE), generated)
        .expect("write generated Pages contribution manifest");
}

fn normalize_targets(
    targets: BTreeMap<String, String>,
    owner_provider: &str,
    owner_version: &str,
) -> BTreeMap<String, String> {
    let mut normalized = BTreeMap::new();
    for (provider, version) in targets {
        let provider = required(&provider, "target provider");
        let version = required(&version, "target provider version");
        if provider == owner_provider {
            if version != owner_version {
                panic!(
                    "target provider '{provider}@{version}' conflicts with owner version '{owner_version}'"
                );
            }
            continue;
        }
        if normalized.insert(provider.clone(), version).is_some() {
            panic!("target provider '{provider}' is duplicated after normalization");
        }
    }
    normalized
}

fn normalize_contributions(
    values: Vec<toml::Value>,
    surface: &str,
    owner_provider: &str,
    owner_version: &str,
    target_providers: &BTreeMap<String, String>,
    exports: &mut BTreeMap<String, ContributionExport>,
) -> Vec<serde_json::Value> {
    values
        .into_iter()
        .map(|mut value| {
            let table = value
                .as_table_mut()
                .unwrap_or_else(|| panic!("{surface} contribution must be a TOML table"));
            let role = take_required_string(table, "role", surface);
            let id = table_required_string(table, "id", surface);
            let provider = table_required_string(table, "provider", &id);
            let expected_version = if provider == owner_provider {
                owner_version
            } else {
                target_providers.get(&provider).map(String::as_str).unwrap_or_else(|| {
                    panic!(
                        "contribution '{id}' targets undeclared provider '{provider}'; declare an exact target_providers version"
                    )
                })
            };

            let metadata = table
                .entry("metadata".to_string())
                .or_insert_with(|| toml::Value::Table(Default::default()));
            let metadata = metadata
                .as_table_mut()
                .unwrap_or_else(|| panic!("contribution '{id}' metadata must be a TOML table"));
            if metadata.contains_key(OWNER_PROVIDER_METADATA_KEY)
                || metadata.contains_key(PROVIDER_VERSION_METADATA_KEY)
            {
                panic!(
                    "contribution '{id}' must not hand-author ownerProvider/providerVersion; build generation owns those fields"
                );
            }
            metadata.insert(
                OWNER_PROVIDER_METADATA_KEY.to_string(),
                toml::Value::String(owner_provider.to_string()),
            );
            metadata.insert(
                PROVIDER_VERSION_METADATA_KEY.to_string(),
                toml::Value::String(expected_version.to_string()),
            );

            let required_capabilities = table_string_array(table, "required_capabilities", &id);
            let blocks = table_string_array(table, "blocks", &id);
            let (property_editor_id, property_editor_component_type) =
                property_editor_export(table, &role, &id);

            let export = ContributionExport {
                id: id.clone(),
                provider,
                required_capabilities,
                blocks,
                property_editor_id,
                property_editor_component_type,
            };
            if exports.insert(role.clone(), export).is_some() {
                panic!("contribution role '{role}' is duplicated");
            }

            serde_json::to_value(value)
                .unwrap_or_else(|error| panic!("contribution '{id}' cannot serialize: {error}"))
        })
        .collect()
}

fn property_editor_export(
    table: &toml::Table,
    role: &str,
    contribution_id: &str,
) -> (Option<String>, Option<String>) {
    let editors = table
        .get("property_editors")
        .and_then(toml::Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or(&[]);
    if role != "metadata" {
        return (None, None);
    }
    if editors.len() != 1 {
        panic!("metadata contribution '{contribution_id}' must declare exactly one property editor");
    }
    let editor = editors[0]
        .as_table()
        .unwrap_or_else(|| panic!("metadata property editor must be a TOML table"));
    (
        Some(table_required_string(editor, "id", "metadata property editor")),
        Some(table_required_string(
            editor,
            "component_type",
            "metadata property editor",
        )),
    )
}

fn table_string_array(table: &toml::Table, key: &str, label: &str) -> Vec<String> {
    let Some(value) = table.get(key) else {
        return Vec::new();
    };
    let values = value
        .as_array()
        .unwrap_or_else(|| panic!("{label}.{key} must be an array"));
    normalize_strings(
        values
            .iter()
            .map(|value| {
                value
                    .as_str()
                    .unwrap_or_else(|| panic!("{label}.{key} entries must be strings"))
                    .to_string()
            })
            .collect(),
        &format!("{label}.{key}"),
    )
}

fn normalize_strings(values: Vec<String>, label: &str) -> Vec<String> {
    let mut seen = BTreeSet::new();
    let mut normalized = Vec::with_capacity(values.len());
    for value in values {
        let value = required(&value, label);
        if !seen.insert(value.clone()) {
            panic!("{label} contains duplicate '{value}'");
        }
        normalized.push(value);
    }
    normalized
}

fn take_required_string(table: &mut toml::Table, key: &str, label: &str) -> String {
    let value = table
        .remove(key)
        .unwrap_or_else(|| panic!("{label} contribution is missing '{key}'"));
    required(
        value
            .as_str()
            .unwrap_or_else(|| panic!("{label} contribution '{key}' must be a string")),
        &format!("{label}.{key}"),
    )
}

fn table_required_string(table: &toml::Table, key: &str, label: &str) -> String {
    required(
        table
            .get(key)
            .and_then(toml::Value::as_str)
            .unwrap_or_else(|| panic!("{label} is missing string '{key}'")),
        &format!("{label}.{key}"),
    )
}

fn required(value: &str, label: &str) -> String {
    let value = value.trim();
    if value.is_empty() {
        panic!("{label} must not be empty");
    }
    value.to_string()
}

fn push_str_const(output: &mut String, name: &str, value: &str) {
    output.push_str(&format!("pub const {name}: &str = {value:?};\n"));
}

fn push_str_slice_const(output: &mut String, name: &str, values: &[String]) {
    output.push_str(&format!("pub const {name}: &[&str] = &["));
    for value in values {
        output.push_str(&format!("{value:?},"));
    }
    output.push_str("];\n");
}
