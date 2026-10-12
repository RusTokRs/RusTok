use thiserror::Error;

/// Result alias for rustok-docs operations.
pub type DocsResult<T> = Result<T, DocsError>;

/// Module-specific error contract for collaborative documents and wiki.
#[derive(Debug, Error)]
pub enum DocsError {
    #[error("Loro CRDT error: {0}")]
    Loro(String),

    #[error("Block not found: {0}")]
    BlockNotFound(String),

    #[error("Invalid block hierarchy: {0}")]
    InvalidHierarchy(String),

    #[error("Serialization error: {0}")]
    Serialization(String),

    #[error("Storage error: {0}")]
    Storage(String),

    #[error("Tenant context missing or unauthorized")]
    Unauthorized,
}

impl From<loro::LoroError> for DocsError {
    fn from(err: loro::LoroError) -> Self {
        Self::Loro(err.to_string())
    }
}

impl From<serde_json::Error> for DocsError {
    fn from(err: serde_json::Error) -> Self {
        Self::Serialization(err.to_string())
    }
}
