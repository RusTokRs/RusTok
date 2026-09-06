//! Immutable receipts for module transitions.
//!
//! Enforces:
//! - Preview receipts with preflight and conflict-fence binding.
//! - Confirmation receipts for maintenance mode cryptographically tied to preview digests.
//! - Apply receipts verifying preview/confirmation prerequisites and serving generation.
//! - Cancellation receipts ensuring operations have not passed the point of no return.
//! - Rollback receipts binding direct predecessor digests and verifying owner reversibility.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;
use uuid::Uuid;

use crate::{ConflictFenceSet, UpdateMode};

#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum TransitionReceiptError {
    #[error(
        "Confirmation receipt preview digest `{received}` does not match preview digest `{expected}`"
    )]
    PreviewDigestMismatch { expected: String, received: String },
    #[error(
        "Maintenance mode requires an explicit operator confirmation receipt before apply can proceed"
    )]
    ConfirmationRequired,
    #[error("Apply receipt requires a valid, non-empty candidate digest")]
    CandidateDigestRequired,
    #[error(
        "Cannot cancel transition `{0}`: operation has already passed the point of no return (`{1}`)"
    )]
    PastPointOfNoReturn(Uuid, String),
    #[error("Rollback prohibited: {0}")]
    RollbackProhibited(String),
    #[error("Rollback predecessor digest cannot be empty")]
    InvalidPredecessorDigest,
}

fn compute_digest<T: Serialize>(value: &T) -> String {
    let bytes = serde_json::to_vec(value).expect("receipt serialization must succeed");
    let hash = Sha256::digest(&bytes);
    format!("sha256:{}", hex::encode(hash))
}

/// Immutable receipt capturing the pre-execution preview of a module transition.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TransitionPreviewReceipt {
    pub operation_id: Uuid,
    pub module_slug: String,
    pub tenant_id: Option<Uuid>,
    pub candidate_digest: String,
    pub predecessor_digest: Option<String>,
    pub mode: UpdateMode,
    pub preflight_receipt_digest: String,
    pub conflict_fences: ConflictFenceSet,
    pub generated_at: DateTime<Utc>,
}

impl TransitionPreviewReceipt {
    pub fn new(
        operation_id: Uuid,
        module_slug: String,
        tenant_id: Option<Uuid>,
        candidate_digest: String,
        predecessor_digest: Option<String>,
        mode: UpdateMode,
        preflight_receipt_digest: String,
        conflict_fences: ConflictFenceSet,
    ) -> Self {
        Self {
            operation_id,
            module_slug,
            tenant_id,
            candidate_digest,
            predecessor_digest,
            mode,
            preflight_receipt_digest,
            conflict_fences,
            generated_at: Utc::now(),
        }
    }

    pub fn digest(&self) -> String {
        compute_digest(self)
    }
}

/// Immutable confirmation receipt recording explicit operator approval for maintenance-mode updates.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TransitionConfirmationReceipt {
    pub operation_id: Uuid,
    pub preview_digest: String,
    pub confirmed_mode: UpdateMode,
    pub operator_actor_id: String,
    pub confirmed_at: DateTime<Utc>,
    pub target_revision: u64,
}

impl TransitionConfirmationReceipt {
    pub fn new(
        operation_id: Uuid,
        preview_digest: String,
        confirmed_mode: UpdateMode,
        operator_actor_id: String,
        target_revision: u64,
    ) -> Self {
        Self {
            operation_id,
            preview_digest,
            confirmed_mode,
            operator_actor_id,
            confirmed_at: Utc::now(),
            target_revision,
        }
    }

    pub fn digest(&self) -> String {
        compute_digest(self)
    }

    /// Verifies that this confirmation binds exactly to the target preview receipt.
    pub fn validate_matches_preview(
        &self,
        preview: &TransitionPreviewReceipt,
    ) -> Result<(), TransitionReceiptError> {
        let expected_digest = preview.digest();
        if self.preview_digest != expected_digest {
            return Err(TransitionReceiptError::PreviewDigestMismatch {
                expected: expected_digest,
                received: self.preview_digest.clone(),
            });
        }
        Ok(())
    }
}

/// Immutable terminal receipt issued upon successful activation and convergence of a module transition.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TransitionApplyReceipt {
    pub operation_id: Uuid,
    pub module_slug: String,
    pub tenant_id: Option<Uuid>,
    pub candidate_digest: String,
    pub predecessor_digest: Option<String>,
    pub preview_digest: String,
    pub confirmation_digest: Option<String>,
    pub serving_generation: u64,
    pub security_epoch: i64,
    pub applied_at: DateTime<Utc>,
}

impl TransitionApplyReceipt {
    pub fn issue(
        preview: &TransitionPreviewReceipt,
        confirmation: Option<&TransitionConfirmationReceipt>,
        serving_generation: u64,
        security_epoch: i64,
    ) -> Result<Self, TransitionReceiptError> {
        if preview.candidate_digest.trim().is_empty() {
            return Err(TransitionReceiptError::CandidateDigestRequired);
        }

        let confirmation_digest = match preview.mode {
            UpdateMode::Automatic => confirmation.map(|c| c.digest()),
            UpdateMode::Maintenance => {
                let conf = confirmation.ok_or(TransitionReceiptError::ConfirmationRequired)?;
                conf.validate_matches_preview(preview)?;
                Some(conf.digest())
            }
        };

        Ok(Self {
            operation_id: preview.operation_id,
            module_slug: preview.module_slug.clone(),
            tenant_id: preview.tenant_id,
            candidate_digest: preview.candidate_digest.clone(),
            predecessor_digest: preview.predecessor_digest.clone(),
            preview_digest: preview.digest(),
            confirmation_digest,
            serving_generation,
            security_epoch,
            applied_at: Utc::now(),
        })
    }

    pub fn digest(&self) -> String {
        compute_digest(self)
    }
}

/// Immutable receipt recording safe cancellation of a transition before irreversible mutations.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TransitionCancellationReceipt {
    pub operation_id: Uuid,
    pub module_slug: String,
    pub tenant_id: Option<Uuid>,
    pub reason: String,
    pub cancelled_by: String,
    pub cancelled_at: DateTime<Utc>,
    pub phase_at_cancellation: String,
}

impl TransitionCancellationReceipt {
    pub fn cancel(
        operation_id: Uuid,
        module_slug: String,
        tenant_id: Option<Uuid>,
        reason: String,
        cancelled_by: String,
        phase_at_cancellation: String,
        point_of_no_return_reached: bool,
    ) -> Result<Self, TransitionReceiptError> {
        if point_of_no_return_reached {
            return Err(TransitionReceiptError::PastPointOfNoReturn(
                operation_id,
                phase_at_cancellation,
            ));
        }

        Ok(Self {
            operation_id,
            module_slug,
            tenant_id,
            reason,
            cancelled_by,
            cancelled_at: Utc::now(),
            phase_at_cancellation,
        })
    }

    pub fn digest(&self) -> String {
        compute_digest(self)
    }
}

/// Immutable receipt capturing a manual or automatic rollback decision.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TransitionRollbackReceipt {
    pub operation_id: Uuid,
    pub module_slug: String,
    pub tenant_id: Option<Uuid>,
    pub predecessor_digest: String,
    pub is_reversible: bool,
    pub failure_reason: String,
    pub recovery_attempt: u32,
    pub decided_at: DateTime<Utc>,
}

impl TransitionRollbackReceipt {
    pub fn issue(
        operation_id: Uuid,
        module_slug: String,
        tenant_id: Option<Uuid>,
        predecessor_digest: String,
        is_reversible: bool,
        failure_reason: String,
        recovery_attempt: u32,
    ) -> Result<Self, TransitionReceiptError> {
        if predecessor_digest.trim().is_empty() {
            return Err(TransitionReceiptError::InvalidPredecessorDigest);
        }

        if !is_reversible {
            return Err(TransitionReceiptError::RollbackProhibited(
                "rollback is prohibited by owner data-migration ledger".to_string(),
            ));
        }

        Ok(Self {
            operation_id,
            module_slug,
            tenant_id,
            predecessor_digest,
            is_reversible,
            failure_reason,
            recovery_attempt,
            decided_at: Utc::now(),
        })
    }

    pub fn digest(&self) -> String {
        compute_digest(self)
    }
}
