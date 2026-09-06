use rustok_modules::ModuleCommandContext;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;
use uuid::Uuid;

use super::{ReviewDecision, ReviewStatus, ScriptId};

pub const MAX_RELEASE_REQUEST_ID_LENGTH: usize = 128;

/// Redacted proof that the exact reviewed source revision completed the
/// capability-free publication smoke entrypoint in the production sandbox.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AlloyPublicationSmokeEvidence {
    pub execution_id: Uuid,
    pub test_path: String,
    pub scenario_digest: String,
    pub executor: String,
    pub runtime_abi: String,
    pub policy_digest: String,
    pub capability_grants: u32,
}

/// Authenticated request to stage one reviewed immutable Alloy source revision
/// at the owner-owned module publication boundary.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AlloyReleaseStageCommand {
    pub script_id: ScriptId,
    pub expected_revision: u32,
    pub publish_request_id: String,
    /// Optimistic concurrency precondition for the owner-owned marketplace
    /// request. It is separate from the Alloy source revision above.
    pub expected_publish_request_revision: i64,
    pub artifact_digest: String,
    /// Tenant-scoped authenticated evidence preserved by the registry owner.
    pub context: ModuleCommandContext,
    /// Authenticated host fact for `modules:manage`. The registry owner still
    /// combines it with the current request and owner binding under lock.
    pub actor_can_manage_modules: bool,
}

impl AlloyReleaseStageCommand {
    pub fn validate(&self) -> Result<(), AlloyReleaseError> {
        if self.script_id.is_nil()
            || self.expected_revision == 0
            || self.publish_request_id.trim().is_empty()
            || self.expected_publish_request_revision < 1
            || self.publish_request_id.len() > MAX_RELEASE_REQUEST_ID_LENGTH
            || self.publish_request_id.chars().any(char::is_control)
            || !is_prefixed_sha256_digest(&self.artifact_digest)
            || self.context.validate().is_err()
            || self.context.tenant_id.is_none()
        {
            return Err(AlloyReleaseError::InvalidCommand);
        }
        Ok(())
    }
}

/// Derives the single tenant-scoped command context accepted by Alloy release
/// staging. Transports supply only authenticated identity, tenant, and their
/// idempotency key; trace evidence is never taken from the release payload.
pub fn alloy_release_command_context(
    tenant_id: Uuid,
    actor_id: Uuid,
    idempotency_key: Uuid,
) -> ModuleCommandContext {
    let trace_id = rustok_telemetry::current_trace_id()
        .filter(|trace_id| !trace_id.trim().is_empty())
        .unwrap_or_else(|| format!("alloy-release:{idempotency_key}"));
    ModuleCommandContext {
        actor_id,
        tenant_id: Some(tenant_id),
        trace_id,
        correlation_id: idempotency_key,
        idempotency_key,
    }
}

/// Hashes the immutable review record that authorizes release staging. The
/// digest lets the module owner bind review evidence without storing Alloy
/// workspace contents in its marketplace ledger.
pub fn review_evidence_digest(review: &ReviewDecision) -> Result<String, AlloyReleaseError> {
    let bytes = serde_json::to_vec(review)
        .map_err(|error| AlloyReleaseError::Serialize(error.to_string()))?;
    Ok(format!("sha256:{}", hex::encode(Sha256::digest(bytes))))
}

pub fn review_reference(review: &ReviewDecision) -> String {
    format!(
        "alloy://scripts/{}/revisions/{}/reviews/{}",
        review.script_id, review.revision, review.id
    )
}

pub fn is_release_approved(review: &ReviewDecision) -> bool {
    review.status == ReviewStatus::Approved
}

fn is_prefixed_sha256_digest(value: &str) -> bool {
    value.strip_prefix("sha256:").is_some_and(|digest| {
        digest.len() == 64 && digest.chars().all(|value| value.is_ascii_hexdigit())
    })
}

#[derive(Debug, Clone, Error, PartialEq, Eq)]
pub enum AlloyReleaseError {
    #[error("Alloy release stage command is invalid")]
    InvalidCommand,
    #[error("Alloy release stage expected current revision {expected}")]
    StaleRevision { expected: u32 },
    #[error("Alloy source revision has no current approved review")]
    ReviewNotApproved,
    #[error("Alloy artifact digest does not match the reviewed source workspace")]
    ArtifactSourceDigestMismatch,
    #[error("Alloy publication sandbox smoke failed: {0}")]
    SandboxSmokeFailed(String),
    #[error("Alloy release evidence serialization failed: {0}")]
    Serialize(String),
    #[error("module publication staging conflict: {0}")]
    GovernanceConflict(String),
    #[error("module publication request was not found: {0}")]
    GovernanceNotFound(String),
    #[error("module publication staging failed: {0}")]
    Governance(String),
}

#[cfg(test)]
mod tests {
    use super::{AlloyReleaseError, AlloyReleaseStageCommand, alloy_release_command_context};
    use uuid::Uuid;

    #[test]
    fn release_stage_requires_an_exact_revision_and_artifact_digest() {
        let command = AlloyReleaseStageCommand {
            script_id: Uuid::new_v4(),
            expected_revision: 1,
            publish_request_id: "rpr_example".to_string(),
            expected_publish_request_revision: 1,
            artifact_digest: format!("sha256:{}", "a".repeat(64)),
            context: alloy_release_command_context(Uuid::new_v4(), Uuid::new_v4(), Uuid::new_v4()),
            actor_can_manage_modules: true,
        };
        assert!(command.validate().is_ok());

        let mut invalid = command;
        invalid.expected_revision = 0;
        assert_eq!(invalid.validate(), Err(AlloyReleaseError::InvalidCommand));
        invalid.expected_revision = 1;
        invalid.expected_publish_request_revision = 0;
        assert_eq!(invalid.validate(), Err(AlloyReleaseError::InvalidCommand));
    }

    #[test]
    fn release_stage_rejects_unbounded_or_control_request_ids() {
        let mut command = AlloyReleaseStageCommand {
            script_id: Uuid::new_v4(),
            expected_revision: 1,
            publish_request_id: "rpr_example".to_string(),
            expected_publish_request_revision: 1,
            artifact_digest: format!("sha256:{}", "a".repeat(64)),
            context: alloy_release_command_context(Uuid::new_v4(), Uuid::new_v4(), Uuid::new_v4()),
            actor_can_manage_modules: true,
        };
        command.publish_request_id = "x".repeat(super::MAX_RELEASE_REQUEST_ID_LENGTH + 1);
        assert_eq!(command.validate(), Err(AlloyReleaseError::InvalidCommand));
        command.publish_request_id = "rpr_\nexample".to_string();
        assert_eq!(command.validate(), Err(AlloyReleaseError::InvalidCommand));
    }
}
