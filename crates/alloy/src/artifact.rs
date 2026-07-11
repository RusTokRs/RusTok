use sha2::{Digest, Sha256};
use thiserror::Error;

use rustok_modules::{
    ArtifactOrigin, ArtifactPayloadKind, ArtifactRelease, ArtifactReleaseDraft,
    ArtifactSourceLineage, ModuleArtifactDescriptor, ModuleArtifactError,
};
use rustok_sandbox::CapabilityName;

use crate::Script;

const RHAI_MODULE_ABI: &str = "rustok:module/runtime@1";

#[derive(Debug, Error)]
pub enum AlloyArtifactError {
    #[error(transparent)]
    Module(#[from] ModuleArtifactError),
    #[error("Alloy source cannot be released as module `{slug}`: {message}")]
    InvalidRelease { slug: String, message: String },
    #[error("release `{slug}@{version}` is not a Rhai module artifact")]
    NotRhaiRelease { slug: String, version: String },
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
    let source_digest = sha256_digest(script.code.as_bytes());
    let descriptor = ModuleArtifactDescriptor {
        slug: module_slug.clone(),
        version: version.into(),
        payload_kind: ArtifactPayloadKind::Rhai,
        runtime_abi: RHAI_MODULE_ABI.to_string(),
        artifact_digest: source_digest.clone(),
        entrypoint: "main".to_string(),
        capabilities,
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

fn sha256_digest(bytes: &[u8]) -> String {
    format!("sha256:{}", hex::encode(Sha256::digest(bytes)))
}

#[cfg(test)]
mod tests {
    use chrono::Utc;
    use rustok_modules::ArtifactPayloadKind;
    use rustok_sandbox::CapabilityName;

    use super::{fork_rhai_module_release, stage_rhai_module_release};
    use crate::{Script, ScriptTrigger};

    fn script(code: &str) -> Script {
        Script::new(
            "tax_adjustment",
            code,
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
        assert_eq!(draft.descriptor.artifact_digest, draft.lineage.source_digest);
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

        assert_eq!(revision.lineage.parent_release, Some(original.descriptor.release_ref()));
        assert_ne!(revision.descriptor.artifact_digest, original.descriptor.artifact_digest);
    }
}
