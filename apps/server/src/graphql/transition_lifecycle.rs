use async_graphql::{Enum, ErrorExtensions, FieldError, SimpleObject};
use rustok_api::graphql::GraphQLError;
use rustok_modules::{
    ModuleTransitionCheckpoint, ModuleTransitionServiceError, ModuleTransitionState,
    RetentionHoldRecord, TransitionCoordinatorError,
};
use uuid::Uuid;

#[derive(Enum, Copy, Clone, Eq, PartialEq, Debug)]
pub enum ModuleTransitionStateGql {
    Preflighting,
    Fenced,
    Prestaging,
    Activating,
    Observing,
    PointOfNoReturn,
    RecoveredToPredecessor,
    Converged,
    FailedClosed,
}

#[derive(SimpleObject, Clone, Debug)]
pub struct ModuleTransitionCheckpointGql {
    pub operation_id: Uuid,
    pub revision: i64,
    pub module_slug: String,
    pub tenant_id: Option<Uuid>,
    pub predecessor_digest: Option<String>,
    pub candidate_digest: String,
    pub state: ModuleTransitionStateGql,
    pub state_details: Option<String>,
    pub security_epoch: i64,
    pub recovery_attempt_count: i32,
    pub created_at: String,
    pub updated_at: String,
}

impl From<ModuleTransitionCheckpoint> for ModuleTransitionCheckpointGql {
    fn from(cp: ModuleTransitionCheckpoint) -> Self {
        let (state_gql, details) = match &cp.state {
            ModuleTransitionState::Preflighting => (ModuleTransitionStateGql::Preflighting, None),
            ModuleTransitionState::Fenced => (ModuleTransitionStateGql::Fenced, None),
            ModuleTransitionState::PreStaging => (ModuleTransitionStateGql::Prestaging, None),
            ModuleTransitionState::Activating => (ModuleTransitionStateGql::Activating, None),
            ModuleTransitionState::Observing { timeout_at } => (
                ModuleTransitionStateGql::Observing,
                Some(format!("Timeout at {}", timeout_at.to_rfc3339())),
            ),
            ModuleTransitionState::PointOfNoReturn { reason, committed_at } => (
                ModuleTransitionStateGql::PointOfNoReturn,
                Some(format!("Committed at {}: {}", committed_at.to_rfc3339(), reason)),
            ),
            ModuleTransitionState::RecoveredToPredecessor { failure_reason, .. } => (
                ModuleTransitionStateGql::RecoveredToPredecessor,
                Some(failure_reason.clone()),
            ),
            ModuleTransitionState::Converged { finalized_at } => (
                ModuleTransitionStateGql::Converged,
                Some(format!("Finalized at {}", finalized_at.to_rfc3339())),
            ),
            ModuleTransitionState::FailedClosed { failure_reason } => (
                ModuleTransitionStateGql::FailedClosed,
                Some(failure_reason.clone()),
            ),
        };

        Self {
            operation_id: cp.operation_id,
            revision: i64::try_from(cp.revision).unwrap_or(i64::MAX),
            module_slug: cp.module_slug,
            tenant_id: cp.tenant_id,
            predecessor_digest: cp.predecessor_digest,
            candidate_digest: cp.candidate_digest,
            state: state_gql,
            state_details: details,
            security_epoch: cp.security_epoch.value() as i64,
            recovery_attempt_count: cp.recovery_attempt_count as i32,
            created_at: cp.created_at.to_rfc3339(),
            updated_at: cp.updated_at.to_rfc3339(),
        }
    }
}

pub(crate) fn map_transition_service_error(error: ModuleTransitionServiceError) -> FieldError {
    match error {
        ModuleTransitionServiceError::NotFound(_) => FieldError::new("Transition checkpoint not found")
            .extend_with(|_, extensions| {
                extensions.set("code", "CHECKPOINT_NOT_FOUND");
                extensions.set("retryable_issue", false);
            }),
        ModuleTransitionServiceError::AuthorizationDenied => {
            <FieldError as GraphQLError>::permission_denied(
                "Permission denied for the module transition scope",
            )
        }
        ModuleTransitionServiceError::InvalidCommand(message) => {
            <FieldError as GraphQLError>::bad_user_input(&message)
        }
        ModuleTransitionServiceError::RevisionConflict { expected, current } => {
            FieldError::new(format!(
                "Transition revision conflict: expected {expected}, current {current}"
            ))
            .extend_with(|_, extensions| {
                extensions.set("code", "REVISION_CONFLICT");
                extensions.set("retryable_issue", true);
                extensions.set("current_revision", current);
            })
        }
        ModuleTransitionServiceError::IdempotencyConflict => {
            FieldError::new("Idempotency key was used for a different transition command")
                .extend_with(|_, extensions| {
                    extensions.set("code", "IDEMPOTENCY_CONFLICT");
                    extensions.set("retryable_issue", false);
                })
        }
        ModuleTransitionServiceError::OperationInProgress => {
            FieldError::new("Transition command is already in progress").extend_with(
                |_, extensions| {
                    extensions.set("code", "OPERATION_IN_PROGRESS");
                    extensions.set("retryable_issue", true);
                },
            )
        }
        ModuleTransitionServiceError::Coordinator(error) => {
            map_transition_coordinator_error(error)
        }
        ModuleTransitionServiceError::Store(_) | ModuleTransitionServiceError::Outbox(_) => {
            <FieldError as GraphQLError>::internal_error("Transition owner service is unavailable")
        }
    }
}

#[derive(SimpleObject, Clone, Debug)]
pub struct RetentionHoldGql {
    pub hold_id: Uuid,
    pub target_type: String,
    pub target_identity: String,
    pub kind: String,
    pub created_at: String,
}

impl From<RetentionHoldRecord> for RetentionHoldGql {
    fn from(record: RetentionHoldRecord) -> Self {
        let (target_type, target_identity) = record.target.identity_key();

        let kind_str =
            serde_json::to_string(&record.kind).unwrap_or_else(|_| "unknown".to_string());

        Self {
            hold_id: record.hold_id,
            target_type: target_type.to_string(),
            target_identity,
            kind: kind_str,
            created_at: record.created_at.to_rfc3339(),
        }
    }
}

pub(crate) fn map_transition_coordinator_error(error: TransitionCoordinatorError) -> FieldError {
    match error {
        TransitionCoordinatorError::RecoveryLimitExhausted(reason) => FieldError::new(reason)
            .extend_with(|_, extensions| {
                extensions.set("code", "RECOVERY_LIMIT_EXHAUSTED");
                extensions.set("retryable_issue", false);
            }),
        TransitionCoordinatorError::InvalidStateTransition { from, to } => FieldError::new(
            format!("Invalid state transition from {from} to {to}"),
        )
        .extend_with(|_, extensions| {
            extensions.set("code", "INVALID_STATE_TRANSITION");
            extensions.set("retryable_issue", false);
        }),
        TransitionCoordinatorError::SecurityEpochStale(e) => FieldError::new(e.to_string())
            .extend_with(|_, extensions| {
                extensions.set("code", "SECURITY_EPOCH_STALE");
                extensions.set("retryable_issue", false);
            }),
        _ => <FieldError as GraphQLError>::internal_error("Transition coordinator failed"),
    }
}
