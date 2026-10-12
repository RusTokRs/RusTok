use thiserror::Error;
use uuid::Uuid;

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

    #[error("Database error: {0}")]
    Database(#[from] sea_orm::DbErr),

    #[error("Page not found: {0}")]
    PageNotFound(Uuid),

    #[error("Page not found for slug '{slug}' in tenant '{tenant_id}'")]
    PageSlugNotFound { slug: String, tenant_id: Uuid },

    #[error("Slug conflict: slug '{slug}' already exists for tenant '{tenant_id}'")]
    SlugConflict { slug: String, tenant_id: Uuid },

    #[error("Invalid parent page: {0}")]
    InvalidParent(Uuid),

    #[error("Storage error: {0}")]
    Storage(String),

    #[error("Snapshot compaction error: {0}")]
    Compaction(String),

    #[error("Tenant context missing or unauthorized")]
    Unauthorized,

    #[error("Internal error: {0}")]
    Internal(String),
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
