//! Pure renderer for the canonical Rust module component source tree.

use rustok_modules::{
    MODULE_ARTIFACT_DESCRIPTOR_SCHEMA_VERSION, MODULE_ARTIFACT_SOURCE_MANIFEST_FILE,
    MODULE_BUILD_COMPONENT_TARGET, MODULE_BUILD_RUNTIME_ABI, ModuleArtifactSourceManifest,
};
use serde_json::{Value, json};
use thiserror::Error;

const JSON_SCHEMA_DRAFT_2020_12: &str = "https://json-schema.org/draft/2020-12/schema";
const CARGO_TEMPLATE: &str = include_str!("../assets/Cargo.toml.template");
const LIB_TEMPLATE: &str = include_str!("../assets/src/lib.rs.template");
const README_TEMPLATE: &str = include_str!("../assets/README.md.template");
const TOOLCHAIN_TEMPLATE: &str = include_str!("../assets/rust-toolchain.toml.template");
const BUILD_POLICY: &str = include_str!("../assets/module-build-policy.toml");
const CONTRACT_TEST: &str = include_str!("../assets/tests/contract.rs");
const SANDBOX_SCENARIO_TEMPLATE: &str =
    include_str!("../assets/tests/sandbox-scenario.json.template");
const EN_LOCALE_TEMPLATE: &str = include_str!("../assets/locales/en.json.template");

/// Independently released template identity recorded in build provenance.
pub const TEMPLATE_VERSION: &str = env!("CARGO_PKG_VERSION");
/// Exact native Rust toolchain selected by the current build protocol.
pub const RUST_TOOLCHAIN: &str = "1.96.0";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModuleTemplateInput {
    pub slug: String,
    pub version: String,
    pub display_name: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TemplateFile {
    pub path: &'static str,
    pub contents: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenderedModule {
    files: Vec<TemplateFile>,
}

impl RenderedModule {
    pub fn files(&self) -> &[TemplateFile] {
        &self.files
    }

    pub fn file(&self, path: &str) -> Option<&[u8]> {
        self.files
            .iter()
            .find(|file| file.path == path)
            .map(|file| file.contents.as_slice())
    }
}

#[derive(Debug, Error)]
pub enum ModuleTemplateError {
    #[error("module display name must be a bounded printable value")]
    InvalidDisplayName,
    #[error("rendered source manifest is invalid: {0}")]
    InvalidSourceManifest(String),
    #[error("rendered local sandbox scenario is invalid: {0}")]
    InvalidSandboxScenario(String),
    #[error("template serialization failed: {0}")]
    Serialization(#[from] serde_json::Error),
    #[error("template still contains an unresolved placeholder")]
    UnresolvedPlaceholder,
}

pub fn render(input: &ModuleTemplateInput) -> Result<RenderedModule, ModuleTemplateError> {
    validate_display_name(&input.display_name)?;
    let package_name = input.slug.replace('_', "-");
    let replacements = [
        ("{{slug}}", input.slug.as_str()),
        ("{{package_name}}", package_name.as_str()),
        ("{{version}}", input.version.as_str()),
        ("{{display_name}}", input.display_name.as_str()),
        ("{{sdk_version}}", rustok_module_sdk::SDK_VERSION),
        ("{{template_version}}", TEMPLATE_VERSION),
        ("{{rust_toolchain}}", RUST_TOOLCHAIN),
        ("{{component_target}}", MODULE_BUILD_COMPONENT_TARGET),
        ("{{runtime_abi}}", MODULE_BUILD_RUNTIME_ABI),
        ("{{wit_world}}", rustok_modules::MODULE_BUILD_WIT_WORLD),
        ("{{wit_version}}", rustok_modules::MODULE_BUILD_WIT_VERSION),
    ];

    let source_manifest = render_source_manifest(input)?;
    let sandbox_scenario = render_text(SANDBOX_SCENARIO_TEMPLATE, &replacements)?;
    rustok_sandbox::LocalSandboxScenario::parse(sandbox_scenario.as_bytes())
        .map_err(|error| ModuleTemplateError::InvalidSandboxScenario(error.to_string()))?;
    let files = vec![
        text_file("Cargo.toml", render_text(CARGO_TEMPLATE, &replacements)?),
        text_file("README.md", render_text(README_TEMPLATE, &replacements)?),
        text_file("src/lib.rs", render_text(LIB_TEMPLATE, &replacements)?),
        text_file(
            "rust-toolchain.toml",
            render_text(TOOLCHAIN_TEMPLATE, &replacements)?,
        ),
        text_file("module-build-policy.toml", BUILD_POLICY.to_string()),
        text_file("tests/contract.rs", CONTRACT_TEST.to_string()),
        text_file("tests/sandbox-scenario.json", sandbox_scenario),
        text_file(
            "locales/en.json",
            render_text(EN_LOCALE_TEMPLATE, &replacements)?,
        ),
        TemplateFile {
            path: MODULE_ARTIFACT_SOURCE_MANIFEST_FILE,
            contents: source_manifest,
        },
    ];
    Ok(RenderedModule { files })
}

fn render_source_manifest(input: &ModuleTemplateInput) -> Result<Vec<u8>, ModuleTemplateError> {
    let input_schema = json!({
        "$schema": JSON_SCHEMA_DRAFT_2020_12,
        "title": format!("{} command input", input.display_name),
        "type": "object",
        "properties": {
            "message": { "type": "string", "maxLength": 4096 }
        },
        "required": ["message"],
        "additionalProperties": false
    });
    let output_schema = json!({
        "$schema": JSON_SCHEMA_DRAFT_2020_12,
        "title": format!("{} command output", input.display_name),
        "type": "object",
        "properties": {
            "message": { "type": "string", "maxLength": 4096 }
        },
        "required": ["message"],
        "additionalProperties": false
    });
    let settings_schema = json!({
        "$schema": JSON_SCHEMA_DRAFT_2020_12,
        "title": format!("{} settings", input.display_name),
        "type": "object",
        "properties": {
            "enabled": { "type": "boolean", "default": true }
        },
        "additionalProperties": false
    });
    let input_digest = schema_digest(&input_schema);
    let output_digest = schema_digest(&output_schema);
    let settings_digest = schema_digest(&settings_schema);
    let permission = format!("{}.execute", input.slug);
    let manifest = json!({
        "schema_version": MODULE_ARTIFACT_DESCRIPTOR_SCHEMA_VERSION,
        "slug": input.slug,
        "version": input.version,
        "payload_kind": "wasm_component",
        "module_kind": "optional",
        "runtime_abi": MODULE_BUILD_RUNTIME_ABI,
        "platform_compatibility": "^0.1",
        "required_features": [],
        "entrypoint": "run",
        "capabilities": ["platform.events"],
        "bindings": [{
            "id": "execute",
            "kind": "command",
            "entrypoint": "run",
            "input_schema_digest": input_digest,
            "output_schema_digest": output_digest,
            "permission": permission,
            "idempotency": "required",
            "limit_profile": "command",
            "capabilities": ["platform.events"]
        }],
        "dependencies": [],
        "permissions": [{
            "key": permission,
            "localizations": [{
                "locale": "en",
                "label": format!("Execute {}", input.display_name),
                "description": format!("Run the {} module command", input.display_name)
            }]
        }],
        "schema_documents": [
            { "digest": input_digest, "document": input_schema },
            { "digest": output_digest, "document": output_schema },
            { "digest": settings_digest, "document": settings_schema }
        ],
        "settings_schema_digest": settings_digest,
        "ui_contributions": []
    });
    let bytes = serde_json::to_vec(&manifest)?;
    let source = ModuleArtifactSourceManifest::parse(&bytes)
        .map_err(|error| ModuleTemplateError::InvalidSourceManifest(error.to_string()))?;
    source
        .to_json_bytes()
        .map_err(|error| ModuleTemplateError::InvalidSourceManifest(error.to_string()))
}

fn schema_digest(schema: &Value) -> String {
    rustok_modules::canonical_schema_digest(schema)
}

fn render_text(
    template: &str,
    replacements: &[(&str, &str)],
) -> Result<String, ModuleTemplateError> {
    let rendered = replacements
        .iter()
        .fold(template.to_string(), |text, (key, value)| {
            text.replace(key, value)
        });
    if rendered.contains("{{") || rendered.contains("}}") {
        return Err(ModuleTemplateError::UnresolvedPlaceholder);
    }
    Ok(rendered)
}

fn validate_display_name(value: &str) -> Result<(), ModuleTemplateError> {
    if value.trim().is_empty() || value.len() > 80 || value.contains(char::is_control) {
        return Err(ModuleTemplateError::InvalidDisplayName);
    }
    Ok(())
}

fn text_file(path: &'static str, contents: String) -> TemplateFile {
    TemplateFile {
        path,
        contents: contents.into_bytes(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn input() -> ModuleTemplateInput {
        ModuleTemplateInput {
            slug: "sample_module".to_string(),
            version: "1.0.0".to_string(),
            display_name: "Sample Module".to_string(),
        }
    }

    #[test]
    fn rendered_project_uses_native_wasi_p2_and_exact_sdk_identity() {
        let rendered = render(&input()).expect("render template");
        let cargo = std::str::from_utf8(rendered.file("Cargo.toml").expect("Cargo.toml"))
            .expect("UTF-8 Cargo.toml");
        let toolchain = std::str::from_utf8(
            rendered
                .file("rust-toolchain.toml")
                .expect("rust-toolchain.toml"),
        )
        .expect("UTF-8 toolchain");
        assert!(cargo.contains(&format!(
            "rustok-module-sdk = {{ version = \"={}\" }}",
            rustok_module_sdk::SDK_VERSION
        )));
        assert!(toolchain.contains("wasm32-wasip2"));
        assert!(!cargo.contains("cargo-component"));
        let source = std::str::from_utf8(rendered.file("src/lib.rs").expect("source"))
            .expect("UTF-8 source");
        assert!(source.contains("module.sample_module.executed"));
        assert!(source.contains("\\\"topic\\\""));
    }

    #[test]
    fn rendered_source_manifest_is_owner_validated_and_has_no_payload_digest() {
        let rendered = render(&input()).expect("render template");
        let bytes = rendered
            .file(MODULE_ARTIFACT_SOURCE_MANIFEST_FILE)
            .expect("source manifest");
        let source = ModuleArtifactSourceManifest::parse(bytes).expect("owner-valid manifest");
        assert!(!source.to_json_bytes().expect("serialize").is_empty());
        assert!(!String::from_utf8_lossy(bytes).contains("artifact_digest"));
    }

    #[test]
    fn invalid_identity_fails_before_files_are_returned() {
        let mut invalid = input();
        invalid.slug = "Sample-Module".to_string();
        assert!(matches!(
            render(&invalid),
            Err(ModuleTemplateError::InvalidSourceManifest(_))
        ));
    }

    #[test]
    fn rendered_sandbox_scenario_matches_the_brokered_event_example() {
        let rendered = render(&input()).expect("render template");
        let scenario: serde_json::Value = serde_json::from_slice(
            rendered
                .file("tests/sandbox-scenario.json")
                .expect("sandbox scenario"),
        )
        .expect("valid scenario JSON");
        assert_eq!(
            scenario["policy"]["grants"][0]["constraints"]["topics"][0],
            "module.sample_module.executed"
        );
        assert_eq!(scenario["expectation"]["output"], scenario["input"]);
    }
}
