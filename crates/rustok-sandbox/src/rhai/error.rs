use thiserror::Error;

#[derive(Debug, Clone, Error, PartialEq, Eq)]
pub enum RhaiError {
    #[error("Rhai compilation failed: {0}")]
    Compilation(String),
    #[error("Rhai execution failed: {0}")]
    Runtime(String),
    #[error("Rhai execution aborted: {0}")]
    Aborted(String),
    #[error("Rhai execution exceeded the {limit_ms} ms deadline")]
    Timeout { limit_ms: u64 },
    #[error("Rhai operation limit exceeded: {limit}")]
    OperationLimit { limit: u64 },
    #[error("Rhai resource limit exceeded: {resource}")]
    ResourceLimit { resource: String },
}

pub type RhaiResult<T> = Result<T, RhaiError>;

