use thiserror::Error;

#[derive(Debug, Error)]
pub enum TenantError {
    #[error("tenant not found")]
    NotFound,
    #[error("tenant slug '{0}' already exists")]
    SlugAlreadyExists(String),
    #[error("invalid tenant settings schema: {0}")]
    InvalidSettingsSchema(String),
    #[error("failed to publish tenant event: {0}")]
    EventPublish(String),
    #[error("database error: {0}")]
    Database(#[from] sea_orm::DbErr),
}
