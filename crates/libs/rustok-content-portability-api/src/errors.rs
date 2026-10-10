//! Portability error types.

use thiserror::Error;

/// Errors that can occur during import/export operations.
#[derive(Debug, Error)]
pub enum PortabilityError {
    /// Validation failed.
    #[error("validation failed: {0}")]
    Validation(String),

    /// Format parsing error.
    #[error("format error: {0}")]
    Format(String),

    /// Transformation error.
    #[error("transformation error: {0}")]
    Transformation(String),

    /// Persistence error (database, file system).
    #[error("persistence error: {0}")]
    Persistence(String),

    /// Format not supported.
    #[error("format not supported: {0}")]
    FormatNotSupported(String),

    /// Source not found.
    #[error("source not found: {0}")]
    SourceNotFound(String),

    /// Duplicate detected.
    #[error("duplicate: {0}")]
    Duplicate(String),

    /// Batch operation partially failed.
    #[error("batch partially failed: {succeeded}/{total} succeeded")]
    BatchPartialFailure {
        succeeded: usize,
        total: usize,
    },

    /// I/O error.
    #[error("I/O error: {0}")]
    Io(String),

    /// Serialization error.
    #[error("serialization error: {0}")]
    Serialization(String),

    /// Internal error.
    #[error("internal error: {0}")]
    Internal(String),
}

impl PortabilityError {
    pub fn validation(msg: impl Into<String>) -> Self {
        Self::Validation(msg.into())
    }

    pub fn format(msg: impl Into<String>) -> Self {
        Self::Format(msg.into())
    }

    pub fn transformation(msg: impl Into<String>) -> Self {
        Self::Transformation(msg.into())
    }

    pub fn persistence(msg: impl Into<String>) -> Self {
        Self::Persistence(msg.into())
    }

    pub fn format_not_supported(format: impl Into<String>) -> Self {
        Self::FormatNotSupported(format.into())
    }

    pub fn source_not_found(id: impl Into<String>) -> Self {
        Self::SourceNotFound(id.into())
    }

    pub fn duplicate(id: impl Into<String>) -> Self {
        Self::Duplicate(id.into())
    }

    pub fn io(msg: impl Into<String>) -> Self {
        Self::Io(msg.into())
    }

    pub fn serialization(msg: impl Into<String>) -> Self {
        Self::Serialization(msg.into())
    }

    pub fn internal(msg: impl Into<String>) -> Self {
        Self::Internal(msg.into())
    }
}

impl From<serde_json::Error> for PortabilityError {
    fn from(err: serde_json::Error) -> Self {
        Self::serialization(err.to_string())
    }
}

impl From<std::io::Error> for PortabilityError {
    fn from(err: std::io::Error) -> Self {
        Self::io(err.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_display_messages_are_actionable() {
        let err = PortabilityError::validation("title is required");
        assert!(err.to_string().contains("title is required"));

        let err = PortabilityError::format_not_supported("WordPress XML");
        assert!(err.to_string().contains("WordPress XML"));

        let err = PortabilityError::BatchPartialFailure {
            succeeded: 5,
            total: 10,
        };
        assert!(err.to_string().contains("5/10"));
    }
}
