use uuid::Uuid;

use crate::TranslationError;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TranslationPublicErrorKind {
    Forbidden,
    NotFound,
    BadInput,
    Internal,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TranslationPublicError {
    pub kind: TranslationPublicErrorKind,
    pub message: String,
    pub code: &'static str,
    pub retryable: bool,
    pub correlation_id: Uuid,
}

impl std::fmt::Display for TranslationPublicError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "{} (code: {}; reference: {})",
            self.message, self.code, self.correlation_id
        )
    }
}

pub fn map_translation_public_error(
    error: &TranslationError,
    operation: &'static str,
    boundary: &'static str,
) -> TranslationPublicError {
    let (kind, message, code, retryable, error_class) = match error {
        TranslationError::Forbidden => (
            TranslationPublicErrorKind::Forbidden,
            "Translation permission denied".to_string(),
            "TRANSLATION_PERMISSION_DENIED",
            false,
            "forbidden",
        ),
        TranslationError::JobNotFound
        | TranslationError::ItemNotFound
        | TranslationError::WorkflowNoteNotFound
        | TranslationError::InterchangeArtifactNotFound
        | TranslationError::InterchangeArtifactExpired
        | TranslationError::ProposalNotFound
        | TranslationError::JobProgressNotFound
        | TranslationError::GlossaryNotFound
        | TranslationError::MemoryEntryNotFound
        | TranslationError::MachineOperationNotFound => (
            TranslationPublicErrorKind::NotFound,
            "Translation resource was not found".to_string(),
            "TRANSLATION_RESOURCE_NOT_FOUND",
            false,
            "not_found",
        ),
        TranslationError::InvalidRequest(_)
        | TranslationError::IdempotencyConflict
        | TranslationError::WorkflowRevisionConflict
        | TranslationError::JobNotWritable(_)
        | TranslationError::ItemNotWritable(_)
        | TranslationError::ProposalNotCurrent
        | TranslationError::ProposalValidationFailed
        | TranslationError::ReviewerSeparationRequired
        | TranslationError::IdempotencyActorMismatch
        | TranslationError::ApplyRecoveryAttemptMismatch
        | TranslationError::InvalidRecoveryReason
        | TranslationError::AssignmentUnchanged
        | TranslationError::ItemAssignedToAnotherActor
        | TranslationError::JobCancellationInProgress
        | TranslationError::JobNotCancellable(_)
        | TranslationError::InvalidWorkflowActor
        | TranslationError::InvalidCancellationReason
        | TranslationError::ItemNotRetryable(_)
        | TranslationError::RetryProposalNotApproved
        | TranslationError::InvalidRetryReason
        | TranslationError::TranslationPolicyConflict { .. }
        | TranslationError::TranslationPolicyStale(_)
        | TranslationError::RequiredTargetLocaleDisabled(_)
        | TranslationError::DuplicateRequiredTargetLocale
        | TranslationError::DisabledJobLocale { .. }
        | TranslationError::GlossaryNameConflict
        | TranslationError::GlossaryRevisionConflict { .. }
        | TranslationError::GlossaryRevisionUnavailable { .. }
        | TranslationError::GlossaryInactive
        | TranslationError::GlossaryActiveStateUnchanged
        | TranslationError::GlossaryLocaleMismatch
        | TranslationError::GlossaryTermConflict(_)
        | TranslationError::MemoryRevisionConflict { .. }
        | TranslationError::MemoryLifecycleConflict(_)
        | TranslationError::MachineOperationCancelled
        | TranslationError::MachineOperationTerminal(_)
        | TranslationError::InvalidMachineCancellationReason
        | TranslationError::InvalidMachineRecoveryReason
        | TranslationError::MachineRecoveryRevisionMismatch
        | TranslationError::MachineRecoveryAlreadyRequested
        | TranslationError::InterchangeArtifactNotReady
        | TranslationError::InterchangeArtifactAlreadyProcessed
        | TranslationError::MemoryRetentionConflict(_) => (
            TranslationPublicErrorKind::BadInput,
            "Translation request is invalid".to_string(),
            "TRANSLATION_REQUEST_INVALID",
            false,
            "bad_input",
        ),
        TranslationError::Provider {
            retryable: true, ..
        }
        | TranslationError::InterchangeArtifactInProgress
        | TranslationError::MachineRecoveryResultUnavailable
        | TranslationError::Database(_) => (
            TranslationPublicErrorKind::Internal,
            "Translation service is temporarily unavailable".to_string(),
            "TRANSLATION_TEMPORARILY_UNAVAILABLE",
            true,
            "temporarily_unavailable",
        ),
        _ => (
            TranslationPublicErrorKind::Internal,
            "Translation operation could not be completed".to_string(),
            "TRANSLATION_OPERATION_FAILED",
            false,
            "internal",
        ),
    };
    let correlation_id = Uuid::new_v4();
    tracing::error!(
        error_class,
        operation,
        boundary,
        public_code = code,
        retryable,
        %correlation_id,
        "Translation service operation failed with bounded diagnostics"
    );
    TranslationPublicError {
        kind,
        message,
        code,
        retryable,
        correlation_id,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn database_details_are_redacted() {
        let error = TranslationError::Database(sea_orm::DbErr::Custom(
            "password=private host=internal".to_string(),
        ));
        let public = map_translation_public_error(&error, "test", "translation_test");
        let rendered = public.to_string();

        assert_eq!(public.code, "TRANSLATION_TEMPORARILY_UNAVAILABLE");
        assert!(public.retryable);
        assert!(!rendered.contains("password=private"));
        assert!(!rendered.contains("host=internal"));
    }

    #[test]
    fn bad_input_details_are_redacted() {
        let error = TranslationError::InvalidRequest("owner-payload=private".to_string());
        let public = map_translation_public_error(&error, "test", "translation_test");
        let rendered = public.to_string();

        assert_eq!(public.kind, TranslationPublicErrorKind::BadInput);
        assert_eq!(public.message, "Translation request is invalid");
        assert_eq!(public.code, "TRANSLATION_REQUEST_INVALID");
        assert!(!public.retryable);
        assert!(!rendered.contains("owner-payload=private"));
    }

    #[test]
    fn active_artifact_processing_is_retryable() {
        let public = map_translation_public_error(
            &TranslationError::InterchangeArtifactInProgress,
            "test",
            "translation_test",
        );

        assert_eq!(public.kind, TranslationPublicErrorKind::Internal);
        assert_eq!(public.code, "TRANSLATION_TEMPORARILY_UNAVAILABLE");
        assert!(public.retryable);
    }
}
