use thiserror::Error;
use uuid::Uuid;

#[derive(Debug, Error)]
pub enum WorkflowError {
    #[error("Workflow not found: {0}")]
    NotFound(Uuid),

    #[error("Workflow version not found: workflow_id={0}, version={1}")]
    VersionNotFound(Uuid, i32),

    #[error("Workflow template not found: {0}")]
    TemplateNotFound(String),

    #[error("Workflow step not found: {0}")]
    StepNotFound(Uuid),

    #[error("Workflow execution not found: {0}")]
    ExecutionNotFound(Uuid),

    #[error("Workflow {0} has active, pending or unresolved execution receipts and cannot be deleted")]
    WorkflowDeletionBlocked(Uuid),

    #[error("Workflow execution {0} is running with an unconfirmed outcome; operator reconciliation required")]
    ExecutionOutcomeUncertain(Uuid),

    #[error("Workflow cron firing {0} requires an explicit reconciliation decision")]
    CronReconciliationConflict(Uuid),

    #[error("Workflow cron reconciliation evidence is missing or invalid")]
    InvalidCronReconciliationEvidence,

    #[error("Workflow is not active (status: {0})")]
    NotActive(String),

    #[error("Step execution failed: {0}")]
    StepFailed(String),

    #[error("Workflow step {step_id} timed out after {timeout_ms}ms")]
    StepTimedOut { step_id: Uuid, timeout_ms: u64 },

    #[error("Unknown step type: {0}")]
    UnknownStepType(String),

    #[error("Invalid trigger config: {0}")]
    InvalidTriggerConfig(String),

    #[error("Invalid step config: {0}")]
    InvalidStepConfig(String),

    #[error("Webhook signature is missing")]
    WebhookSignatureMissing,

    #[error("Webhook signature is invalid")]
    WebhookSignatureInvalid,

    #[error("Webhook secret is not configured")]
    WebhookSecretNotConfigured,

    #[error("Database error: {0}")]
    Database(#[from] sea_orm::DbErr),

    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
}

pub type WorkflowResult<T> = Result<T, WorkflowError>;
