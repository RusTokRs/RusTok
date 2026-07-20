use thiserror::Error;

use rustok_modules::{
    ArtifactAdmissionLimits, ArtifactOrigin, ArtifactPayloadKind, ArtifactRelease,
    ArtifactReleaseDraft, ArtifactSourceLineage, ModuleArtifactDescriptor, ModuleArtifactError,
    ModuleArtifactPackage, OciArtifactReference, MODULE_ARTIFACT_DESCRIPTOR_SCHEMA_VERSION,
};
use rustok_sandbox::CapabilityName;

use crate::Script;

pub(crate) const RHAI_MODULE_ABI: &str = "rustok:module/runtime@1";

#[derive(Debug, Error)]
pub enum AlloyArtifactError {
    #[error(transparent)]
    Module(#[from] ModuleArtifactError),
    #[error("Alloy source cannot be released as module `{slug}`: {message}")]
    InvalidRelease { slug: String, message: String },
    #[error("release `{slug}@{version}` is not a Rhai module artifact")]
    NotRhaiRelease { slug: String, version: String },
    #[error("Rhai module package is invalid: {0}")]
    InvalidPackage(String),
}

/// Stages an immutable Rhai module artifact from an Alloy source revision.
///
/// Capability grants are supplied by the review/policy layer rather than inferred
/// from the script's application permissions. A source-backed Rhai artifact uses
/// its canonical source digest as the payload digest until OCI packaging adds the
/// equivalent source layer to a manifest.
pub fn stage_rhai_module_release(
    module_slug: impl Into<String>,
    version: impl Into<String>,
    script: &Script,
    capabilities: Vec<CapabilityName>,
) -> Result<ArtifactReleaseDraft, AlloyArtifactError> {
    let module_slug = module_slug.into();
    let source_digest =
        script
            .workspace
            .digest()
            .map_err(|error| AlloyArtifactError::InvalidRelease {
                slug: module_slug.clone(),
                message: error.to_string(),
            })?;
    let descriptor = ModuleArtifactDescriptor {
        schema_version: MODULE_ARTIFACT_DESCRIPTOR_SCHEMA_VERSION,
        slug: module_slug.clone(),
        version: version.into(),
        payload_kind: ArtifactPayloadKind::Rhai,
        module_kind: rustok_modules::ArtifactModuleKind::Optional,
        runtime_abi: RHAI_MODULE_ABI.to_string(),
        platform_compatibility: "^0.1".to_string(),
        required_features: Vec::new(),
        artifact_digest: source_digest.clone(),
        entrypoint: script.workspace.entrypoint.clone(),
        capabilities,
        bindings: Vec::new(),
        dependencies: Vec::new(),
        permissions: Vec::new(),
        schema_documents: Vec::new(),
        settings_schema_digest: None,
        data_schema_digest: None,
        ui_contributions: Vec::new(),
        persistence_contract: None,
    };
    descriptor
        .validate()
        .map_err(|error| AlloyArtifactError::InvalidRelease {
            slug: module_slug,
            message: error.to_string(),
        })?;

    Ok(ArtifactReleaseDraft {
        descriptor,
        lineage: ArtifactSourceLineage {
            origin: ArtifactOrigin::AlloyDraft,
            source_digest,
            parent_release: None,
        },
    })
}

/// Starts the next immutable Rhai release from a marketplace release lineage.
pub fn fork_rhai_module_release(
    parent: &ArtifactRelease,
    version: impl Into<String>,
    script: &Script,
    capabilities: Vec<CapabilityName>,
) -> Result<ArtifactReleaseDraft, AlloyArtifactError> {
    if parent.descriptor.payload_kind != ArtifactPayloadKind::Rhai {
        return Err(AlloyArtifactError::NotRhaiRelease {
            slug: parent.descriptor.slug.clone(),
            version: parent.descriptor.version.clone(),
        });
    }

    let draft = stage_rhai_module_release(
        parent.descriptor.slug.clone(),
        version,
        script,
        capabilities,
    )?;
    parent
        .fork(draft.descriptor, draft.lineage.source_digest)
        .map_err(AlloyArtifactError::from)
}

/// Packages reviewed Alloy source as the immutable OCI payload selected by an
/// already-staged module release. The caller supplies a digest-pinned OCI
/// manifest location; the descriptor separately pins the source payload layer.
pub async fn package_rhai_module_release(
    reference: OciArtifactReference,
    draft: &ArtifactReleaseDraft,
    script: &Script,
) -> Result<ModuleArtifactPackage, AlloyArtifactError> {
    if draft.descriptor.payload_kind != ArtifactPayloadKind::Rhai {
        return Err(AlloyArtifactError::InvalidRelease {
            slug: draft.descriptor.slug.clone(),
            message: "only Rhai release drafts can be packaged from Alloy source".to_string(),
        });
    }
    let package = ModuleArtifactPackage {
        reference,
        descriptor: draft.descriptor.clone(),
        media_type: crate::ALLOY_DRAFT_RHAI_MEDIA_TYPE.to_string(),
        payload: rustok_modules::ArtifactPayloadSource::Bytes(
            script
                .workspace
                .canonical_bytes()
                .map_err(|error| AlloyArtifactError::InvalidPackage(error.to_string()))?,
        ),
    };
    package
        .verify(ArtifactAdmissionLimits::default())
        .await
        .map_err(|error| AlloyArtifactError::InvalidPackage(error.to_string()))?;
    Ok(package)
}

#[cfg(test)]
mod tests {
    use chrono::Utc;
    use rustok_modules::ArtifactPayloadKind;
    use rustok_sandbox::CapabilityName;

    use super::{fork_rhai_module_release, package_rhai_module_release, stage_rhai_module_release};
    use crate::{AlloyWorkspace, Script, ScriptTrigger};

    fn script(code: &str) -> Script {
        Script::new(
            "tax_adjustment",
            AlloyWorkspace::single_source(code),
            ScriptTrigger::Manual,
        )
    }

    #[test]
    fn reviewed_rhai_source_stages_as_a_module_artifact() {
        let draft = stage_rhai_module_release(
            "tax_adjustment",
            "1.0.0",
            &script("input.total * 0.2"),
            vec![CapabilityName::new("platform.events").expect("capability")],
        )
        .expect("stage release");

        assert_eq!(draft.descriptor.payload_kind, ArtifactPayloadKind::Rhai);
        assert_eq!(
            draft.descriptor.artifact_digest,
            draft.lineage.source_digest
        );
        assert_eq!(draft.descriptor.runtime_abi, "rustok:module/runtime@1");
    }

    #[test]
    fn editing_a_marketplace_rhai_release_creates_new_lineage() {
        let original = stage_rhai_module_release(
            "tax_adjustment",
            "1.0.0",
            &script("input.total * 0.2"),
            Vec::new(),
        )
        .expect("stage original")
        .publish(Utc::now())
        .expect("publish original");

        let revision = fork_rhai_module_release(
            &original,
            "1.1.0",
            &script("input.total * 0.21"),
            Vec::new(),
        )
        .expect("fork release")
        .publish(Utc::now())
        .expect("publish revision");

        assert_eq!(
            revision.lineage.parent_release,
            Some(original.descriptor.release_ref())
        );
        assert_ne!(
            revision.descriptor.artifact_digest,
            original.descriptor.artifact_digest
        );
    }

    #[tokio::test]
    async fn reviewed_rhai_source_packages_at_a_digest_pinned_oci_reference() {
        let source = script("input.total * 0.2");
        let draft = stage_rhai_module_release("tax_adjustment", "1.0.0", &source, Vec::new())
            .expect("stage release");
        let package = package_rhai_module_release(
            rustok_modules::OciArtifactReference {
                registry: "registry.example".to_string(),
                repository: "modules/tax_adjustment".to_string(),
                digest: format!("sha256:{}", "c".repeat(64)),
            },
            &draft,
            &source,
        )
        .await
        .expect("package release");

        assert_ne!(package.reference.digest, draft.descriptor.artifact_digest);
        assert!(matches!(
            package.payload,
            rustok_modules::ArtifactPayloadSource::Bytes(payload)
                if payload == source.workspace.canonical_bytes().expect("workspace bytes")
        ));
    }
}
