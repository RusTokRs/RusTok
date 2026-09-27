use thiserror::Error;

pub type StarterResult<T> = Result<T, StarterError>;

#[derive(Debug, Error)]
pub enum StarterError {
    #[error("Database error: {0}")]
    Database(#[from] sea_orm::DbErr),

    #[error("Serialization error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("Pages domain error: {0}")]
    Pages(#[from] rustok_pages::error::PagesError),

    #[error("Blog domain error: {0}")]
    Blog(#[from] rustok_blog::BlogError),

    #[error("Forum domain error: {0}")]
    Forum(#[from] rustok_forum::error::ForumError),

    #[error("Navigation domain error: {0}")]
    Navigation(#[from] rustok_navigation::error::NavigationError),

    #[error("Validation error: {0}")]
    Validation(String),

    #[error("Tenant not found: {0}")]
    TenantNotFound(String),

    #[error("Starter blueprint not found: {0}")]
    BlueprintNotFound(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}
