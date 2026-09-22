use async_trait::async_trait;
use rustok_modules::{
    ArtifactInstallationResolver, ArtifactReleaseRef, ArtifactSandboxPolicyResolver,
    ModuleControlPlane, ModuleGovernanceErrorCategory,
};
use rustok_storage::StorageRuntime;
use sea_orm::DatabaseConnection;
use uuid::Uuid;

/// Server composition adapter for one exact published Rhai release. Registry
/// metadata and workspace bytes are resolved through the module owner; this
/// adapter cannot read catalog DTOs, upload objects, or mutable OCI tags.
#[derive(Clone)]
pub(crate) struct ServerAlloyPublishedRhaiSourceProvider {
    db: DatabaseConnection,
    storage: StorageRuntime,
}

impl ServerAlloyPublishedRhaiSourceProvider {
    pub(crate) fn new(db: DatabaseConnection, storage: StorageRuntime) -> Self {
        Self { db, storage }
    }
}

#[async_trait]
impl alloy::AlloyPublishedRhaiSourceProvider for ServerAlloyPublishedRhaiSourceProvider {
    async fn load_published_rhai_source(
        &self,
        release: &ArtifactReleaseRef,
    ) -> Result<alloy::AlloyPublishedRhaiSource, alloy::AlloyImportError> {
        let control_plane = ModuleControlPlane::new(self.db.clone());
        let blobs = control_plane.artifact_blob_store(self.storage.clone());
        let source = control_plane
            .release()
            .published_rhai_workspace(release, &blobs)
            .await
            .map_err(|error| match error.category() {
                ModuleGovernanceErrorCategory::NotFound => {
                    alloy::AlloyImportError::SourceUnavailable(
                        "the canonical published Rhai workspace is unavailable".to_string(),
                    )
                }
                ModuleGovernanceErrorCategory::InvalidInput
                | ModuleGovernanceErrorCategory::PermissionDenied
                | ModuleGovernanceErrorCategory::Conflict => {
                    alloy::AlloyImportError::IneligibleRelease
                }
                ModuleGovernanceErrorCategory::Internal => alloy::AlloyImportError::Storage(
                    "published Rhai workspace resolution failed".to_string(),
                ),
            })?;
        Ok(alloy::AlloyPublishedRhaiSource {
            release: source.release,
            workspace: source.workspace,
        })
    }
}

/// Server composition adapter for the capability policy of one exact parent
/// artifact installed for the draft tenant. The module owner retains release,
/// admission, lifecycle, policy revision, and descriptor validation.
#[derive(Clone)]
pub(crate) struct ServerAlloyImportedDraftPolicyProvider {
    db: DatabaseConnection,
}

impl ServerAlloyImportedDraftPolicyProvider {
    pub(crate) fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }
}

#[async_trait]
impl alloy::AlloyImportedDraftPolicyProvider for ServerAlloyImportedDraftPolicyProvider {
    async fn resolve_policy(
        &self,
        tenant_id: Uuid,
        parent_release: &ArtifactReleaseRef,
    ) -> Result<rustok_sandbox::SandboxPolicy, alloy::AlloyImportedDraftPolicyError> {
        let control_plane = ModuleControlPlane::new(self.db.clone());
        let installation = ArtifactInstallationResolver::resolve(
            &control_plane.installation(),
            parent_release,
            tenant_id,
        )
        .await
        .map_err(|_| alloy::AlloyImportedDraftPolicyError::Unavailable)?;
        ArtifactSandboxPolicyResolver::resolve(
            &control_plane.artifact_sandbox_policy(),
            &installation,
            tenant_id,
        )
        .await
        .map_err(|_| alloy::AlloyImportedDraftPolicyError::Unavailable)
    }
}
