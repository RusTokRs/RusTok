//! Pure owner validation for registry publish bundles.

use serde::Deserialize;

use crate::ModulePublishValidationContract;

/// Maximum accepted serialized publish bundle size. The bundle carries only
/// registry metadata and bounded manifest text, never an executable payload.
pub const MODULE_PUBLISH_ARTIFACT_MAX_BYTES: usize = 2 * 1024 * 1024;
/// Maximum text size for any embedded TOML manifest in a publish bundle.
pub const MODULE_PUBLISH_ARTIFACT_MANIFEST_MAX_BYTES: usize = 256 * 1024;

/// Required `artifact_type` for an uploaded registry publish bundle.
pub const MODULE_PUBLISH_BUNDLE_TYPE: &str = "rustok-module-publish-bundle";

/// Content-free validation evidence suitable for durable governance events.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ModulePublishBundleValidation {
    pub warnings: Vec<String>,
    pub errors: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct Bundle {
    schema_version: u32,
    artifact_type: String,
    module: BundleModule,
    files: BundleFiles,
}

#[derive(Debug, Deserialize)]
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

#[derive(Debug, Default, Deserialize)]
struct BundleMarketplace {
    category: Option<String>,
    #[serde(default)]
    tags: Vec<String>,
}

#[derive(Debug, Default, Deserialize)]
struct BundleUiPackages {
    admin: Option<BundleUiPackage>,
    storefront: Option<BundleUiPackage>,
}

#[derive(Debug, Deserialize)]
struct BundleUiPackage {
    crate_name: String,
}

#[derive(Debug, Deserialize)]
struct BundleFiles {
    #[serde(rename = "rustok-module.toml")]
    package_manifest: Option<String>,
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
    let package_manifest = require_file(
        "rustok-module.toml",
        bundle.files.package_manifest.as_deref(),
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
    if let Some(source) = package_manifest {
        validate_package_manifest(source, contract, validation);
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

fn validate_package_manifest(
    source: &str,
    contract: &ModulePublishValidationContract,
    validation: &mut ModulePublishBundleValidation,
) {
    let manifest = match source.parse::<toml::Table>() {
        Ok(manifest) => toml::Value::Table(manifest),
        Err(_) => {
            validation
                .errors
                .push("Artifact file rustok-module.toml is not valid TOML.".to_string());
            return;
        }
    };
    validate_toml_string(
        &manifest,
        &["module", "slug"],
        "rustok-module.toml [module].slug",
        &contract.slug,
        validation,
    );
    validate_toml_string(
        &manifest,
        &["module", "name"],
        "rustok-module.toml [module].name",
        &contract.module_name,
        validation,
    );
    validate_toml_string(
        &manifest,
        &["module", "version"],
        "rustok-module.toml [module].version",
        &contract.version,
        validation,
    );
    validate_toml_string(
        &manifest,
        &["module", "description"],
        "rustok-module.toml [module].description",
        &contract.module_description,
        validation,
    );
    validate_toml_string(
        &manifest,
        &["module", "ownership"],
        "rustok-module.toml [module].ownership",
        &contract.ownership,
        validation,
    );
    validate_toml_string(
        &manifest,
        &["module", "trust_level"],
        "rustok-module.toml [module].trust_level",
        &contract.trust_level,
        validation,
    );
    validate_toml_optional(
        &manifest,
        &["marketplace", "category"],
        "rustok-module.toml [marketplace].category",
        contract.marketplace_category.as_deref(),
        validation,
    );
    validate_toml_optional(
        &manifest,
        &["crate", "entry_type"],
        "rustok-module.toml [crate].entry_type",
        contract.entry_type.as_deref(),
        validation,
    );
    if toml_string_list(&manifest, &["marketplace", "tags"])
        != normalize_string_list(&contract.marketplace_tags)
    {
        validation.errors.push(
            "Artifact file rustok-module.toml [marketplace].tags does not match the publish request."
                .to_string(),
        );
    }
    validate_toml_optional(
        &manifest,
        &["provides", "admin_ui", "leptos_crate"],
        "rustok-module.toml [provides.admin_ui].leptos_crate",
        contract.admin_ui_crate_name.as_deref(),
        validation,
    );
    validate_toml_optional(
        &manifest,
        &["provides", "storefront_ui", "leptos_crate"],
        "rustok-module.toml [provides.storefront_ui].leptos_crate",
        contract.storefront_ui_crate_name.as_deref(),
        validation,
    );
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

fn toml_string_list(value: &toml::Value, path: &[&str]) -> Vec<String> {
    toml_value_at_path(value, path)
        .and_then(toml::Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(|item| item.as_str().map(str::trim).map(ToString::to_string))
                .filter(|value| !value.is_empty())
                .collect::<Vec<_>>()
        })
        .map(|mut values| {
            values.sort();
            values.dedup();
            values
        })
        .unwrap_or_default()
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

fn validate_toml_optional(
    manifest: &toml::Value,
    path: &[&str],
    label: &str,
    expected: Option<&str>,
    validation: &mut ModulePublishBundleValidation,
) {
    let expected = expected
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string);
    if toml_string(manifest, path) != expected {
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

    #[test]
    fn oversized_untrusted_artifact_text_never_enters_validation_diagnostics() {
        let marker = "<untrusted-prompt-injection-marker>";
        let source = marker.repeat(MODULE_PUBLISH_ARTIFACT_MANIFEST_MAX_BYTES / marker.len() + 1);
        let mut validation = ModulePublishBundleValidation::default();

        require_file("Cargo.toml", Some(&source), &mut validation);

        assert!(!validation.errors.is_empty());
        assert!(validation
            .errors
            .iter()
            .all(|diagnostic| !diagnostic.contains(marker)));
    }
}
