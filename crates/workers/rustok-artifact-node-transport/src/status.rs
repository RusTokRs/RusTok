use rustok_modules::ModuleArtifactNodeReconciliationError;
use tonic::Status;

/// Maps the durable owner error taxonomy to the same public status semantics
/// for both the node-agent and topology-authoring gRPC services.
pub(crate) fn owner_status(error: ModuleArtifactNodeReconciliationError) -> Status {
    match error {
        ModuleArtifactNodeReconciliationError::InvalidCommand
        | ModuleArtifactNodeReconciliationError::InvalidNodeIdentity
        | ModuleArtifactNodeReconciliationError::InvalidRuntimeIdentity
        | ModuleArtifactNodeReconciliationError::InvalidTopology
        | ModuleArtifactNodeReconciliationError::ObservationIdentityMismatch
        | ModuleArtifactNodeReconciliationError::InvalidTransition => {
            Status::invalid_argument("artifact node reconciliation command is invalid")
        }
        ModuleArtifactNodeReconciliationError::AuthorizationDenied(_) => {
            Status::permission_denied("artifact node reconciliation authorization was denied")
        }
        ModuleArtifactNodeReconciliationError::RevisionConflict { .. }
        | ModuleArtifactNodeReconciliationError::ObservationRevisionConflict { .. }
        | ModuleArtifactNodeReconciliationError::TopologyDigestMismatch
        | ModuleArtifactNodeReconciliationError::ReconciliationInProgress
        | ModuleArtifactNodeReconciliationError::ClaimConflict
        | ModuleArtifactNodeReconciliationError::IdempotencyConflict => {
            Status::aborted("artifact node reconciliation operation conflicted")
        }
        ModuleArtifactNodeReconciliationError::TopologyResolution(_)
        | ModuleArtifactNodeReconciliationError::Store(_) => {
            Status::unavailable("artifact node reconciliation owner is unavailable")
        }
        ModuleArtifactNodeReconciliationError::NoReconciliationChange
        | ModuleArtifactNodeReconciliationError::StaleReconciliation
        | ModuleArtifactNodeReconciliationError::TerminalReconciliation
        | ModuleArtifactNodeReconciliationError::ReconciliationNotFound
        | ModuleArtifactNodeReconciliationError::AssignmentNotFound
        | ModuleArtifactNodeReconciliationError::AssignmentUnavailable
        | ModuleArtifactNodeReconciliationError::LeaseOverflow
        | ModuleArtifactNodeReconciliationError::RevisionOverflow => {
            Status::failed_precondition("artifact node reconciliation state is not eligible")
        }
    }
}
