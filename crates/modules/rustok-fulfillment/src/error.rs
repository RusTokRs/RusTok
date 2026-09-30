use sea_orm::DbErr;
use thiserror::Error;
use uuid::Uuid;

pub type FulfillmentResult<T> = Result<T, FulfillmentError>;

#[derive(Debug, Error)]
pub enum FulfillmentError {
    #[error("validation failed: {0}")]
    Validation(String),
    #[error("shipping option {0} not found")]
    ShippingOptionNotFound(Uuid),
    #[error("fulfillment {0} not found")]
    FulfillmentNotFound(Uuid),
    #[error("shipping option {0} translation revision conflicts with the current state")]
    ShippingOptionTranslationRevisionConflict(Uuid),
    #[error("invalid fulfillment transition from `{from}` to `{to}`")]
    InvalidTransition { from: String, to: String },
    #[error(transparent)]
    Database(#[from] DbErr),
}
