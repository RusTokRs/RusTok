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

    #[error("Script revision conflict: expected version {expected}")]
    RevisionConflict { expected: u32 },

    #[error("Max call depth exceeded: {depth}")]
    MaxDepthExceeded { depth: usize },

    #[error("Storage error: {0}")]
    Storage(String),

    #[error("Invalid trigger: {0}")]
    InvalidTrigger(String),

    #[error("Invalid status: {0}")]
    InvalidStatus(String),

    #[error("Invalid Alloy workspace: {0}")]
    InvalidWorkspace(String),

    #[error(transparent)]
    Review(#[from] crate::model::ReviewError),

    #[error(transparent)]
    TestRun(#[from] crate::model::TestRunError),

    #[error(transparent)]
    Release(#[from] crate::model::AlloyReleaseError),
}

impl From<crate::model::WorkspaceError> for ScriptError {
    fn from(error: crate::model::WorkspaceError) -> Self {
        Self::InvalidWorkspace(error.to_string())
    }
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

impl From<rustok_sandbox::SandboxError> for ScriptError {
    fn from(error: rustok_sandbox::SandboxError) -> Self {
        match error {
            rustok_sandbox::SandboxError::Compilation(message) => Self::Compilation(message),
            rustok_sandbox::SandboxError::Timeout { limit_ms } => Self::Timeout { limit_ms },
            rustok_sandbox::SandboxError::LimitExceeded { resource, limit } => {
                if resource == "instructions" {
                    Self::OperationLimit { limit }
                } else {
                    Self::ResourceLimit { resource }
                }
            }
            rustok_sandbox::SandboxError::Trap(message)
            | rustok_sandbox::SandboxError::InvalidRequest(message)
            | rustok_sandbox::SandboxError::Internal(message) => Self::Runtime(message),
            rustok_sandbox::SandboxError::Aborted(reason) => Self::Aborted(reason),
            rustok_sandbox::SandboxError::CapabilityDenied(capability) => {
                Self::Runtime(format!("capability `{capability}` is denied"))
            }
            rustok_sandbox::SandboxError::CapabilityConstraintDenied { capability, reason } => {
                Self::Runtime(format!(
                    "capability `{capability}` violates policy: {reason}"
                ))
            }
            rustok_sandbox::SandboxError::CapabilityContextMismatch { field } => Self::Runtime(
                format!("capability call {field} does not match its execution"),
            ),
            rustok_sandbox::SandboxError::HostCapability {
                capability,
                message,
            } => Self::Runtime(format!("capability `{capability}` failed: {message}")),
            rustok_sandbox::SandboxError::Cancelled => Self::Runtime("execution cancelled".into()),
            rustok_sandbox::SandboxError::ExecutorNotRegistered(kind) => {
                Self::Runtime(format!("sandbox executor `{kind}` is unavailable"))
            }
            rustok_sandbox::SandboxError::ExecutorAlreadyRegistered(kind) => {
                Self::Runtime(format!("sandbox executor `{kind}` is duplicated"))
            }
            rustok_sandbox::SandboxError::AuditUnavailable(message) => {
                Self::Runtime(format!("execution audit unavailable: {message}"))
            }
        }
    }
}
