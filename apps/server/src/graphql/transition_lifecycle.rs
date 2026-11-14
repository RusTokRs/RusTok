use async_graphql::{Enum, ErrorExtensions, FieldError, SimpleObject};
use rustok_api::graphql::GraphQLError;
use rustok_api::{
    ModuleRetentionHoldView, ModuleTransitionCheckpointView, ModuleTransitionStateView,
};
use rustok_modules::{
    ExecutorReadinessError, GovernedArtifactTransitionError, ModuleTransitionServiceError,
    TransitionCoordinatorError,
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
    InterventionRequired,
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

/// Converting an owner view must stay fallible: identifier fields are plain
/// strings on the read contract, so a malformed value is an owner consistency
/// failure that must surface as a sanitized GraphQL error rather than a panic.
impl TryFrom<ModuleTransitionCheckpointView> for ModuleTransitionCheckpointGql {
    type Error = FieldError;

    fn try_from(checkpoint: ModuleTransitionCheckpointView) -> Result<Self, Self::Error> {
        let state_for_details = checkpoint.state;
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
            ModuleTransitionStateView::InterventionRequired => {
                ModuleTransitionStateGql::InterventionRequired
            }
        };
        let operation_id = parse_owner_identity(
            &checkpoint.operation_id,
            "Module transition checkpoint operation identity is unavailable",
        )?;
        let tenant_id = checkpoint
            .tenant_id
            .as_deref()
            .map(|tenant_id| {
                parse_owner_identity(
                    tenant_id,
                    "Module transition checkpoint tenant identity is unavailable",
                )
            })
            .transpose()?;
        Ok(Self {
            operation_id,
            revision: checkpoint.revision,
            module_slug: checkpoint.module_slug,
            tenant_id,
            predecessor_digest: checkpoint.predecessor_digest,
            candidate_digest: checkpoint.candidate_digest,
            state,
            state_details: sanitize_state_details(state_for_details, checkpoint.state_details),
            security_epoch: checkpoint.security_epoch,
            recovery_attempt_count: checkpoint.recovery_attempt_count,
            created_at: checkpoint.created_at,
            updated_at: checkpoint.updated_at,
        })
    }
}

/// Reads a UUID that the owner read contract exposes as an opaque string. A
/// malformed value is reported as an internal consistency failure; the offending
/// value never reaches the client or a panic hook.
fn parse_owner_identity(value: &str, message: &str) -> Result<Uuid, FieldError> {
    Uuid::parse_str(value).map_err(|error| {
        tracing::error!(%error, "owner read contract carried a malformed UUID identity");
        <FieldError as GraphQLError>::internal_error(message)
    })
}

fn sanitize_state_details(
    state: ModuleTransitionStateView,
    state_details: Option<String>,
) -> Option<String> {
    match state {
        ModuleTransitionStateView::RecoveredToPredecessor if state_details.is_some() => {
            Some("Transition recovered to the retained predecessor".to_string())
        }
        ModuleTransitionStateView::InterventionRequired if state_details.is_some() => Some(
            "Transition requires manual intervention; serving selection was not changed"
                .to_string(),
        ),
        _ => state_details,
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

pub(crate) fn map_governed_transition_error(error: GovernedArtifactTransitionError) -> FieldError {
    match error {
        GovernedArtifactTransitionError::AuthorizationDenied => {
            <FieldError as GraphQLError>::permission_denied(
                "Permission denied for the governed transition scope",
            )
        }
        GovernedArtifactTransitionError::InvalidCommand(message) => {
            <FieldError as GraphQLError>::bad_user_input(&message)
        }
        GovernedArtifactTransitionError::Conflict(message) => {
            FieldError::new(message).extend_with(|_, extensions| {
                extensions.set("code", "GOVERNED_TRANSITION_CONFLICT");
                extensions.set("retryable_issue", true);
            })
        }
        GovernedArtifactTransitionError::PreflightFailed { reasons } => {
            FieldError::new(
                "Governed transition is not eligible for automatic mode; a maintenance transition is required",
            )
            .extend_with(|_, extensions| {
                extensions.set("code", "PREFLIGHT_REQUIRES_MAINTENANCE");
                extensions.set("retryable_issue", false);
                extensions.set("reasons", reasons);
            })
        }
        GovernedArtifactTransitionError::Readiness(error) => match error {
            ExecutorReadinessError::AuthorizationDenied => {
                <FieldError as GraphQLError>::permission_denied(
                    "Permission denied for executor readiness evaluation",
                )
            }
            ExecutorReadinessError::InvalidCommand(message)
            | ExecutorReadinessError::InvalidConfiguration(message)
            | ExecutorReadinessError::InvalidPoolIdentity(message)
            | ExecutorReadinessError::InvalidFingerprint(message) => {
                <FieldError as GraphQLError>::bad_user_input(&message)
            }
            ExecutorReadinessError::AutomaticModeDenied(message)
            | ExecutorReadinessError::SmokeExecutionFailed(message)
            | ExecutorReadinessError::MissingCapabilityRoute(message) => {
                FieldError::new(message).extend_with(|_, extensions| {
                    extensions.set("code", "EXECUTOR_READINESS_DENIED");
                    extensions.set("retryable_issue", false);
                })
            }
            ExecutorReadinessError::EngineFingerprintMismatch(..)
            | ExecutorReadinessError::StalePoolGeneration(..) => {
                FieldError::new(
                    "Executor pool fingerprint or generation changed; smoke readiness must repeat",
                )
                .extend_with(|_, extensions| {
                    extensions.set("code", "EXECUTOR_READINESS_STALE");
                    extensions.set("retryable_issue", true);
                })
            }
            ExecutorReadinessError::IsolatedWorkerRequired
            | ExecutorReadinessError::MissingWorkerAttestation
            | ExecutorReadinessError::UnsupportedPayloadKind(_) => FieldError::new(
                "No composed executor pool can serve this artifact under the owner placement policy",
            )
            .extend_with(|_, extensions| {
                extensions.set("code", "EXECUTOR_POOL_UNAVAILABLE");
                extensions.set("retryable_issue", false);
            }),
            ExecutorReadinessError::ProbeUnavailable(_)
            | ExecutorReadinessError::BlobStore(_)
            | ExecutorReadinessError::Store(_) => <FieldError as GraphQLError>::internal_error(
                "Executor readiness service is unavailable",
            ),
        },
        GovernedArtifactTransitionError::Coordinator(error) => {
            map_transition_coordinator_error(error)
        }
        GovernedArtifactTransitionError::Installation(message) => {
            tracing::error!(%message, "governed transition installation owner failed");
            <FieldError as GraphQLError>::internal_error(
                "Governed transition installation owner is unavailable",
            )
        }
        GovernedArtifactTransitionError::Store(message) => {
            tracing::error!(%message, "governed transition store failed");
            <FieldError as GraphQLError>::internal_error("Governed transition store is unavailable")
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

impl TryFrom<ModuleRetentionHoldView> for RetentionHoldGql {
    type Error = FieldError;

    fn try_from(record: ModuleRetentionHoldView) -> Result<Self, Self::Error> {
        Ok(Self {
            hold_id: parse_owner_identity(
                &record.hold_id,
                "Module retention hold identity is unavailable",
            )?,
            target_type: record.target_type,
            target_identity: record.target_identity,
            kind: record.kind,
            created_at: record.created_at,
        })
    }
}

pub(crate) fn map_transition_coordinator_error(error: TransitionCoordinatorError) -> FieldError {
    match error {
        TransitionCoordinatorError::RecoveryLimitExhausted(reason) => {
            tracing::error!(%reason, "module transition automatic recovery limit exhausted");
            FieldError::new("Automatic transition recovery limit was exhausted").extend_with(
                |_, extensions| {
                    extensions.set("code", "RECOVERY_LIMIT_EXHAUSTED");
                    extensions.set("retryable_issue", false);
                },
            )
        }
        TransitionCoordinatorError::InvalidStateTransition { from, to } => FieldError::new(
            format!("Invalid state transition from {from} to {to}"),
        )
        .extend_with(|_, extensions| {
            extensions.set("code", "INVALID_STATE_TRANSITION");
            extensions.set("retryable_issue", false);
        }),
        TransitionCoordinatorError::SecurityEpochStale(error) => {
            tracing::warn!(%error, "module transition security epoch is stale");
            FieldError::new("Transition security epoch is stale; reload the current state")
                .extend_with(|_, extensions| {
                    extensions.set("code", "SECURITY_EPOCH_STALE");
                    extensions.set("retryable_issue", false);
                })
        }
        _ => <FieldError as GraphQLError>::internal_error("Transition coordinator failed"),
    }
}

#[cfg(test)]
mod tests {
    use super::{ModuleTransitionStateView, map_transition_coordinator_error};
    use rustok_modules::{SecurityEpochConflictError, TransitionCoordinatorError};

    #[test]
    fn recovery_limit_error_redacts_owner_reason() {
        let error =
            map_transition_coordinator_error(TransitionCoordinatorError::RecoveryLimitExhausted(
                "Automatic recovery already attempted: database password=secret".to_string(),
            ));

        assert_eq!(
            error.message,
            "Automatic transition recovery limit was exhausted"
        );
        assert!(!error.message.contains("database password=secret"));
    }

    #[test]
    fn malformed_owner_identity_is_a_sanitized_graphql_error() {
        let error = super::ModuleTransitionCheckpointGql::try_from(
            rustok_api::ModuleTransitionCheckpointView {
                operation_id: "not-a-uuid".to_string(),
                revision: 1,
                module_slug: "rustok-blog".to_string(),
                tenant_id: None,
                predecessor_digest: None,
                candidate_digest: "digest".to_string(),
                state: ModuleTransitionStateView::Fenced,
                state_details: None,
                security_epoch: 1,
                recovery_attempt_count: 0,
                created_at: "2026-01-01T00:00:00Z".to_string(),
                updated_at: "2026-01-01T00:00:00Z".to_string(),
            },
        )
        .expect_err("malformed owner identity must not panic or be projected");
        assert_eq!(
            error.message,
            "Module transition checkpoint operation identity is unavailable"
        );
        assert!(!error.message.contains("not-a-uuid"));

        let hold_error = super::RetentionHoldGql::try_from(rustok_api::ModuleRetentionHoldView {
            hold_id: "still-not-a-uuid".to_string(),
            target_type: "artifact".to_string(),
            target_identity: "identity".to_string(),
            kind: "cas".to_string(),
            created_at: "2026-01-01T00:00:00Z".to_string(),
        })
        .expect_err("malformed retention hold identity must not panic or be projected");
        assert_eq!(
            hold_error.message,
            "Module retention hold identity is unavailable"
        );
    }

    #[test]
    fn failed_transition_state_details_redact_owner_reason() {
        let details = super::sanitize_state_details(
            ModuleTransitionStateView::InterventionRequired,
            Some("database password=secret".to_string()),
        );

        assert_eq!(
            details.as_deref(),
            Some("Transition requires manual intervention; serving selection was not changed")
        );
        assert!(!details.unwrap().contains("database password=secret"));
    }

    #[test]
    fn security_epoch_error_redacts_latest_reason() {
        let error =
            map_transition_coordinator_error(TransitionCoordinatorError::SecurityEpochStale(
                SecurityEpochConflictError::EpochStale {
                    expected: rustok_modules::GlobalSecurityEpoch(1),
                    current: rustok_modules::GlobalSecurityEpoch(2),
                    latest_reason: "secret operational reason".to_string(),
                },
            ));

        assert_eq!(
            error.message,
            "Transition security epoch is stale; reload the current state"
        );
        assert!(!error.message.contains("secret operational reason"));
    }
}
