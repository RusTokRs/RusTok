use thiserror::Error;
use uuid::Uuid;

#[derive(Debug, Error)]
pub enum WorkflowError {
    #[error("Workflow not found: {0}")]
    NotFound(Uuid),

    #[error("Workflow step not found: {0}")]
    StepNotFound(Uuid),

    #[error("Workflow execution not found: {0}")]
    ExecutionNotFound(Uuid),

    #[error("Workflow is not active (status: {0})")]
    NotActive(String),

    #[error("Step execution failed: {0}")]
    StepFailed(String),

    #[error("Unknown step type: {0}")]
    UnknownStepType(String),

    #[error("Invalid trigger config: {0}")]
    InvalidTriggerConfig(String),

    #[error("Invalid step config: {0}")]
    InvalidStepConfig(String),

    #[error("Database error: {0}")]
    Database(#[from] sea_orm::DbErr),

    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
}

pub type WorkflowResult<T> = Result<T, WorkflowError>;
