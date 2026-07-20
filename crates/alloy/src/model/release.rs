use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;
use uuid::Uuid;

use super::{ReviewDecision, ReviewStatus, ScriptId};

pub const MAX_RELEASE_ACTOR_ID_LENGTH: usize = 255;

/// Authenticated request to stage one reviewed immutable Alloy source revision
/// at the owner-owned module publication boundary.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AlloyReleaseStageCommand {
    pub script_id: ScriptId,
    pub expected_revision: u32,
    pub publish_request_id: String,
    pub artifact_digest: String,
    pub actor_id: String,
    pub idempotency_key: Uuid,
}

impl AlloyReleaseStageCommand {
    pub fn validate(&self) -> Result<(), AlloyReleaseError> {
        if self.script_id.is_nil()
            || self.expected_revision == 0
            || self.publish_request_id.trim().is_empty()
            || !is_prefixed_sha256_digest(&self.artifact_digest)
            || !is_bounded_actor_id(&self.actor_id)
            || self.idempotency_key.is_nil()
        {
            return Err(AlloyReleaseError::InvalidCommand);
        }
        Ok(())
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

fn is_bounded_actor_id(value: &str) -> bool {
    value.trim() == value
        && !value.is_empty()
        && value.len() <= MAX_RELEASE_ACTOR_ID_LENGTH
        && !value.chars().any(char::is_control)
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
    #[error("Alloy release evidence serialization failed: {0}")]
    Serialize(String),
    #[error("module publication staging failed: {0}")]
    Governance(String),
}

#[cfg(test)]
mod tests {
    use super::{AlloyReleaseError, AlloyReleaseStageCommand};
    use uuid::Uuid;

    #[test]
    fn release_stage_requires_an_exact_revision_and_artifact_digest() {
        let command = AlloyReleaseStageCommand {
            script_id: Uuid::new_v4(),
            expected_revision: 1,
            publish_request_id: "rpr_example".to_string(),
            artifact_digest: format!("sha256:{}", "a".repeat(64)),
            actor_id: "operator".to_string(),
            idempotency_key: Uuid::new_v4(),
        };
        assert!(command.validate().is_ok());

        let mut invalid = command;
        invalid.expected_revision = 0;
        assert_eq!(invalid.validate(), Err(AlloyReleaseError::InvalidCommand));
    }
}
