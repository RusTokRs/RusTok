use thiserror::Error;

pub type AiResult<T> = Result<T, AiError>;

#[derive(Debug, Error)]
pub enum AiError {
    #[error("invalid configuration: {0}")]
    InvalidConfig(String),
    #[error("validation error: {0}")]
    Validation(String),
    #[error("provider error: {0}")]
    Provider(String),
    #[error("MCP error: {0}")]
    Mcp(String),
    #[error("approval required for tool `{0}`")]
    ApprovalRequired(String),
    #[error("not found: {0}")]
    NotFound(String),
    #[error("serialization error: {0}")]
    Serialization(String),
    #[error("runtime error: {0}")]
    Runtime(String),
    #[error(transparent)]
    Transport(#[from] reqwest::Error),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}
