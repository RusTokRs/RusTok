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
        let mut smoke_context = ExecutionContext::new(ExecutionPhase::Manual)
            .with_tenant(source.tenant_id.to_string())
            .with_user(command.actor_id.clone());
        // The release idempotency key is also the stable logical sandbox
        // execution identity, so a transport retry cannot manufacture a
        // different immutable owner command for the same release attempt.
        smoke_context.execution_id = command.idempotency_key;
        let smoke_evidence = self
            .runtime
            .execute_publication_smoke(&smoke_script, &smoke_context)
            .await
            .map_err(|error| AlloyReleaseError::SandboxSmokeFailed(error.to_string()))?;
        self.governance
            .stage_alloy_authored(ModuleAlloyAuthoredStageCommand {
                request_id: command.publish_request_id,
                alloy_tenant_id: source.tenant_id,
                alloy_script_id: source.script_id,
                artifact_digest: command.artifact_digest,
                source_digest: source.source_digest,
                source_revision: source.revision,
                review_reference: review_reference(review),
                review_digest: review_evidence_digest(review)?,
                review_policy_revision: review.policy_revision.clone(),
                reviewed_by_principal: serde_json::json!({
                    "kind": "alloy_reviewer",
                    "id": review.actor_id,
                }),
                sandbox_execution_id: smoke_evidence.execution_id,
                sandbox_test_path: smoke_evidence.test_path,
                sandbox_executor: smoke_evidence.executor,
                sandbox_runtime_abi: smoke_evidence.runtime_abi,
                sandbox_policy_digest: smoke_evidence.policy_digest,
                sandbox_capability_grants: smoke_evidence.capability_grants,
                idempotency_key: command.idempotency_key,
                actor_principal: serde_json::json!({
                    "kind": "alloy_actor",
                    "id": command.actor_id,
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
