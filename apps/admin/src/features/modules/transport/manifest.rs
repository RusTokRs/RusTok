use leptos::prelude::ServerFnError;
use serde::{Deserialize, Serialize};

use crate::entities::module::MarketplaceModule;
#[allow(unused_imports)]
use crate::entities::module::model::{
    MarketplaceModuleVersion, RegistryFollowUpGateLifecycle, RegistryGovernanceActionLifecycle,
    RegistryGovernanceEventLifecycle, RegistryGovernanceEventPayloadLifecycle,
    RegistryModuleLifecycle, RegistryOwnerLifecycle, RegistryPublishRequestLifecycle,
    RegistryReleaseLifecycle, RegistryValidationStageLifecycle,
    registry_principal_label_from_value,
};

#[cfg(feature = "ssr")]
use super::native_server_adapter::*;

#[cfg(feature = "ssr")]
#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct RuntimeModulesManifest {
    #[serde(default)]
    pub schema: u32,
    #[serde(default)]
    pub app: String,
    #[serde(default)]
    pub build: RuntimeBuildConfig,
    #[serde(default)]
    pub modules: std::collections::HashMap<String, RuntimeManifestModuleSpec>,
    #[serde(default)]
    pub settings: RuntimeSettingsManifest,
}

#[cfg(feature = "ssr")]
#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct RuntimeBuildConfig {
    #[serde(default)]
    pub target: String,
    #[serde(default)]
    pub profile: String,
    #[serde(default)]
    pub server: RuntimeServerBuildConfig,
    #[serde(default)]
    pub admin: RuntimeAdminBuildConfig,
    #[serde(default)]
    pub storefront: Vec<RuntimeStorefrontBuildConfig>,
}

#[cfg(feature = "ssr")]
#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct RuntimeServerBuildConfig {
    #[serde(default)]
    pub embed_admin: bool,
    #[serde(default)]
    pub embed_storefront: bool,
}

#[cfg(feature = "ssr")]
#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct RuntimeAdminBuildConfig {
    #[serde(default)]
    pub stack: String,
    #[serde(default)]
    pub public_url: String,
    #[serde(default)]
    pub redirect_uris: Vec<String>,
}

#[cfg(feature = "ssr")]
#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct RuntimeStorefrontBuildConfig {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub stack: String,
    #[serde(default)]
    pub public_url: String,
    #[serde(default)]
    pub redirect_uris: Vec<String>,
}

#[cfg(feature = "ssr")]
#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct RuntimeSettingsManifest {
    #[serde(default)]
    pub default_enabled: Vec<String>,
}

#[cfg(feature = "ssr")]
#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct RuntimeManifestModuleSpec {
    pub source: String,
    #[serde(rename = "crate", default)]
    pub crate_name: String,
    #[serde(default)]
    pub path: Option<String>,
    #[serde(default)]
    pub version: Option<String>,
    #[serde(default)]
    pub git: Option<String>,
    #[serde(default)]
    pub rev: Option<String>,
    #[serde(default)]
    pub required: bool,
    #[serde(default)]
    pub depends_on: Vec<String>,
}

#[cfg(feature = "ssr")]
#[derive(Debug, Deserialize, Default)]
pub struct RuntimeModulePackageManifest {
    pub module: RuntimeModuleMetadata,
    #[serde(default)]
    pub marketplace: RuntimeModuleMarketplaceMetadata,
    #[serde(default)]
    pub dependencies: std::collections::BTreeMap<String, RuntimeModuleDependencySpec>,
    #[serde(default)]
    pub settings: std::collections::BTreeMap<String, RuntimeModuleSettingSpec>,
}

#[cfg(feature = "ssr")]
#[derive(Debug, Deserialize, Default)]
pub struct RuntimeModuleMetadata {
    pub slug: String,
    pub name: String,
    pub version: String,
    #[serde(default)]
    pub description: String,
    #[serde(default = "default_module_ownership")]
    pub ownership: String,
    #[serde(default = "default_module_trust_level")]
    pub trust_level: String,
    #[serde(default)]
    pub recommended_admin_surfaces: Vec<String>,
    #[serde(default)]
    pub showcase_admin_surfaces: Vec<String>,
    #[serde(default)]
    pub rustok_min_version: Option<String>,
    #[serde(default)]
    pub rustok_max_version: Option<String>,
}

#[cfg(feature = "ssr")]
#[derive(Debug, Deserialize, Default)]
pub struct RuntimeModuleMarketplaceMetadata {
    #[serde(default)]
    pub category: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub icon_url: Option<String>,
    #[serde(default)]
    pub banner_url: Option<String>,
    #[serde(default)]
    pub screenshots: Vec<String>,
    #[serde(default)]
    pub publisher: Option<String>,
    #[serde(default)]
    pub checksum_sha256: Option<String>,
    #[serde(default)]
    pub signature: Option<String>,
}

#[cfg(feature = "ssr")]
#[derive(Debug, Deserialize, Default)]
pub struct RuntimeModuleDependencySpec {
    #[allow(dead_code)]
    #[serde(default)]
    pub version_req: Option<String>,
}

#[cfg(feature = "ssr")]
#[derive(Debug, Deserialize, Serialize, Default)]
pub struct RuntimeModuleSettingSpec {
    #[serde(rename = "type", default)]
    pub value_type: String,
    #[serde(default)]
    pub required: bool,
    #[serde(default)]
    pub default: Option<serde_json::Value>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub min: Option<f64>,
    #[serde(default)]
    pub max: Option<f64>,
    #[serde(default)]
    pub options: Vec<serde_json::Value>,
    #[serde(default)]
    pub object_keys: Vec<String>,
    #[serde(default)]
    pub item_type: Option<String>,
    #[serde(default)]
    pub properties: std::collections::BTreeMap<String, RuntimeModuleSettingSpec>,
    #[serde(default)]
    pub items: Option<Box<RuntimeModuleSettingSpec>>,
}

#[cfg(feature = "ssr")]
pub fn runtime_setting_shape(spec: &RuntimeModuleSettingSpec) -> Option<serde_json::Value> {
    let mut shape = serde_json::Map::new();

    if !spec.properties.is_empty() {
        let properties = spec
            .properties
            .iter()
            .map(|(key, property_spec)| {
                (
                    key.clone(),
                    serde_json::to_value(property_spec)
                        .expect("runtime setting property schema should serialize"),
                )
            })
            .collect::<serde_json::Map<String, serde_json::Value>>();
        shape.insert(
            "properties".to_string(),
            serde_json::Value::Object(properties),
        );
    }

    if let Some(items) = &spec.items {
        shape.insert(
            "items".to_string(),
            serde_json::to_value(items.as_ref())
                .expect("runtime setting item schema should serialize"),
        );
    }

    (!shape.is_empty()).then_some(serde_json::Value::Object(shape))
}

#[cfg(feature = "ssr")]
pub fn runtime_setting_object_keys(spec: &RuntimeModuleSettingSpec) -> Vec<String> {
    if spec.properties.is_empty() {
        spec.object_keys.clone()
    } else {
        spec.properties.keys().cloned().collect()
    }
}

#[cfg(feature = "ssr")]
pub fn runtime_setting_item_type(spec: &RuntimeModuleSettingSpec) -> Option<String> {
    spec.items
        .as_deref()
        .map(|item| item.value_type.trim().to_string())
        .filter(|value| !value.is_empty())
        .or_else(|| spec.item_type.clone())
}

#[cfg(feature = "ssr")]
#[derive(Debug, Deserialize, Default)]
pub struct RuntimeCargoManifest {
    pub package: RuntimeCargoPackage,
}

#[cfg(feature = "ssr")]
#[derive(Debug, Deserialize, Default)]
pub struct RuntimeCargoPackage {
    pub name: String,
}

#[cfg(feature = "ssr")]
#[allow(dead_code)]
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeFrontendBuildTool {
    Cargo,
    Trunk,
}

#[cfg(feature = "ssr")]
#[allow(dead_code)]
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeFrontendArtifactKind {
    File,
    Directory,
}

#[cfg(feature = "ssr")]
#[allow(dead_code)]
#[derive(Debug, Clone, Serialize)]
pub struct RuntimeFrontendBuildPlan {
    pub surface: String,
    pub tool: RuntimeFrontendBuildTool,
    pub package: String,
    pub workspace_path: String,
    pub profile: String,
    pub target: Option<String>,
    pub artifact_path: String,
    pub artifact_kind: RuntimeFrontendArtifactKind,
    pub command: String,
}

#[cfg(feature = "ssr")]
#[allow(dead_code)]
#[derive(Debug, Clone, Serialize)]
pub struct RuntimeBuildExecutionPlan {
    pub cargo_package: String,
    pub cargo_profile: String,
    pub cargo_target: Option<String>,
    pub cargo_features: Vec<String>,
    pub cargo_command: String,
    pub admin_build: Option<RuntimeFrontendBuildPlan>,
    pub storefront_build: Option<RuntimeFrontendBuildPlan>,
}

#[cfg(feature = "ssr")]
#[derive(Debug, Clone)]
pub struct RuntimePlatformSnapshot {
    #[allow(dead_code)]
    pub revision: i64,
    pub manifest: RuntimeModulesManifest,
}

#[cfg(feature = "ssr")]
pub fn bootstrap_runtime_modules_manifest() -> Result<RuntimeModulesManifest, ServerFnError> {
    let raw = include_str!("../../../../../../modules.toml");
    toml::from_str(raw)
        .map_err(|err| server_error(format!("failed to parse embedded modules.toml: {err}")))
}

#[cfg(feature = "ssr")]
pub fn runtime_workspace_root() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../")
}

#[cfg(feature = "ssr")]
pub fn load_toml_file<T: serde::de::DeserializeOwned>(
    path: &std::path::Path,
) -> Result<T, ServerFnError> {
    let raw = std::fs::read_to_string(path)
        .map_err(|err| server_error(format!("failed to read {}: {err}", path.display())))?;
    toml::from_str(&raw)
        .map_err(|err| server_error(format!("failed to parse {}: {err}", path.display())))
}

#[cfg(feature = "ssr")]
#[allow(dead_code)]
pub fn runtime_binary_output_dir_name(profile: &str) -> &str {
    if profile == "release" {
        "release"
    } else {
        profile
    }
}

#[cfg(feature = "ssr")]
#[allow(dead_code)]
pub fn runtime_executable_suffix(target: Option<&str>) -> &'static str {
    match target {
        Some(value) if value.contains("windows") => "exe",
        Some(_) => "",
        None => std::env::consts::EXE_EXTENSION,
    }
}

#[cfg(feature = "ssr")]
#[allow(dead_code)]
pub fn runtime_binary_file_name(package: &str, target: Option<&str>) -> String {
    let suffix = runtime_executable_suffix(target);
    if suffix.is_empty() {
        package.to_string()
    } else {
        format!("{package}.{suffix}")
    }
}

#[cfg(feature = "ssr")]
#[allow(dead_code)]
pub fn runtime_admin_frontend_build_plan(
    manifest: &RuntimeModulesManifest,
    cargo_profile: &str,
) -> Option<RuntimeFrontendBuildPlan> {
    let admin_stack = manifest.build.admin.stack.trim().to_ascii_lowercase();
    let requires_leptos_admin = manifest.build.server.embed_admin || admin_stack == "leptos";

    requires_leptos_admin.then(|| {
        let mut command_parts = vec!["trunk".to_string(), "build".to_string()];
        if cargo_profile == "release" {
            command_parts.push("--release".to_string());
        }

        RuntimeFrontendBuildPlan {
            surface: "admin".to_string(),
            tool: RuntimeFrontendBuildTool::Trunk,
            package: "rustok-admin".to_string(),
            workspace_path: "apps/admin".to_string(),
            profile: cargo_profile.to_string(),
            target: None,
            artifact_path: "apps/admin/dist".to_string(),
            artifact_kind: RuntimeFrontendArtifactKind::Directory,
            command: command_parts.join(" "),
        }
    })
}

#[cfg(feature = "ssr")]
#[allow(dead_code)]
pub fn runtime_storefront_frontend_build_plan(
    manifest: &RuntimeModulesManifest,
    cargo_profile: &str,
    cargo_target: Option<&str>,
) -> Option<RuntimeFrontendBuildPlan> {
    let has_leptos_storefront = manifest.build.server.embed_storefront
        || manifest
            .build
            .storefront
            .iter()
            .any(|storefront| storefront.stack.trim().eq_ignore_ascii_case("leptos"));

    has_leptos_storefront.then(|| {
        let mut command_parts = vec![
            "cargo".to_string(),
            "build".to_string(),
            "-p".to_string(),
            "rustok-storefront".to_string(),
        ];
        if cargo_profile == "release" {
            command_parts.push("--release".to_string());
        } else {
            command_parts.push("--profile".to_string());
            command_parts.push(cargo_profile.to_string());
        }
        if let Some(target) = cargo_target {
            command_parts.push("--target".to_string());
            command_parts.push(target.to_string());
        }

        let mut artifact_path = String::from("target/");
        if let Some(target) = cargo_target {
            artifact_path.push_str(target);
            artifact_path.push('/');
        }
        artifact_path.push_str(runtime_binary_output_dir_name(cargo_profile));
        artifact_path.push('/');
        artifact_path.push_str(&runtime_binary_file_name("rustok-storefront", cargo_target));

        RuntimeFrontendBuildPlan {
            surface: "storefront".to_string(),
            tool: RuntimeFrontendBuildTool::Cargo,
            package: "rustok-storefront".to_string(),
            workspace_path: ".".to_string(),
            profile: cargo_profile.to_string(),
            target: cargo_target.map(ToString::to_string),
            artifact_path,
            artifact_kind: RuntimeFrontendArtifactKind::File,
            command: command_parts.join(" "),
        }
    })
}

#[cfg(feature = "ssr")]
#[allow(dead_code)]
pub fn runtime_build_execution_plan(
    manifest: &RuntimeModulesManifest,
) -> RuntimeBuildExecutionPlan {
    let cargo_package = if manifest.app.trim().is_empty() {
        "rustok-server".to_string()
    } else {
        manifest.app.trim().to_string()
    };

    let cargo_profile = if manifest.build.profile.trim().is_empty() {
        "release".to_string()
    } else {
        manifest.build.profile.trim().to_string()
    };

    let cargo_target = (!manifest.build.target.trim().is_empty())
        .then(|| manifest.build.target.trim().to_string());

    let mut cargo_features = Vec::new();
    if manifest.build.server.embed_admin {
        cargo_features.push("embed-admin".to_string());
    }
    if manifest.build.server.embed_storefront {
        cargo_features.push("embed-storefront".to_string());
    }

    let mut command_parts = vec![
        "cargo".to_string(),
        "build".to_string(),
        "-p".to_string(),
        cargo_package.clone(),
    ];
    if cargo_profile == "release" {
        command_parts.push("--release".to_string());
    } else {
        command_parts.push("--profile".to_string());
        command_parts.push(cargo_profile.clone());
    }
    if let Some(target) = &cargo_target {
        command_parts.push("--target".to_string());
        command_parts.push(target.clone());
    }
    if !cargo_features.is_empty() {
        command_parts.push("--features".to_string());
        command_parts.push(cargo_features.join(","));
    }

    RuntimeBuildExecutionPlan {
        cargo_package,
        cargo_profile: cargo_profile.clone(),
        cargo_target: cargo_target.clone(),
        cargo_features,
        cargo_command: command_parts.join(" "),
        admin_build: runtime_admin_frontend_build_plan(manifest, &cargo_profile),
        storefront_build: runtime_storefront_frontend_build_plan(
            manifest,
            &cargo_profile,
            cargo_target.as_deref(),
        ),
    }
}

#[cfg(feature = "ssr")]
#[allow(dead_code)]
pub fn runtime_deployment_profile(manifest: &RuntimeModulesManifest) -> String {
    match (
        manifest.build.server.embed_admin,
        manifest.build.server.embed_storefront,
    ) {
        (true, true) => "monolith".to_string(),
        (true, false) => "server-with-admin".to_string(),
        (false, true) => "server-with-storefront".to_string(),
        (false, false) => "headless-api".to_string(),
    }
}

#[cfg(feature = "ssr")]
pub fn runtime_manifest_hash(manifest: &RuntimeModulesManifest) -> String {
    rustok_api::manifest_hash::hash_manifest(manifest)
        .unwrap_or_else(|_| runtime_manifest_snapshot_hash(&serde_json::Value::Null))
}

#[cfg(feature = "ssr")]
pub fn runtime_manifest_snapshot_hash(snapshot: &serde_json::Value) -> String {
    rustok_api::manifest_hash::hash_manifest_snapshot(snapshot)
}

#[cfg(all(test, feature = "ssr"))]
pub mod runtime_manifest_hash_tests {
    use super::{
        RuntimeBuildConfig, RuntimeManifestModuleSpec, RuntimeModulesManifest,
        RuntimeSettingsManifest, runtime_manifest_hash, runtime_manifest_snapshot_hash,
    };
    use std::collections::HashMap;

    fn sample_manifest() -> RuntimeModulesManifest {
        let mut modules = HashMap::new();
        modules.insert(
            "catalog".to_string(),
            RuntimeManifestModuleSpec {
                source: "git".to_string(),
                crate_name: "rustok-catalog".to_string(),
                path: Some("crates/rustok-catalog".to_string()),
                version: Some("1.0.0".to_string()),
                git: Some("https://example.invalid/catalog.git".to_string()),
                rev: Some("abc123".to_string()),
                required: false,
                depends_on: vec!["pricing".to_string()],
            },
        );
        RuntimeModulesManifest {
            schema: 1,
            app: "rustok".to_string(),
            build: RuntimeBuildConfig {
                profile: "release".to_string(),
                ..Default::default()
            },
            modules,
            settings: RuntimeSettingsManifest {
                default_enabled: vec!["catalog".to_string()],
            },
        }
    }

    #[test]
    fn manifest_snapshot_hash_is_sha256_hex() {
        let hash = runtime_manifest_snapshot_hash(&serde_json::json!({
            "modules": {"catalog": {"enabled": true}}
        }));
        assert_eq!(hash.len(), 64);
        assert!(hash.chars().all(|ch| ch.is_ascii_hexdigit()));
    }

    #[test]
    fn manifest_snapshot_hash_is_stable_for_key_order() {
        let left = runtime_manifest_snapshot_hash(&serde_json::json!({
            "modules": {"catalog": {"enabled": true}, "pricing": {"enabled": false}},
            "profile": "default",
            "settings": {"b": 1, "a": 2}
        }));
        let right = runtime_manifest_snapshot_hash(&serde_json::json!({
            "settings": {"a": 2, "b": 1},
            "profile": "default",
            "modules": {"pricing": {"enabled": false}, "catalog": {"enabled": true}}
        }));

        assert_eq!(left, right);
    }

    #[test]
    fn manifest_snapshot_hash_changes_for_meaningful_change() {
        let left =
            runtime_manifest_snapshot_hash(&serde_json::json!({"settings": {"locale": "en"}}));
        let right =
            runtime_manifest_snapshot_hash(&serde_json::json!({"settings": {"locale": "ru"}}));

        assert_ne!(left, right);
    }

    #[test]
    fn manifest_snapshot_hash_matches_known_sha256_vector() {
        let hash = runtime_manifest_snapshot_hash(&serde_json::json!({"b": 2, "a": 1}));
        assert_eq!(
            hash,
            "43258cff783fe7036d8a43033f830adfc60ec037382473548ac742b888292777"
        );
    }

    #[test]
    fn runtime_manifest_hash_changes_when_profile_changes() {
        let left = sample_manifest();
        let mut right = left.clone();
        right.build.profile = "debug".to_string();

        assert_ne!(runtime_manifest_hash(&left), runtime_manifest_hash(&right));
    }

    #[test]
    fn runtime_manifest_hash_changes_when_dependency_metadata_changes() {
        let left = sample_manifest();
        let mut right = left.clone();
        right
            .modules
            .get_mut("catalog")
            .expect("catalog module exists")
            .depends_on
            .push("inventory".to_string());

        assert_ne!(runtime_manifest_hash(&left), runtime_manifest_hash(&right));
    }

    #[test]
    fn runtime_manifest_hash_changes_when_source_pin_changes() {
        let left = sample_manifest();
        let mut right = left.clone();
        right
            .modules
            .get_mut("catalog")
            .expect("catalog module exists")
            .rev = Some("def456".to_string());

        assert_ne!(runtime_manifest_hash(&left), runtime_manifest_hash(&right));
    }

    #[test]
    fn runtime_manifest_hash_matches_canonical_snapshot_hash() {
        let manifest = sample_manifest();
        let snapshot = rustok_api::manifest_hash::canonical_manifest_snapshot_json(&manifest)
            .expect("serialize manifest snapshot");

        assert_eq!(
            runtime_manifest_hash(&manifest),
            runtime_manifest_snapshot_hash(&snapshot),
            "manifest hash must use the same canonical snapshot contract",
        );
    }

    #[test]
    fn runtime_manifest_hash_is_stable_for_module_map_order() {
        let mut left = sample_manifest();
        left.modules.insert(
            "pricing".to_string(),
            RuntimeManifestModuleSpec {
                source: "workspace".to_string(),
                crate_name: "rustok-pricing".to_string(),
                path: Some("crates/rustok-pricing".to_string()),
                version: Some("1.0.0".to_string()),
                git: None,
                rev: None,
                required: false,
                depends_on: vec![],
            },
        );

        let mut right = sample_manifest();
        right.modules.insert(
            "pricing".to_string(),
            RuntimeManifestModuleSpec {
                source: "workspace".to_string(),
                crate_name: "rustok-pricing".to_string(),
                path: Some("crates/rustok-pricing".to_string()),
                version: Some("1.0.0".to_string()),
                git: None,
                rev: None,
                required: false,
                depends_on: vec![],
            },
        );

        // Reinsert to ensure potentially different insertion history still hashes identically.
        let catalog = right
            .modules
            .remove("catalog")
            .expect("catalog module exists");
        right.modules.insert("catalog".to_string(), catalog);

        assert_eq!(
            runtime_manifest_hash(&left),
            runtime_manifest_hash(&right),
            "canonical serializer must normalize map ordering",
        );
    }
}

#[cfg(feature = "ssr")]
#[allow(dead_code)]
pub fn runtime_modules_delta_json(
    manifest: &RuntimeModulesManifest,
    summary: String,
) -> serde_json::Value {
    let modules = manifest
        .modules
        .iter()
        .map(|(slug, spec)| {
            (
                slug.clone(),
                serde_json::json!({
                    "source": spec.source,
                    "crate_name": spec.crate_name,
                    "version": spec.version,
                    "git": spec.git,
                    "rev": spec.rev,
                    "path": spec.path,
                }),
            )
        })
        .collect::<serde_json::Map<String, serde_json::Value>>();

    serde_json::json!({
        "summary": summary,
        "modules": modules,
        "execution_plan": runtime_build_execution_plan(manifest),
    })
}

#[cfg(feature = "ssr")]
pub fn runtime_module_roots(
    manifest: &RuntimeModulesManifest,
) -> Result<Vec<std::path::PathBuf>, ServerFnError> {
    let workspace_root = runtime_workspace_root();
    let crates_root = workspace_root.join("crates");
    let mut roots = std::collections::BTreeSet::new();

    if crates_root.exists() {
        for entry in std::fs::read_dir(&crates_root).map_err(|err| {
            server_error(format!("failed to read {}: {err}", crates_root.display()))
        })? {
            let entry = entry.map_err(|err| server_error(err.to_string()))?;
            let path = entry.path();
            if path.join("rustok-module.toml").exists() {
                roots.insert(path);
            }
        }
    }

    for spec in manifest.modules.values() {
        if let Some(path) = spec.path.as_ref() {
            let module_root = workspace_root.join(path);
            if module_root.join("rustok-module.toml").exists() {
                roots.insert(module_root);
            }
        }
    }

    Ok(roots.into_iter().collect())
}

#[cfg(feature = "ssr")]
pub fn load_runtime_marketplace_modules(
    registry: &rustok_core::ModuleRegistry,
    manifest: &RuntimeModulesManifest,
) -> Result<Vec<MarketplaceModule>, ServerFnError> {
    let module_roots = runtime_module_roots(manifest)?;
    let mut installed_by_slug = manifest.modules.clone();
    let mut modules = Vec::new();

    for module_root in module_roots {
        let package_manifest: RuntimeModulePackageManifest =
            load_toml_file(&module_root.join("rustok-module.toml"))?;
        let cargo_manifest: RuntimeCargoManifest = load_toml_file(&module_root.join("Cargo.toml"))?;
        let slug = package_manifest.module.slug.clone();
        let installed_entry = installed_by_slug.remove(&slug);
        let runtime_module = registry.get(&slug);
        let latest_version = runtime_module
            .map(|module| module.version().to_string())
            .unwrap_or_else(|| package_manifest.module.version.clone());
        let installed_version = installed_entry
            .as_ref()
            .and_then(|entry| entry.version.clone());
        let dependencies = runtime_module
            .map(|module| {
                module
                    .dependencies()
                    .iter()
                    .map(|dependency| dependency.to_string())
                    .collect::<Vec<_>>()
            })
            .unwrap_or_else(|| {
                if package_manifest.dependencies.is_empty() {
                    installed_entry
                        .as_ref()
                        .map(|entry| entry.depends_on.clone())
                        .unwrap_or_default()
                } else {
                    package_manifest.dependencies.keys().cloned().collect()
                }
            });

        modules.push(MarketplaceModule {
            slug: slug.clone(),
            name: runtime_module
                .map(|module| module.name().to_string())
                .unwrap_or_else(|| package_manifest.module.name.clone()),
            latest_version: latest_version.clone(),
            description: runtime_module
                .map(|module| module.description().to_string())
                .filter(|value| !value.is_empty())
                .unwrap_or_else(|| package_manifest.module.description.clone()),
            source: installed_entry
                .as_ref()
                .map(|entry| entry.source.clone())
                .unwrap_or_else(|| "path".to_string()),
            kind: if registry.is_core(&slug)
                || installed_entry.as_ref().is_some_and(|entry| entry.required)
            {
                "core".to_string()
            } else {
                "optional".to_string()
            },
            category: package_manifest
                .marketplace
                .category
                .clone()
                .unwrap_or_else(|| fallback_module_category(&slug).to_string()),
            tags: package_manifest.marketplace.tags.clone(),
            icon_url: package_manifest.marketplace.icon_url.clone(),
            banner_url: package_manifest.marketplace.banner_url.clone(),
            screenshots: package_manifest.marketplace.screenshots.clone(),
            crate_name: installed_entry
                .as_ref()
                .map(|entry| entry.crate_name.clone())
                .filter(|value| !value.is_empty())
                .unwrap_or_else(|| cargo_manifest.package.name.clone()),
            dependencies,
            ownership: package_manifest.module.ownership.clone(),
            trust_level: package_manifest.module.trust_level.clone(),
            rustok_min_version: package_manifest.module.rustok_min_version.clone(),
            rustok_max_version: package_manifest.module.rustok_max_version.clone(),
            publisher: package_manifest.marketplace.publisher.clone(),
            checksum_sha256: package_manifest.marketplace.checksum_sha256.clone(),
            signature_present: package_manifest.marketplace.signature.is_some(),
            versions: vec![crate::entities::module::model::MarketplaceModuleVersion {
                version: latest_version.clone(),
                changelog: None,
                yanked: false,
                published_at: None,
                checksum_sha256: package_manifest.marketplace.checksum_sha256.clone(),
                signature_present: package_manifest.marketplace.signature.is_some(),
            }],
            has_admin_ui: false,
            has_storefront_ui: false,
            ui_classification: "no-ui".to_string(),
            registry_lifecycle: None,
            compatible: true,
            recommended_admin_surfaces: package_manifest.module.recommended_admin_surfaces.clone(),
            showcase_admin_surfaces: package_manifest.module.showcase_admin_surfaces.clone(),
            settings_schema: runtime_setting_fields(&package_manifest.settings),
            installed: installed_entry.is_some(),
            installed_version: installed_version.clone(),
            update_available: installed_version
                .as_ref()
                .is_some_and(|version| version != &latest_version),
        });
    }

    for (slug, entry) in installed_by_slug {
        let latest_version = registry
            .get(&slug)
            .map(|module| module.version().to_string())
            .or(entry.version.clone())
            .unwrap_or_else(|| "workspace".to_string());
        modules.push(MarketplaceModule {
            slug: slug.clone(),
            name: registry
                .get(&slug)
                .map(|module| module.name().to_string())
                .unwrap_or_else(|| humanize_module_slug(&slug)),
            latest_version: latest_version.clone(),
            description: registry
                .get(&slug)
                .map(|module| module.description().to_string())
                .unwrap_or_else(|| format!("{} module", humanize_module_slug(&slug))),
            source: entry.source,
            kind: if registry.is_core(&slug) || entry.required {
                "core".to_string()
            } else {
                "optional".to_string()
            },
            category: fallback_module_category(&slug).to_string(),
            tags: Vec::new(),
            icon_url: None,
            banner_url: None,
            screenshots: Vec::new(),
            crate_name: if entry.crate_name.is_empty() {
                format!("rustok-{slug}")
            } else {
                entry.crate_name
            },
            dependencies: if entry.depends_on.is_empty() {
                registry
                    .get(&slug)
                    .map(|module| {
                        module
                            .dependencies()
                            .iter()
                            .map(|dependency| dependency.to_string())
                            .collect()
                    })
                    .unwrap_or_default()
            } else {
                entry.depends_on
            },
            ownership: "third_party".to_string(),
            trust_level: "unverified".to_string(),
            rustok_min_version: None,
            rustok_max_version: None,
            publisher: None,
            checksum_sha256: None,
            signature_present: false,
            versions: vec![crate::entities::module::model::MarketplaceModuleVersion {
                version: latest_version.clone(),
                changelog: None,
                yanked: false,
                published_at: None,
                checksum_sha256: None,
                signature_present: false,
            }],
            has_admin_ui: false,
            has_storefront_ui: false,
            ui_classification: "no-ui".to_string(),
            registry_lifecycle: None,
            compatible: true,
            recommended_admin_surfaces: Vec::new(),
            showcase_admin_surfaces: Vec::new(),
            settings_schema: Vec::new(),
            installed: true,
            installed_version: entry.version.clone(),
            update_available: entry
                .version
                .as_ref()
                .is_some_and(|version| version != &latest_version),
        });
    }

    modules.sort_by(|left, right| left.slug.cmp(&right.slug));
    Ok(modules)
}

#[cfg(feature = "ssr")]
pub fn load_runtime_module_package_manifest_by_slug(
    module_slug: &str,
    manifest: &RuntimeModulesManifest,
) -> Result<Option<RuntimeModulePackageManifest>, ServerFnError> {
    for module_root in runtime_module_roots(manifest)? {
        let package_manifest: RuntimeModulePackageManifest =
            load_toml_file(&module_root.join("rustok-module.toml"))?;
        if package_manifest.module.slug == module_slug {
            return Ok(Some(package_manifest));
        }
    }

    Ok(None)
}
