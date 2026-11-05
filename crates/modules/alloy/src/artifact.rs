use thiserror::Error;

use rustok_modules::{
    ModuleArtifactDescriptor, ModuleArtifactSourceManifest, ModuleArtifactSourceManifestError,
};
use rustok_sandbox::{RhaiWorkspace, RhaiWorkspaceCapabilityError, RhaiWorkspaceError};

/// The module-owned declaration is part of the immutable Rhai workspace, so
/// review, source digest, descriptor finalization, and the executable payload
/// all select the same revision. `policy/` is a non-executable workspace area;
/// the sandbox never interprets this file as guest code.
pub const RHAI_MODULE_SOURCE_MANIFEST_PATH: &str = "policy/module-artifact.json";

#[derive(Debug, Error)]
pub enum AlloyArtifactError {
    #[error("Alloy Rhai workspace is invalid: {0}")]
    Workspace(#[source] RhaiWorkspaceError),
    #[error("Alloy Rhai workspace is missing `{RHAI_MODULE_SOURCE_MANIFEST_PATH}`")]
    MissingSourceManifest,
    #[error("Alloy Rhai release declaration is invalid: {0}")]
    SourceManifest(#[source] ModuleArtifactSourceManifestError),
    #[error("Alloy Rhai capability declaration is invalid: {0}")]
    Capabilities(#[source] RhaiWorkspaceCapabilityError),
}

/// Finalizes the one descriptor that may accompany an approved Alloy Rhai
/// revision. The author-controlled declaration is parsed from the same
/// canonical workspace bytes that become the payload. Its digest is finalized
/// only after the full workspace and the exact declared capability set have
/// been validated.
pub(crate) fn prepare_rhai_module_descriptor(
    workspace: &RhaiWorkspace,
) -> Result<ModuleArtifactDescriptor, AlloyArtifactError> {
    workspace
        .validate()
        .map_err(AlloyArtifactError::Workspace)?;
    let manifest = workspace
        .files
        .iter()
        .find(|file| file.path == RHAI_MODULE_SOURCE_MANIFEST_PATH)
        .ok_or(AlloyArtifactError::MissingSourceManifest)?;
    let manifest = ModuleArtifactSourceManifest::parse(manifest.contents.as_bytes())
        .map_err(AlloyArtifactError::SourceManifest)?;
    manifest
        .validate_rhai_declaration(&workspace.entrypoint)
        .map_err(AlloyArtifactError::SourceManifest)?;
    workspace
        .validate_declared_capabilities(manifest.capabilities())
        .map_err(AlloyArtifactError::Capabilities)?;
    let source_digest = workspace.digest().map_err(AlloyArtifactError::Workspace)?;
    manifest
        .finalize(source_digest)
        .map_err(AlloyArtifactError::SourceManifest)
}

#[cfg(test)]
mod tests {
    use rustok_modules::MODULE_ARTIFACT_DESCRIPTOR_SCHEMA_VERSION;
    use rustok_sandbox::{RhaiWorkspace, RhaiWorkspaceFile, RhaiWorkspaceFileKind};

    use super::{
        AlloyArtifactError, RHAI_MODULE_SOURCE_MANIFEST_PATH, prepare_rhai_module_descriptor,
    };

    fn workspace(source: &str, capabilities: serde_json::Value) -> RhaiWorkspace {
        RhaiWorkspace {
            schema_version: 1,
            entrypoint: "src/main.rhai".to_string(),
            files: vec![
                RhaiWorkspaceFile {
                    path: "src/main.rhai".to_string(),
                    kind: RhaiWorkspaceFileKind::Source,
                    contents: source.to_string(),
                },
                RhaiWorkspaceFile {
                    path: RHAI_MODULE_SOURCE_MANIFEST_PATH.to_string(),
                    kind: RhaiWorkspaceFileKind::Policy,
                    contents: serde_json::json!({
                        "schema_version": MODULE_ARTIFACT_DESCRIPTOR_SCHEMA_VERSION,
                        "slug": "tax_adjustment",
                        "version": "1.0.0",
                        "payload_kind": "rhai",
                        "module_kind": "optional",
                        "runtime_abi": rustok_sandbox::RHAI_SANDBOX_RUNTIME_ABI,
                        "platform_compatibility": "^0.1",
                        "entrypoint": "src/main.rhai",
                        "capabilities": capabilities,
                    })
                    .to_string(),
                },
            ],
        }
    }

    #[test]
    fn release_descriptor_is_finalized_from_the_reviewed_workspace() {
        let workspace = workspace(
            "capability_call(\"platform.events\", \"emit\", #{})",
            serde_json::json!(["platform.events"]),
        );

        let descriptor = prepare_rhai_module_descriptor(&workspace).expect("release descriptor");

        assert_eq!(
            descriptor.artifact_digest,
            workspace.digest().expect("workspace digest")
        );
        assert_eq!(descriptor.entrypoint, workspace.entrypoint);
        assert!(descriptor.validate().is_ok());
    }

    #[test]
    fn release_descriptor_requires_the_exact_declared_capability_set() {
        let workspace = workspace(
            "http_get(\"https://example.test/health\")",
            serde_json::json!([]),
        );

        assert!(matches!(
            prepare_rhai_module_descriptor(&workspace),
            Err(AlloyArtifactError::Capabilities(
                rustok_sandbox::RhaiWorkspaceCapabilityError::CapabilityDeclarationMismatch { .. }
            ))
        ));
    }

    #[test]
    fn release_descriptor_rejects_invalid_rhai_declaration() {
        let workspace = workspace("40 + 2", serde_json::json!([]));

        assert!(matches!(
            rustok_modules::ModuleArtifactSourceManifest::parse(
                workspace.files[1].contents.as_bytes()
            )
            .expect("source manifest")
            .validate_rhai_release("different_module", "1.0.0", "src/main.rhai"),
            Err(rustok_modules::ModuleArtifactSourceManifestError::RhaiReleaseIdentityMismatch)
        ));
    }

    #[test]
    fn release_descriptor_requires_the_workspace_declaration() {
        let workspace = RhaiWorkspace::single_source("40 + 2");

        assert!(matches!(
            prepare_rhai_module_descriptor(&workspace),
            Err(AlloyArtifactError::MissingSourceManifest)
        ));
    }

    #[test]
    fn source_manifest_cannot_include_a_finalized_payload_digest() {
        let mut workspace = workspace("40 + 2", serde_json::json!([]));
        let manifest = workspace
            .files
            .iter_mut()
            .find(|file| file.path == RHAI_MODULE_SOURCE_MANIFEST_PATH)
            .expect("manifest");
        let mut declaration = serde_json::from_str::<serde_json::Value>(&manifest.contents)
            .expect("declaration JSON");
        declaration["artifact_digest"] = serde_json::json!(format!("sha256:{}", "a".repeat(64)));
        manifest.contents = declaration.to_string();

        assert!(matches!(
            prepare_rhai_module_descriptor(&workspace),
            Err(AlloyArtifactError::SourceManifest(
                rustok_modules::ModuleArtifactSourceManifestError::BuildDerivedDigest
            ))
        ));
    }
}
