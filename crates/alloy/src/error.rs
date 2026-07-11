use thiserror::Error;

#[derive(Error, Debug, Clone)]
pub enum ScriptError {
    #[error("Compilation failed: {0}")]
    Compilation(String),

    #[error("Runtime error: {0}")]
    Runtime(String),

    #[error("Script aborted: {0}")]
    Aborted(String),

    #[error("Timeout: script exceeded {limit_ms}ms")]
    Timeout { limit_ms: u64 },

    #[error("Operation limit exceeded: {limit} operations")]
    OperationLimit { limit: u64 },

    #[error("Resource limit exceeded: {resource}")]
    ResourceLimit { resource: String },

    #[error("Script not found: {name}")]
    NotFound { name: String },

    #[error("Max call depth exceeded: {depth}")]
    MaxDepthExceeded { depth: usize },

    #[error("Storage error: {0}")]
    Storage(String),

    #[error("Invalid trigger: {0}")]
    InvalidTrigger(String),

    #[error("Invalid status: {0}")]
    InvalidStatus(String),
}

pub type ScriptResult<T> = Result<T, ScriptError>;

impl From<rustok_sandbox::rhai::RhaiError> for ScriptError {
    fn from(error: rustok_sandbox::rhai::RhaiError) -> Self {
        match error {
            rustok_sandbox::rhai::RhaiError::Compilation(message) => Self::Compilation(message),
            rustok_sandbox::rhai::RhaiError::Runtime(message) => Self::Runtime(message),
            rustok_sandbox::rhai::RhaiError::Aborted(reason) => Self::Aborted(reason),
            rustok_sandbox::rhai::RhaiError::Timeout { limit_ms } => Self::Timeout { limit_ms },
            rustok_sandbox::rhai::RhaiError::OperationLimit { limit } => {
                Self::OperationLimit { limit }
            }
            rustok_sandbox::rhai::RhaiError::ResourceLimit { resource } => {
                Self::ResourceLimit { resource }
            }
        }
    }
}
