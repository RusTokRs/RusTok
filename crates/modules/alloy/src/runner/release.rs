use std::sync::Arc;

use async_trait::async_trait;
use rustok_modules::{
    ModuleAlloyAuthoredStageCommand, ModuleAlloyAuthoredStageResult, ModuleGovernanceError,
    SeaOrmModuleGovernanceService,
};

use crate::model::{is_release_approved, review_evidence_digest, review_reference};
use crate::{
    AlloyDraftRuntime, AlloyReleaseError, AlloyReleaseStageCommand, ExecutionContext,
    ExecutionPhase, Script, ScriptRegistry, ScriptTrigger,
};

/// Owner boundary used by Alloy to stage reviewed source. Marketplace state is
/// never owned or written by Alloy; this trait allows a host transport adapter
/// to replace the direct owner-service composition when processes split.
#[async_trait]
pub trait AlloyReleaseGovernance: Send + Sync {
    async fn stage_alloy_authored(
        &self,
        command: ModuleAlloyAuthoredStageCommand,
    ) -> Result<ModuleAlloyAuthoredStageResult, ModuleGovernanceError>;
}

/// Host-provided owner boundary used by Alloy HTTP and GraphQL transports.
/// Alloy never owns marketplace persistence; the host injects this handle when
/// composing the module routes and schema.
#[derive(Clone)]
pub struct AlloyReleaseGovernanceHandle(pub Arc<dyn AlloyReleaseGovernance>);

#[async_trait]
impl<T> AlloyReleaseGovernance for Arc<T>
where
    T: AlloyReleaseGovernance + ?Sized,
{
    async fn stage_alloy_authored(
        &self,
        command: ModuleAlloyAuthoredStageCommand,
    ) -> Result<ModuleAlloyAuthoredStageResult, ModuleGovernanceError> {
        self.as_ref().stage_alloy_authored(command).await
    }
}

#[async_trait]
impl AlloyReleaseGovernance for SeaOrmModuleGovernanceService {
    async fn stage_alloy_authored(
        &self,
        command: ModuleAlloyAuthoredStageCommand,
    ) -> Result<ModuleAlloyAuthoredStageResult, ModuleGovernanceError> {
        SeaOrmModuleGovernanceService::stage_alloy_authored(self, command).await
    }
}

/// Selects immutable Alloy source and review evidence before invoking the
/// owner-owned module publication stage. A later source revision or a later
/// archived/rejected review cannot be substituted after this precondition.
pub struct RevisionedReleaseStager<R, G>
where
    R: ScriptRegistry,
    G: AlloyReleaseGovernance + ?Sized,
{
    registry: Arc<R>,
    governance: Arc<G>,
    runtime: AlloyDraftRuntime,
}

impl<R, G> RevisionedReleaseStager<R, G>
where
    R: ScriptRegistry,
    G: AlloyReleaseGovernance + ?Sized,
{
    pub fn new(runtime: AlloyDraftRuntime, registry: Arc<R>, governance: Arc<G>) -> Self {
        Self {
            registry,
            governance,
            runtime,
        }
    }

    pub async fn stage(
        &self,
        command: AlloyReleaseStageCommand,
    ) -> Result<ModuleAlloyAuthoredStageResult, AlloyReleaseError> {
        command.validate()?;
        let script = self
            .registry
            .get(command.script_id)
            .await
            .map_err(script_error_to_release)?;
        if script.version != command.expected_revision {
            return Err(AlloyReleaseError::StaleRevision {
                expected: command.expected_revision,
            });
        }
        let source = self
            .registry
            .get_source_revision(command.script_id, command.expected_revision)
            .await
            .map_err(script_error_to_release)?;
        if command.context.tenant_id != Some(source.tenant_id) {
            return Err(AlloyReleaseError::InvalidCommand);
        }
        if command.artifact_digest != source.source_digest {
            return Err(AlloyReleaseError::ArtifactSourceDigestMismatch);
        }
        let reviews = self
            .registry
            .list_reviews(command.script_id, command.expected_revision)
            .await
            .map_err(script_error_to_release)?;
        let review = reviews
            .last()
            .filter(|review| {
                review.script_id == source.script_id
                    && review.revision == source.revision
                    && review.source_digest == source.source_digest
                    && is_release_approved(review)
            })
            .ok_or(AlloyReleaseError::ReviewNotApproved)?;
        let mut smoke_script = Script::new(
            format!("publication-smoke:{}", source.script_id),
            source.workspace.clone(),
            ScriptTrigger::Manual,
        );
        smoke_script.id = source.script_id;
        smoke_script.tenant_id = source.tenant_id;
        smoke_script.version = source.revision;
        // A publication smoke for an imported fork uses the same exact
        // installed-parent policy gate as every other execution. The sandbox
        // still strips grants below, retaining only that policy's limits.
        smoke_script.parent_release = source.parent_release.clone();
        let mut smoke_context = ExecutionContext::new(ExecutionPhase::Manual)
            .with_tenant(source.tenant_id.to_string())
            .with_user(command.context.actor_id.to_string());
        // The release idempotency key is also the stable logical sandbox
        // execution identity, so a transport retry cannot manufacture a
        // different immutable owner command for the same release attempt.
        smoke_context.execution_id = command.context.idempotency_key;
        let smoke_evidence = self
            .runtime
            .execute_publication_smoke(&smoke_script, &smoke_context)
            .await
            .map_err(|error| AlloyReleaseError::SandboxSmokeFailed(error.to_string()))?;
        self.governance
            .stage_alloy_authored(ModuleAlloyAuthoredStageCommand {
                request_id: command.publish_request_id,
                expected_revision: command.expected_publish_request_revision,
                alloy_tenant_id: source.tenant_id,
                alloy_script_id: source.script_id,
                artifact_digest: command.artifact_digest,
                source_digest: source.source_digest,
                source_revision: source.revision,
                parent_release: source.parent_release.clone(),
                review_reference: review_reference(review),
                review_digest: review_evidence_digest(review)?,
                review_policy_revision: review.policy_revision.clone(),
                reviewed_by_principal: serde_json::json!({
                    "kind": "alloy_reviewer",
                    "id": review.actor_id,
                }),
                sandbox_execution_id: smoke_evidence.execution_id,
                sandbox_test_path: smoke_evidence.test_path,
                sandbox_scenario_digest: smoke_evidence.scenario_digest,
                sandbox_executor: smoke_evidence.executor,
                sandbox_runtime_abi: smoke_evidence.runtime_abi,
                sandbox_policy_digest: smoke_evidence.policy_digest,
                sandbox_capability_grants: smoke_evidence.capability_grants,
                context: command.context.clone(),
                actor_can_manage_modules: command.actor_can_manage_modules,
                actor_principal: serde_json::json!({
                    "kind": "user",
                    "id": command.context.actor_id,
                }),
            })
            .await
            .map_err(|error| match error {
                conflict @ ModuleGovernanceError::AlloyAuthoredStageIdempotencyConflict => {
                    AlloyReleaseError::GovernanceConflict(conflict.to_string())
                }
                not_found @ ModuleGovernanceError::PublishRequestNotFound => {
                    AlloyReleaseError::GovernanceNotFound(not_found.to_string())
                }
                error => AlloyReleaseError::Governance(error.to_string()),
            })
    }
}

fn script_error_to_release(error: crate::ScriptError) -> AlloyReleaseError {
    AlloyReleaseError::Governance(error.to_string())
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use async_trait::async_trait;
    use rustok_modules::{ArtifactReleaseRef, ModuleAlloyAuthoredStageResult};
    use rustok_sandbox::SandboxPolicy;
    use uuid::Uuid;

    use super::{AlloyReleaseGovernance, RevisionedReleaseStager};
    use crate::{
        AlloyImportedDraftPolicyError, AlloyImportedDraftPolicyProvider,
        AlloyImportedDraftPolicyProviderHandle, AlloyReleaseStageCommand, InMemoryStorage,
        ReviewCommand, ReviewStatus, RhaiWorkspace, RhaiWorkspaceFile, RhaiWorkspaceFileKind,
        Script, ScriptRegistry, ScriptTrigger, alloy_release_command_context,
    };

    #[derive(Default)]
    struct CapturingGovernance {
        command: Mutex<Option<rustok_modules::ModuleAlloyAuthoredStageCommand>>,
    }

    #[async_trait]
    impl AlloyReleaseGovernance for CapturingGovernance {
        async fn stage_alloy_authored(
            &self,
            command: rustok_modules::ModuleAlloyAuthoredStageCommand,
        ) -> Result<ModuleAlloyAuthoredStageResult, rustok_modules::ModuleGovernanceError> {
            *self.command.lock().expect("governance command lock") = Some(command);
            Ok(ModuleAlloyAuthoredStageResult {
                staging_id: "rpas_test".to_string(),
                created: true,
                request_revision: 2,
            })
        }
    }

    #[derive(Default)]
    struct CapturingParentPolicy {
        parents: Mutex<Vec<ArtifactReleaseRef>>,
    }

    #[async_trait]
    impl AlloyImportedDraftPolicyProvider for CapturingParentPolicy {
        async fn resolve_policy(
            &self,
            _tenant_id: Uuid,
            parent_release: &ArtifactReleaseRef,
        ) -> Result<SandboxPolicy, AlloyImportedDraftPolicyError> {
            self.parents
                .lock()
                .expect("parent policy lock")
                .push(parent_release.clone());
            Ok(SandboxPolicy::default())
        }
    }

    #[tokio::test]
    async fn imported_fork_stage_carries_parent_lineage_through_the_owner_gate() {
        let parent_release = ArtifactReleaseRef {
            slug: "tax_rule".to_string(),
            version: "1.0.0".to_string(),
            digest: format!("sha256:{}", "a".repeat(64)),
        };
        let tenant_id = Uuid::new_v4();
        let mut script = Script::new(
            "imported_tax_rule",
            RhaiWorkspace::single_source("42"),
            ScriptTrigger::Manual,
        );
        script.tenant_id = tenant_id;
        script.parent_release = Some(parent_release.clone());
        script.workspace.files.push(RhaiWorkspaceFile {
            path: rustok_modules::ALLOY_PUBLICATION_SMOKE_TEST_PATH.to_string(),
            kind: RhaiWorkspaceFileKind::Test,
            contents: "true".to_string(),
        });

        let storage = Arc::new(InMemoryStorage::new());
        let script = storage.save(script).await.expect("save imported draft");
        storage
            .review(ReviewCommand {
                script_id: script.id,
                expected_revision: script.version,
                status: ReviewStatus::Approved,
                policy_revision: "review-policy".to_string(),
                actor_id: "reviewer".to_string(),
                reason: None,
                idempotency_key: Uuid::new_v4(),
            })
            .await
            .expect("approve imported draft");

        let policy = Arc::new(CapturingParentPolicy::default());
        let governance = Arc::new(CapturingGovernance::default());
        let runtime = crate::create_test_alloy_draft_runtime().with_imported_draft_policy_provider(
            AlloyImportedDraftPolicyProviderHandle(policy.clone()),
        );
        let stager = RevisionedReleaseStager::new(runtime, storage, governance.clone());
        let source_digest = script.workspace.digest().expect("source digest");

        let staged = stager
            .stage(AlloyReleaseStageCommand {
                script_id: script.id,
                expected_revision: script.version,
                publish_request_id: "request-imported-fork".to_string(),
                expected_publish_request_revision: 1,
                artifact_digest: source_digest,
                context: alloy_release_command_context(tenant_id, Uuid::new_v4(), Uuid::new_v4()),
                actor_can_manage_modules: true,
            })
            .await
            .expect("stage imported fork");

        assert!(staged.created);
        let command = governance
            .command
            .lock()
            .expect("governance command lock")
            .clone()
            .expect("owner stage command");
        assert_eq!(command.parent_release, Some(parent_release.clone()));
        assert_eq!(
            policy
                .parents
                .lock()
                .expect("parent policy lock")
                .as_slice(),
            &[parent_release]
        );
    }
}
