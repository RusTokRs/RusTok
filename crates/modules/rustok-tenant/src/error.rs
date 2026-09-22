use thiserror::Error;

#[derive(Debug, Error)]
pub enum TenantError {
    #[error("tenant not found")]
    NotFound,
    #[error("tenant slug '{0}' already exists")]
    SlugAlreadyExists(String),
    #[error("invalid tenant settings schema: {0}")]
    InvalidSettingsSchema(String),
    #[error("invalid tenant locale policy: {0}")]
    InvalidLocalePolicy(String),
    #[error("tenant locale policy revision conflict: expected {expected}, actual {actual}")]
    LocalePolicyConflict { expected: i64, actual: i64 },
    #[error("tenant locale policy idempotency key conflicts with a previous request")]
    LocalePolicyIdempotencyConflict,
    #[error("tenant locale policy invariant violated: {0}")]
    LocalePolicyInvariant(String),
    #[error("failed to publish tenant event: {0}")]
    EventPublish(String),
    #[error("database error: {0}")]
    Database(#[from] sea_orm::DbErr),
}
