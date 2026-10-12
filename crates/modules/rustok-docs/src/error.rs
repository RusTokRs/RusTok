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

impl From<DocsError> for rustok_web::HttpError {
    fn from(err: DocsError) -> Self {
        match err {
            DocsError::PageNotFound(id) => {
                rustok_web::HttpError::not_found("page_not_found", format!("Page '{id}' not found"))
            }
            DocsError::PageSlugNotFound { slug, .. } => rustok_web::HttpError::not_found(
                "page_slug_not_found",
                format!("Page slug '{slug}' not found"),
            ),
            DocsError::SlugConflict { slug, .. } => rustok_web::HttpError::new(
                axum::http::StatusCode::CONFLICT,
                "slug_conflict",
                format!("Slug '{slug}' already exists"),
            ),
            DocsError::InvalidParent(id) => rustok_web::HttpError::bad_request(
                "invalid_parent",
                format!("Invalid parent page: {id}"),
            ),
            DocsError::InvalidHierarchy(msg) => {
                rustok_web::HttpError::bad_request("invalid_hierarchy", msg)
            }
            DocsError::BlockNotFound(id) => rustok_web::HttpError::not_found(
                "block_not_found",
                format!("Block '{id}' not found"),
            ),
            DocsError::Unauthorized => {
                rustok_web::HttpError::unauthorized("unauthorized", "Unauthorized")
            }
            DocsError::Loro(msg) => rustok_web::HttpError::bad_request("crdt_error", msg),
            DocsError::Serialization(msg) => {
                rustok_web::HttpError::bad_request("serialization_error", msg)
            }
            DocsError::Database(err) => {
                tracing::error!("Database error in docs module: {err}");
                rustok_web::HttpError::internal("A database error occurred")
            }
            DocsError::Storage(msg) => {
                tracing::error!("Storage error in docs module: {msg}");
                rustok_web::HttpError::internal("Storage error")
            }
            DocsError::Compaction(msg) => {
                tracing::error!("Compaction error in docs module: {msg}");
                rustok_web::HttpError::internal("Snapshot compaction error")
            }
            DocsError::Internal(msg) => {
                tracing::error!("Internal error in docs module: {msg}");
                rustok_web::HttpError::internal("Internal server error")
            }
        }
    }
}
