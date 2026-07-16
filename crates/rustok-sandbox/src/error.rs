use thiserror::Error;

use crate::{CapabilityName, SandboxExecutorKind};

#[derive(Debug, Clone, Error, PartialEq, Eq)]
pub enum SandboxError {
    #[error("sandbox request is invalid: {0}")]
    InvalidRequest(String),
    #[error("sandbox executor `{0}` is not registered")]
    ExecutorNotRegistered(SandboxExecutorKind),
    #[error("sandbox executor `{0}` is already registered")]
    ExecutorAlreadyRegistered(SandboxExecutorKind),
    #[error("sandbox capability `{0}` is not granted")]
    CapabilityDenied(CapabilityName),
    #[error("sandbox capability `{capability}` violates its policy constraints: {reason}")]
    CapabilityConstraintDenied {
        capability: CapabilityName,
        reason: String,
    },
    #[error("sandbox capability call {field} does not match the active execution")]
    CapabilityContextMismatch { field: &'static str },
    #[error("sandbox compilation failed: {0}")]
    Compilation(String),
    #[error("sandbox execution trapped: {0}")]
    Trap(String),
    #[error("sandbox execution was aborted: {0}")]
    Aborted(String),
    #[error("sandbox execution exceeded the {limit_ms} ms deadline")]
    Timeout { limit_ms: u64 },
    #[error("sandbox resource limit exceeded for `{resource}` ({limit})")]
    LimitExceeded { resource: String, limit: u64 },
    #[error("sandbox host capability `{capability}` failed: {message}")]
    HostCapability {
        capability: CapabilityName,
        message: String,
    },
    #[error("sandbox execution audit is unavailable: {0}")]
    AuditUnavailable(String),
    #[error("sandbox execution was cancelled")]
    Cancelled,
    #[error("sandbox internal error: {0}")]
    Internal(String),
}

impl SandboxError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::InvalidRequest(_) => "INVALID_REQUEST",
            Self::ExecutorNotRegistered(_) => "EXECUTOR_NOT_REGISTERED",
            Self::ExecutorAlreadyRegistered(_) => "EXECUTOR_ALREADY_REGISTERED",
            Self::CapabilityDenied(_) => "CAPABILITY_DENIED",
            Self::CapabilityConstraintDenied { .. } => "CAPABILITY_CONSTRAINT_DENIED",
            Self::CapabilityContextMismatch { .. } => "CAPABILITY_CONTEXT_MISMATCH",
            Self::Compilation(_) => "COMPILATION_FAILED",
            Self::Trap(_) => "EXECUTION_TRAPPED",
            Self::Aborted(_) => "EXECUTION_ABORTED",
            Self::Timeout { .. } => "EXECUTION_TIMEOUT",
            Self::LimitExceeded { .. } => "RESOURCE_LIMIT_EXCEEDED",
            Self::HostCapability { .. } => "HOST_CAPABILITY_FAILED",
            Self::AuditUnavailable(_) => "AUDIT_UNAVAILABLE",
            Self::Cancelled => "EXECUTION_CANCELLED",
            Self::Internal(_) => "INTERNAL_ERROR",
        }
    }
}

pub type SandboxResult<T> = Result<T, SandboxError>;
