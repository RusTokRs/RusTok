//! Tenant-bound MCP contract for importing an immutable published Alloy release.

use std::sync::Arc;

use alloy::storage::ScriptRegistry;
use alloy::{
    AlloyImportError, AlloyPublishedReleaseImportCommand, AlloyPublishedRhaiSourceProviderHandle,
    AlloyReleaseImporter,
};
use rustok_modules::ArtifactReleaseRef;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// MCP tool name for a tenant-bound import of an exact published Rhai release.
/// The server remote transport composes this tool only after it has resolved a
/// durable MCP tenant binding and owner-backed source provider.
pub const TOOL_ALLOY_IMPORT_PUBLISHED_RELEASE: &str = "alloy_import_published_release";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct AlloyPublishedReleaseRef {
    pub slug: String,
    pub version: String,
    pub digest: String,
}

impl From<AlloyPublishedReleaseRef> for ArtifactReleaseRef {
    fn from(release: AlloyPublishedReleaseRef) -> Self {
        Self {
            slug: release.slug,
            version: release.version,
            digest: release.digest,
        }
    }
}

impl From<ArtifactReleaseRef> for AlloyPublishedReleaseRef {
    fn from(release: ArtifactReleaseRef) -> Self {
        Self {
            slug: release.slug,
            version: release.version,
            digest: release.digest,
        }
    }
}

/// Untrusted MCP arguments. Tenant and actor identity are deliberately absent:
/// the remote host derives both from its authenticated MCP runtime binding.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct AlloyPublishedReleaseImportRequest {
    pub release: AlloyPublishedReleaseRef,
    pub draft_name: String,
    pub idempotency_key: Uuid,
}

/// Redacted draft identity returned by a successful or replayed MCP import.
/// The source workspace is not echoed through the tool response.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct AlloyPublishedReleaseImportResponse {
    pub script_id: Uuid,
    pub draft_name: String,
    pub revision: u32,
    pub parent_release: AlloyPublishedReleaseRef,
    pub created: bool,
}

/// Imports one immutable release through a supplied owner-backed source
/// provider and an already tenant-scoped Alloy registry. Callers must derive
/// `tenant_id` and `actor_id` from authenticated MCP runtime state.
pub async fn import_published_release<R>(
    registry: Arc<R>,
    source: AlloyPublishedRhaiSourceProviderHandle,
    tenant_id: Uuid,
    actor_id: String,
    request: AlloyPublishedReleaseImportRequest,
) -> Result<AlloyPublishedReleaseImportResponse, AlloyImportError>
where
    R: ScriptRegistry,
{
    let release: ArtifactReleaseRef = request.release.into();
    let imported = AlloyReleaseImporter::new(registry, source.0)
        .import(AlloyPublishedReleaseImportCommand {
            tenant_id,
            release,
            draft_name: request.draft_name,
            actor_id,
            idempotency_key: request.idempotency_key,
        })
        .await?;
    let parent_release = imported
        .script
        .parent_release
        .clone()
        .ok_or(AlloyImportError::InvalidSource)?;
    Ok(AlloyPublishedReleaseImportResponse {
        script_id: imported.script.id,
        draft_name: imported.script.name,
        revision: imported.script.version,
        parent_release: parent_release.into(),
        created: imported.created,
    })
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use async_trait::async_trait;
    use chrono::Utc;

    use super::*;
    use alloy::{
        AlloyPublishedRhaiSource, AlloyPublishedRhaiSourceProvider, InMemoryStorage, RhaiWorkspace,
        RhaiWorkspaceFile, RhaiWorkspaceFileKind, Script, ScriptRegistry, ScriptTrigger,
    };
    use rustok_modules::{
        ArtifactOrigin, ArtifactRelease, ArtifactSourceLineage,
        MODULE_ARTIFACT_DESCRIPTOR_SCHEMA_VERSION, ModuleArtifactSourceManifest,
    };

    const RHAI_SOURCE_MANIFEST_PATH: &str = "policy/module-artifact.json";

    #[derive(Clone)]
    struct FixedSourceProvider {
        source: AlloyPublishedRhaiSource,
    }

    #[async_trait]
    impl AlloyPublishedRhaiSourceProvider for FixedSourceProvider {
        async fn load_published_rhai_source(
            &self,
            release: &ArtifactReleaseRef,
        ) -> Result<AlloyPublishedRhaiSource, AlloyImportError> {
            if self.source.release.descriptor.release_ref() != *release {
                return Err(AlloyImportError::SourceUnavailable(
                    "release was not found".to_string(),
                ));
            }
            Ok(self.source.clone())
        }
    }

    fn published_source() -> AlloyPublishedRhaiSource {
        let mut script = Script::new(
            "published_tax_rule",
            RhaiWorkspace::single_source("40 + 2"),
            ScriptTrigger::Manual,
        );
        script.workspace.files.push(RhaiWorkspaceFile {
            path: RHAI_SOURCE_MANIFEST_PATH.to_string(),
            kind: RhaiWorkspaceFileKind::Policy,
            contents: serde_json::json!({
                "schema_version": MODULE_ARTIFACT_DESCRIPTOR_SCHEMA_VERSION,
                "slug": "tax_rule",
                "version": "1.0.0",
                "payload_kind": "rhai",
                "module_kind": "optional",
                "runtime_abi": rustok_sandbox::RHAI_SANDBOX_RUNTIME_ABI,
                "platform_compatibility": "^0.1",
                "entrypoint": "src/main.rhai",
            })
            .to_string(),
        });
        let source_digest = script.workspace.digest().expect("source digest");
        let source_manifest = script
            .workspace
            .files
            .iter()
            .find(|file| file.path == RHAI_SOURCE_MANIFEST_PATH)
            .expect("source manifest");
        let descriptor = ModuleArtifactSourceManifest::parse(source_manifest.contents.as_bytes())
            .expect("source manifest must be valid")
            .finalize(source_digest.clone())
            .expect("release descriptor must be finalized");
        let release = ArtifactRelease {
            descriptor,
            lineage: ArtifactSourceLineage {
                origin: ArtifactOrigin::Marketplace,
                source_digest,
                parent_release: None,
            },
            published_at: Utc::now(),
        };
        AlloyPublishedRhaiSource {
            release,
            workspace: script.workspace,
        }
    }

    #[tokio::test]
    async fn import_preserves_the_exact_parent_release_without_echoing_source() {
        let source = published_source();
        let parent_release = source.release.descriptor.release_ref();
        let registry = Arc::new(InMemoryStorage::new());
        let response = import_published_release(
            registry.clone(),
            AlloyPublishedRhaiSourceProviderHandle(Arc::new(FixedSourceProvider { source })),
            Uuid::new_v4(),
            "mcp-client-42".to_string(),
            AlloyPublishedReleaseImportRequest {
                release: parent_release.clone().into(),
                draft_name: "imported_tax_rule".to_string(),
                idempotency_key: Uuid::new_v4(),
            },
        )
        .await
        .expect("published release import");

        assert!(response.created);
        assert_eq!(response.parent_release, parent_release.clone().into());
        assert_eq!(response.draft_name, "imported_tax_rule");
        let script = registry
            .get(response.script_id)
            .await
            .expect("imported draft");
        assert_eq!(script.parent_release, Some(parent_release));
    }
}
