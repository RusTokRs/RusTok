use std::sync::Arc;

use async_trait::async_trait;
use rustok_modules::{
    ModuleAlloyAuthoredStageCommand, ModuleAlloyAuthoredStageResult, ModuleGovernanceError,
    SeaOrmModuleGovernanceService,
};

use crate::model::{is_release_approved, review_evidence_digest, review_reference};
use crate::{AlloyReleaseError, AlloyReleaseStageCommand, ScriptRegistry};

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
    G: AlloyReleaseGovernance,
{
    registry: Arc<R>,
    governance: Arc<G>,
}

impl<R, G> RevisionedReleaseStager<R, G>
where
    R: ScriptRegistry,
    G: AlloyReleaseGovernance,
{
    pub fn new(registry: Arc<R>, governance: Arc<G>) -> Self {
        Self {
            registry,
            governance,
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
                idempotency_key: command.idempotency_key,
                actor_principal: serde_json::json!({
                    "kind": "alloy_actor",
                    "id": command.actor_id,
                }),
            })
            .await
            .map_err(|error| AlloyReleaseError::Governance(error.to_string()))
    }
}

fn script_error_to_release(error: crate::ScriptError) -> AlloyReleaseError {
    AlloyReleaseError::Governance(error.to_string())
}
