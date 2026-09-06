//! Pure owner validation for registry publish bundles.

use serde::{Deserialize, Serialize};

use crate::{
    MODULE_ARTIFACT_SOURCE_MANIFEST_FILE, ModuleArtifactSourceManifest,
    ModulePublicationArtifactOrigin, ModulePublishValidationContract,
};

/// Maximum accepted serialized publish bundle size. The bundle carries only
/// registry metadata and bounded manifest text, never an executable payload.
pub const MODULE_PUBLISH_ARTIFACT_MAX_BYTES: usize = 2 * 1024 * 1024;
/// Maximum text size for any embedded TOML manifest in a publish bundle.
pub const MODULE_PUBLISH_ARTIFACT_MANIFEST_MAX_BYTES: usize = 256 * 1024;
/// Maximum serialized canonical workspace accepted for an Alloy-authored
/// artifact. The exact bytes still have to match the reviewed source digest at
/// the owner staging boundary.
pub const MODULE_PUBLISH_ALLOY_WORKSPACE_MAX_BYTES: usize = 1024 * 1024;

/// Required `artifact_type` for an uploaded registry publish bundle.
pub const MODULE_PUBLISH_BUNDLE_TYPE: &str = "rustok-module-publish-bundle";
/// Canonical media type for the bounded registry metadata bundle.
pub const MODULE_PUBLISH_BUNDLE_CONTENT_TYPE: &str = "application/json";

/// Content-free validation evidence suitable for durable governance events.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ModulePublishBundleValidation {
    pub warnings: Vec<String>,
    pub errors: Vec<String>,
}

/// Current authoring files embedded in the bounded registry metadata bundle.
/// Executable Component bytes are published and addressed separately in OCI.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ModulePublishBundleFiles {
    pub source_manifest: String,
    pub crate_manifest: String,
    pub admin_manifest: Option<String>,
    pub storefront_manifest: Option<String>,
}

/// Builds the one canonical metadata bundle and immediately validates the
/// serialized representation against the same owner contract used by the
/// isolated registry-validation worker.
pub fn build_module_publish_bundle(
    contract: &ModulePublishValidationContract,
    files: ModulePublishBundleFiles,
) -> Result<Vec<u8>, ModulePublishBundleValidation> {
    let bundle = Bundle {
        schema_version: 1,
        artifact_type: MODULE_PUBLISH_BUNDLE_TYPE.to_string(),
        module: BundleModule {
            slug: contract.slug.clone(),
            version: contract.version.clone(),
            crate_name: contract.crate_name.clone(),
            module_name: contract.module_name.clone(),
            module_description: contract.module_description.clone(),
            ownership: contract.ownership.clone(),
            trust_level: contract.trust_level.clone(),
            license: contract.license.clone(),
            module_entry_type: contract.entry_type.clone(),
            marketplace: BundleMarketplace {
                category: contract.marketplace_category.clone(),
                tags: normalize_string_list(&contract.marketplace_tags),
            },
            ui_packages: BundleUiPackages {
                admin: contract
                    .admin_ui_crate_name
                    .as_ref()
                    .map(|crate_name| BundleUiPackage {
                        crate_name: crate_name.clone(),
                    }),
                storefront: contract
                    .storefront_ui_crate_name
                    .as_ref()
                    .map(|crate_name| BundleUiPackage {
                        crate_name: crate_name.clone(),
                    }),
            },
        },
        files: BundleFiles {
            source_manifest: Some(files.source_manifest),
            crate_manifest: Some(files.crate_manifest),
            admin_manifest: files.admin_manifest,
            storefront_manifest: files.storefront_manifest,
        },
    };
    let bytes = match serde_json::to_vec(&bundle) {
        Ok(bytes) => bytes,
        Err(_) => {
            return Err(ModulePublishBundleValidation {
                warnings: Vec::new(),
                errors: vec!["Registry publish bundle serialization failed.".to_string()],
            });
        }
    };
    let validation =
        validate_module_publish_bundle(contract, MODULE_PUBLISH_BUNDLE_CONTENT_TYPE, &bytes);
    if validation.errors.is_empty() {
        Ok(bytes)
    } else {
        Err(validation)
    }
}

/// Validates the delivery representation selected by immutable artifact
/// origin. Platform-built and external artifacts retain the registry metadata
/// bundle contract; Alloy-authored releases carry the executable canonical
/// workspace whose exact digest is later bound to reviewed source evidence.
pub fn validate_module_publish_artifact(
    artifact_origin: ModulePublicationArtifactOrigin,
    contract: &ModulePublishValidationContract,
    content_type: &str,
    bytes: &[u8],
) -> ModulePublishBundleValidation {
    match artifact_origin {
        ModulePublicationArtifactOrigin::AlloyAuthored => {
            validate_alloy_workspace_delivery(content_type, bytes)
        }
        ModulePublicationArtifactOrigin::PlatformBuilt
        | ModulePublicationArtifactOrigin::ExternalPrebuilt => {
            validate_module_publish_bundle(contract, content_type, bytes)
        }
    }
}

fn validate_alloy_workspace_delivery(
    content_type: &str,
    bytes: &[u8],
) -> ModulePublishBundleValidation {
    let mut validation = ModulePublishBundleValidation::default();
    if bytes.len() > MODULE_PUBLISH_ALLOY_WORKSPACE_MAX_BYTES {
        validation.errors.push(format!(
            "Alloy workspace artifact exceeds the {} byte validation limit.",
            MODULE_PUBLISH_ALLOY_WORKSPACE_MAX_BYTES
        ));
        return validation;
    }
    if content_type != rustok_sandbox::RHAI_WORKSPACE_MEDIA_TYPE {
        validation
            .errors
            .push("Alloy workspace artifact content type is unsupported.".to_string());
        return validation;
    }
    let workspace = match serde_json::from_slice::<serde_json::Value>(bytes) {
        Ok(serde_json::Value::Object(workspace)) => workspace,
        _ => {
            validation
                .errors
                .push("Alloy workspace artifact is not a valid JSON object.".to_string());
            return validation;
        }
    };
    if workspace
        .get("schema_version")
        .and_then(serde_json::Value::as_u64)
        != Some(1)
    {
        validation
            .errors
            .push("Alloy workspace artifact schema_version is unsupported.".to_string());
    }
    if workspace
        .get("entrypoint")
        .and_then(serde_json::Value::as_str)
        .is_none_or(|entrypoint| entrypoint.trim().is_empty())
    {
        validation
            .errors
            .push("Alloy workspace artifact entrypoint is missing.".to_string());
    }
    if workspace
        .get("files")
        .and_then(serde_json::Value::as_array)
        .is_none_or(Vec::is_empty)
    {
        validation
            .errors
            .push("Alloy workspace artifact files are missing.".to_string());
    }
    validation
}

#[derive(Debug, Serialize, Deserialize)]
struct Bundle {
    schema_version: u32,
    artifact_type: String,
    module: BundleModule,
    files: BundleFiles,
}

#[derive(Debug, Serialize, Deserialize)]
struct BundleModule {
    slug: String,
    version: String,
    crate_name: String,
    module_name: String,
    module_description: String,
    ownership: String,
    trust_level: String,
    license: String,
    module_entry_type: Option<String>,
    marketplace: BundleMarketplace,
    ui_packages: BundleUiPackages,
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct BundleMarketplace {
    category: Option<String>,
    #[serde(default)]
    tags: Vec<String>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct BundleUiPackages {
    admin: Option<BundleUiPackage>,
    storefront: Option<BundleUiPackage>,
}

#[derive(Debug, Serialize, Deserialize)]
struct BundleUiPackage {
    crate_name: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct BundleFiles {
    #[serde(rename = "module-artifact.json")]
    source_manifest: Option<String>,
    #[serde(rename = "Cargo.toml")]
    crate_manifest: Option<String>,
    #[serde(rename = "admin/Cargo.toml")]
    admin_manifest: Option<String>,
    #[serde(rename = "storefront/Cargo.toml")]
    storefront_manifest: Option<String>,
}

/// Validates an uploaded artifact against immutable owner-supplied request
/// facts. Diagnostics deliberately never include untrusted bundle text.
pub fn validate_module_publish_bundle(
    contract: &ModulePublishValidationContract,
    content_type: &str,
    bytes: &[u8],
) -> ModulePublishBundleValidation {
    let mut validation = ModulePublishBundleValidation::default();
    if bytes.len() > MODULE_PUBLISH_ARTIFACT_MAX_BYTES {
        validation.errors.push(format!(
            "Artifact bundle exceeds the {} byte validation limit.",
            MODULE_PUBLISH_ARTIFACT_MAX_BYTES
        ));
        return validation;
    }
    if !content_type.eq_ignore_ascii_case("application/json") {
        validation.warnings.push(
            "Artifact upload content type is accepted, but application/json is the canonical bundle content type."
                .to_string(),
        );
    }
    let bundle = match serde_json::from_slice::<Bundle>(bytes) {
        Ok(bundle) => bundle,
        Err(_) => {
            validation.errors.push(
                "Artifact bundle is not valid JSON for the registry publish contract.".to_string(),
            );
            return validation;
        }
    };
    if bundle.schema_version != 1 {
        validation
            .errors
            .push("Artifact bundle schema_version is unsupported.".to_string());
    }
    if bundle.artifact_type != MODULE_PUBLISH_BUNDLE_TYPE {
        validation
            .errors
            .push("Artifact bundle type is unsupported.".to_string());
    }
    validate_module_contract(contract, &bundle, &mut validation);
    validate_file_contract(contract, &bundle, &mut validation);
    dedupe(&mut validation.warnings);
    dedupe(&mut validation.errors);
    validation
}

fn validate_module_contract(
    contract: &ModulePublishValidationContract,
    bundle: &Bundle,
    validation: &mut ModulePublishBundleValidation,
) {
    validate_exact(
        "module.slug",
        &bundle.module.slug,
        &contract.slug,
        validation,
    );
    validate_exact(
        "module.version",
        &bundle.module.version,
        &contract.version,
        validation,
    );
    validate_exact(
        "module.crate_name",
        &bundle.module.crate_name,
        &contract.crate_name,
        validation,
    );
    validate_exact(
        "module.name",
        &bundle.module.module_name,
        &contract.module_name,
        validation,
    );
    validate_exact(
        "module.description",
        &bundle.module.module_description,
        &contract.module_description,
        validation,
    );
    validate_exact(
        "module.ownership",
        &bundle.module.ownership,
        &contract.ownership,
        validation,
    );
    validate_exact(
        "module.trust_level",
        &bundle.module.trust_level,
        &contract.trust_level,
        validation,
    );
    validate_exact(
        "module.license",
        &bundle.module.license,
        &contract.license,
        validation,
    );
    validate_optional(
        "module.entry_type",
        bundle.module.module_entry_type.as_deref(),
        contract.entry_type.as_deref(),
        validation,
    );
    validate_optional(
        "module.marketplace.category",
        bundle.module.marketplace.category.as_deref(),
        contract.marketplace_category.as_deref(),
        validation,
    );
    if normalize_string_list(&bundle.module.marketplace.tags)
        != normalize_string_list(&contract.marketplace_tags)
    {
        validation.errors.push(
            "Artifact bundle module.marketplace.tags does not match the publish request."
                .to_string(),
        );
    }
    validate_optional(
        "module.ui_packages.admin.crate_name",
        bundle
            .module
            .ui_packages
            .admin
            .as_ref()
            .map(|ui| ui.crate_name.as_str()),
        contract.admin_ui_crate_name.as_deref(),
        validation,
    );
    validate_optional(
        "module.ui_packages.storefront.crate_name",
        bundle
            .module
            .ui_packages
            .storefront
            .as_ref()
            .map(|ui| ui.crate_name.as_str()),
        contract.storefront_ui_crate_name.as_deref(),
        validation,
    );
}

fn validate_file_contract(
    contract: &ModulePublishValidationContract,
    bundle: &Bundle,
    validation: &mut ModulePublishBundleValidation,
) {
    let source_manifest = require_file(
        MODULE_ARTIFACT_SOURCE_MANIFEST_FILE,
        bundle.files.source_manifest.as_deref(),
        validation,
    );
    let crate_manifest = require_file(
        "Cargo.toml",
        bundle.files.crate_manifest.as_deref(),
        validation,
    );
    let admin_manifest = optional_file(
        "admin/Cargo.toml",
        bundle.files.admin_manifest.as_deref(),
        validation,
    );
    let storefront_manifest = optional_file(
        "storefront/Cargo.toml",
        bundle.files.storefront_manifest.as_deref(),
        validation,
    );
    validate_ui_file_presence(
        "admin/Cargo.toml",
        contract.admin_ui_crate_name.is_some(),
        admin_manifest.is_some(),
        validation,
    );
    validate_ui_file_presence(
        "storefront/Cargo.toml",
        contract.storefront_ui_crate_name.is_some(),
        storefront_manifest.is_some(),
        validation,
    );
    if let Some(source) = source_manifest {
        validate_source_manifest(source, contract, validation);
    }
    if let Some(source) = crate_manifest {
        validate_cargo_manifest(
            "Cargo.toml",
            source,
            &contract.crate_name,
            &contract.version,
            Some(&contract.license),
            validation,
        );
    }
    if let (Some(crate_name), Some(source)) = (&contract.admin_ui_crate_name, admin_manifest) {
        validate_cargo_manifest(
            "admin/Cargo.toml",
            source,
            crate_name,
            &contract.version,
            None,
            validation,
        );
    }
    if let (Some(crate_name), Some(source)) =
        (&contract.storefront_ui_crate_name, storefront_manifest)
    {
        validate_cargo_manifest(
            "storefront/Cargo.toml",
            source,
            crate_name,
            &contract.version,
            None,
            validation,
        );
    }
}

fn validate_ui_file_presence(
    label: &str,
    declared: bool,
    present: bool,
    validation: &mut ModulePublishBundleValidation,
) {
    match (declared, present) {
        (true, false) => validation.errors.push(format!(
            "Artifact bundle must include {label} because the publish request declares that UI package."
        )),
        (false, true) => validation.errors.push(format!(
            "Artifact bundle includes {label}, but the publish request does not declare that UI package."
        )),
        _ => {}
    }
}

fn validate_source_manifest(
    source: &str,
    contract: &ModulePublishValidationContract,
    validation: &mut ModulePublishBundleValidation,
) {
    let manifest = match ModuleArtifactSourceManifest::parse(source.as_bytes()) {
        Ok(manifest) => manifest,
        Err(_) => {
            validation.errors.push(format!(
                "Artifact file {MODULE_ARTIFACT_SOURCE_MANIFEST_FILE} is not a valid current source manifest."
            ));
            return;
        }
    };
    if manifest.slug() != contract.slug {
        validation
            .errors
            .push("Artifact source manifest slug does not match the publish request.".to_string());
    }
    if manifest.version() != contract.version {
        validation.errors.push(
            "Artifact source manifest version does not match the publish request.".to_string(),
        );
    }
}

fn validate_cargo_manifest(
    label: &str,
    source: &str,
    expected_name: &str,
    expected_version: &str,
    expected_license: Option<&str>,
    validation: &mut ModulePublishBundleValidation,
) {
    let manifest = match source.parse::<toml::Table>() {
        Ok(manifest) => toml::Value::Table(manifest),
        Err(_) => {
            validation
                .errors
                .push(format!("Artifact file {label} is not valid TOML."));
            return;
        }
    };
    validate_toml_string(
        &manifest,
        &["package", "name"],
        &format!("{label} [package].name"),
        expected_name,
        validation,
    );
    validate_toml_workspace_aware(
        &manifest,
        &["package", "version"],
        &format!("{label} [package].version"),
        expected_version,
        validation,
    );
    if let Some(expected_license) = expected_license {
        validate_toml_workspace_aware(
            &manifest,
            &["package", "license"],
            &format!("{label} [package].license"),
            expected_license,
            validation,
        );
    }
}

fn validate_exact(
    label: &str,
    actual: &str,
    expected: &str,
    validation: &mut ModulePublishBundleValidation,
) {
    if actual.trim() != expected.trim() {
        validation.errors.push(format!(
            "Artifact bundle {label} does not match the publish request."
        ));
    }
}

fn validate_optional(
    label: &str,
    actual: Option<&str>,
    expected: Option<&str>,
    validation: &mut ModulePublishBundleValidation,
) {
    let actual = actual.map(str::trim).filter(|value| !value.is_empty());
    let expected = expected.map(str::trim).filter(|value| !value.is_empty());
    if actual != expected {
        validation.errors.push(format!(
            "Artifact bundle {label} does not match the publish request."
        ));
    }
}

fn require_file<'a>(
    label: &str,
    source: Option<&'a str>,
    validation: &mut ModulePublishBundleValidation,
) -> Option<&'a str> {
    match source.map(str::trim) {
        Some(source)
            if !source.is_empty() && source.len() <= MODULE_PUBLISH_ARTIFACT_MANIFEST_MAX_BYTES =>
        {
            Some(source)
        }
        Some(source) if source.len() > MODULE_PUBLISH_ARTIFACT_MANIFEST_MAX_BYTES => {
            validation.errors.push(format!(
                "Artifact bundle file '{label}' exceeds the {} byte validation limit.",
                MODULE_PUBLISH_ARTIFACT_MANIFEST_MAX_BYTES
            ));
            None
        }
        _ => {
            validation.errors.push(format!(
                "Artifact bundle must include non-empty file '{label}'."
            ));
            None
        }
    }
}

fn optional_file<'a>(
    label: &str,
    source: Option<&'a str>,
    validation: &mut ModulePublishBundleValidation,
) -> Option<&'a str> {
    match source.map(str::trim).filter(|source| !source.is_empty()) {
        Some(source) if source.len() <= MODULE_PUBLISH_ARTIFACT_MANIFEST_MAX_BYTES => Some(source),
        Some(_) => {
            validation.errors.push(format!(
                "Artifact bundle file '{label}' exceeds the {} byte validation limit.",
                MODULE_PUBLISH_ARTIFACT_MANIFEST_MAX_BYTES
            ));
            None
        }
        None => None,
    }
}

fn normalize_string_list(values: &[String]) -> Vec<String> {
    let mut values = values
        .iter()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .collect::<Vec<_>>();
    values.sort();
    values.dedup();
    values
}

fn toml_value_at_path<'a>(value: &'a toml::Value, path: &[&str]) -> Option<&'a toml::Value> {
    let mut current = value;
    for segment in path {
        current = current.get(*segment)?;
    }
    Some(current)
}

fn toml_string(value: &toml::Value, path: &[&str]) -> Option<String> {
    toml_value_at_path(value, path)
        .and_then(toml::Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
}

fn toml_is_workspace_inherited(value: &toml::Value, path: &[&str]) -> bool {
    toml_value_at_path(value, path)
        .and_then(toml::Value::as_table)
        .and_then(|table| table.get("workspace"))
        .and_then(toml::Value::as_bool)
        == Some(true)
}

fn validate_toml_string(
    manifest: &toml::Value,
    path: &[&str],
    label: &str,
    expected: &str,
    validation: &mut ModulePublishBundleValidation,
) {
    if toml_string(manifest, path).as_deref() != Some(expected.trim()) {
        validation.errors.push(format!(
            "Artifact file {label} does not match the publish request."
        ));
    }
}

fn validate_toml_workspace_aware(
    manifest: &toml::Value,
    path: &[&str],
    label: &str,
    expected: &str,
    validation: &mut ModulePublishBundleValidation,
) {
    if let Some(actual) = toml_string(manifest, path) {
        if actual != expected.trim() {
            validation.errors.push(format!(
                "Artifact file {label} does not match the publish request."
            ));
        }
        return;
    }
    if toml_is_workspace_inherited(manifest, path) {
        validation.warnings.push(format!(
            "Artifact file {label} uses workspace inheritance, so the registry validator cannot verify it from the uploaded bundle alone."
        ));
        return;
    }
    validation.warnings.push(format!(
        "Artifact file {label} is missing, so the registry validator could not verify it from the uploaded bundle."
    ));
}

fn dedupe(values: &mut Vec<String>) {
    values.retain(|value| !value.trim().is_empty());
    values.sort();
    values.dedup();
}

#[cfg(test)]
mod tests {
    use super::*;

    fn contract() -> ModulePublishValidationContract {
        ModulePublishValidationContract {
            slug: "sample_module".to_string(),
            version: "1.0.0".to_string(),
            crate_name: "sample_module".to_string(),
            module_name: "Sample module".to_string(),
            module_description: "Sample module description".to_string(),
            ownership: "first_party".to_string(),
            trust_level: "sandboxed".to_string(),
            license: "MIT".to_string(),
            entry_type: None,
            marketplace_category: None,
            marketplace_tags: Vec::new(),
            admin_ui_crate_name: None,
            storefront_ui_crate_name: None,
        }
    }

    fn bundle() -> serde_json::Value {
        serde_json::json!({
            "schema_version": 1,
            "artifact_type": MODULE_PUBLISH_BUNDLE_TYPE,
            "module": {
                "slug": "sample_module",
                "version": "1.0.0",
                "crate_name": "sample_module",
                "module_name": "Sample module",
                "module_description": "Sample module description",
                "ownership": "first_party",
                "trust_level": "sandboxed",
                "license": "MIT",
                "module_entry_type": null,
                "marketplace": { "category": null, "tags": [] },
                "ui_packages": { "admin": null, "storefront": null }
            },
            "files": {
                "module-artifact.json": serde_json::to_string(&serde_json::json!({
                    "schema_version": crate::MODULE_ARTIFACT_DESCRIPTOR_SCHEMA_VERSION,
                    "slug": "sample_module",
                    "version": "1.0.0",
                    "payload_kind": "wasm_component",
                    "module_kind": "optional",
                    "runtime_abi": crate::MODULE_BUILD_RUNTIME_ABI,
                    "platform_compatibility": "^0.1",
                    "required_features": [],
                    "entrypoint": "run",
                    "capabilities": [],
                    "bindings": [],
                    "dependencies": [],
                    "permissions": [],
                    "schema_documents": [],
                    "settings_schema_digest": null,
                    "data_schema_digest": null,
                    "ui_contributions": [],
                    "persistence_contract": null
                })).expect("source manifest JSON"),
                "Cargo.toml": "[package]\nname = \"sample_module\"\nversion = \"1.0.0\"\nlicense = \"MIT\"\n"
            }
        })
    }

    fn validate_bundle(bundle: &serde_json::Value) -> ModulePublishBundleValidation {
        validate_module_publish_bundle(
            &contract(),
            "application/json",
            &serde_json::to_vec(bundle).expect("bundle JSON"),
        )
    }

    #[test]
    fn canonical_writer_emits_the_current_source_manifest_bundle() {
        let fixture = bundle();
        let bytes = build_module_publish_bundle(
            &contract(),
            ModulePublishBundleFiles {
                source_manifest: fixture["files"]["module-artifact.json"]
                    .as_str()
                    .expect("source manifest")
                    .to_string(),
                crate_manifest: fixture["files"]["Cargo.toml"]
                    .as_str()
                    .expect("Cargo manifest")
                    .to_string(),
                admin_manifest: None,
                storefront_manifest: None,
            },
        )
        .expect("canonical bundle");
        let written: serde_json::Value = serde_json::from_slice(&bytes).expect("bundle JSON");

        assert_eq!(written["artifact_type"], MODULE_PUBLISH_BUNDLE_TYPE);
        assert!(written["files"]["module-artifact.json"].is_string());
        assert!(written["files"].get("rustok-module.toml").is_none());
        assert!(
            validate_module_publish_bundle(&contract(), MODULE_PUBLISH_BUNDLE_CONTENT_TYPE, &bytes)
                .errors
                .is_empty()
        );
    }

    #[test]
    fn registry_bundle_fixture_binds_metadata_and_manifests_to_owner_contract() {
        let accepted = validate_bundle(&bundle());
        assert!(accepted.errors.is_empty(), "{:?}", accepted.errors);

        let mut substituted_metadata = bundle();
        substituted_metadata["module"]["version"] = serde_json::json!("9.9.9");
        assert!(!validate_bundle(&substituted_metadata).errors.is_empty());

        let mut substituted_package_manifest = bundle();
        let mut source_manifest: serde_json::Value = serde_json::from_str(
            substituted_package_manifest["files"]["module-artifact.json"]
                .as_str()
                .expect("source manifest text"),
        )
        .expect("source manifest JSON");
        source_manifest["slug"] = serde_json::json!("other_module");
        substituted_package_manifest["files"]["module-artifact.json"] =
            serde_json::json!(serde_json::to_string(&source_manifest).expect("source manifest"));
        assert!(
            !validate_bundle(&substituted_package_manifest)
                .errors
                .is_empty()
        );

        let mut substituted_cargo_manifest = bundle();
        substituted_cargo_manifest["files"]["Cargo.toml"] = serde_json::json!(
            "[package]\nname = \"other_module\"\nversion = \"1.0.0\"\nlicense = \"GPL-3.0\"\n"
        );
        assert!(
            !validate_bundle(&substituted_cargo_manifest)
                .errors
                .is_empty()
        );
    }

    #[test]
    fn registry_bundle_fixture_rejects_undeclared_ui_manifest_without_echoing_it() {
        let marker = "<untrusted-ui-manifest-marker>";
        let mut unexpected_ui = bundle();
        unexpected_ui["files"]["admin/Cargo.toml"] = serde_json::json!(format!(
            "[package]\nname = \"unexpected\"\nversion = \"1.0.0\"\n# {marker}\n"
        ));

        let validation = validate_bundle(&unexpected_ui);
        assert!(!validation.errors.is_empty());
        assert!(
            validation
                .errors
                .iter()
                .all(|diagnostic| !diagnostic.contains(marker))
        );
    }

    #[test]
    fn alloy_delivery_accepts_only_the_bounded_workspace_envelope() {
        let workspace = br#"{"schema_version":1,"entrypoint":"src/main.rhai","files":[{"path":"src/main.rhai","kind":"source","contents":"40 + 2"}]}"#;
        let accepted = validate_module_publish_artifact(
            ModulePublicationArtifactOrigin::AlloyAuthored,
            &contract(),
            rustok_sandbox::RHAI_WORKSPACE_MEDIA_TYPE,
            workspace,
        );
        assert!(accepted.errors.is_empty());

        let wrong_type = validate_module_publish_artifact(
            ModulePublicationArtifactOrigin::AlloyAuthored,
            &contract(),
            "application/json",
            workspace,
        );
        assert_eq!(wrong_type.errors.len(), 1);

        let oversized = validate_module_publish_artifact(
            ModulePublicationArtifactOrigin::AlloyAuthored,
            &contract(),
            rustok_sandbox::RHAI_WORKSPACE_MEDIA_TYPE,
            &vec![b'x'; MODULE_PUBLISH_ALLOY_WORKSPACE_MAX_BYTES + 1],
        );
        assert_eq!(oversized.errors.len(), 1);
    }

    #[test]
    fn oversized_untrusted_artifact_text_never_enters_validation_diagnostics() {
        let marker = "<untrusted-prompt-injection-marker>";
        let source = marker.repeat(MODULE_PUBLISH_ARTIFACT_MANIFEST_MAX_BYTES / marker.len() + 1);
        let mut validation = ModulePublishBundleValidation::default();

        require_file("Cargo.toml", Some(&source), &mut validation);

        assert!(!validation.errors.is_empty());
        assert!(
            validation
                .errors
                .iter()
                .all(|diagnostic| !diagnostic.contains(marker))
        );
    }
}
