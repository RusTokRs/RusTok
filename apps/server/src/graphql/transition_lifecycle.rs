use async_graphql::{Enum, ErrorExtensions, FieldError, SimpleObject};
use rustok_api::graphql::GraphQLError;
use rustok_api::{
    ModuleRetentionHoldView, ModuleTransitionCheckpointView, ModuleTransitionStateView,
};
use rustok_modules::{ModuleTransitionServiceError, TransitionCoordinatorError};
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

impl From<ModuleTransitionCheckpointView> for ModuleTransitionCheckpointGql {
    fn from(checkpoint: ModuleTransitionCheckpointView) -> Self {
        let state = match checkpoint.state {
            ModuleTransitionStateView::Preflighting => ModuleTransitionStateGql::Preflighting,
            ModuleTransitionStateView::Fenced => ModuleTransitionStateGql::Fenced,
            ModuleTransitionStateView::Prestaging => ModuleTransitionStateGql::Prestaging,
            ModuleTransitionStateView::Activating => ModuleTransitionStateGql::Activating,
            ModuleTransitionStateView::Observing => ModuleTransitionStateGql::Observing,
            ModuleTransitionStateView::PointOfNoReturn => ModuleTransitionStateGql::PointOfNoReturn,
            ModuleTransitionStateView::RecoveredToPredecessor => {
                ModuleTransitionStateGql::RecoveredToPredecessor
            }
            ModuleTransitionStateView::Converged => ModuleTransitionStateGql::Converged,
            ModuleTransitionStateView::FailedClosed => ModuleTransitionStateGql::FailedClosed,
        };
        Self {
            operation_id: checkpoint
                .operation_id
                .parse()
                .expect("owner transition view contains a UUID operation identity"),
            revision: checkpoint.revision,
            module_slug: checkpoint.module_slug,
            tenant_id: checkpoint
                .tenant_id
                .map(|tenant_id| tenant_id.parse())
                .transpose()
                .expect("owner transition view contains a UUID tenant identity"),
            predecessor_digest: checkpoint.predecessor_digest,
            candidate_digest: checkpoint.candidate_digest,
            state,
            state_details: checkpoint.state_details,
            security_epoch: checkpoint.security_epoch,
            recovery_attempt_count: checkpoint.recovery_attempt_count,
            created_at: checkpoint.created_at,
            updated_at: checkpoint.updated_at,
        }
    }
}

pub(crate) fn map_transition_service_error(error: ModuleTransitionServiceError) -> FieldError {
    match error {
        ModuleTransitionServiceError::NotFound(_) => {
            FieldError::new("Transition checkpoint not found").extend_with(|_, extensions| {
                extensions.set("code", "CHECKPOINT_NOT_FOUND");
                extensions.set("retryable_issue", false);
            })
        }
        ModuleTransitionServiceError::AuthorizationDenied => {
            <FieldError as GraphQLError>::permission_denied(
                "Permission denied for the module transition scope",
            )
        }
        ModuleTransitionServiceError::InvalidCommand(message) => {
            <FieldError as GraphQLError>::bad_user_input(&message)
        }
        ModuleTransitionServiceError::RevisionConflict { expected, current } => FieldError::new(
            format!("Transition revision conflict: expected {expected}, current {current}"),
        )
        .extend_with(|_, extensions| {
            extensions.set("code", "REVISION_CONFLICT");
            extensions.set("retryable_issue", true);
            extensions.set("current_revision", current);
        }),
        ModuleTransitionServiceError::IdempotencyConflict => {
            FieldError::new("Idempotency key was used for a different transition command")
                .extend_with(|_, extensions| {
                    extensions.set("code", "IDEMPOTENCY_CONFLICT");
                    extensions.set("retryable_issue", false);
                })
        }
        ModuleTransitionServiceError::OperationInProgress => FieldError::new(
            "Transition command is already in progress",
        )
        .extend_with(|_, extensions| {
            extensions.set("code", "OPERATION_IN_PROGRESS");
            extensions.set("retryable_issue", true);
        }),
        ModuleTransitionServiceError::Coordinator(error) => map_transition_coordinator_error(error),
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

impl From<ModuleRetentionHoldView> for RetentionHoldGql {
    fn from(record: ModuleRetentionHoldView) -> Self {
        Self {
            hold_id: record
                .hold_id
                .parse()
                .expect("owner retention hold view contains a UUID hold identity"),
            target_type: record.target_type,
            target_identity: record.target_identity,
            kind: record.kind,
            created_at: record.created_at,
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
