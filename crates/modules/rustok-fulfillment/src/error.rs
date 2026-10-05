use rustok_api::PortError;
use sea_orm::DbErr;
use thiserror::Error;
use uuid::Uuid;

pub type FulfillmentResult<T> = Result<T, FulfillmentError>;

#[derive(Debug, Error)]
pub enum FulfillmentError {
    #[error("validation failed: {0}")]
    Validation(String),
    #[error("provider returned an invalid result after execution: {0}")]
    ProviderResultInvalid(String),
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

pub(crate) fn map_fulfillment_error_without_context(error: FulfillmentError) -> PortError {
    match error {
        FulfillmentError::Validation(_) => {
            PortError::validation("fulfillment.validation", "fulfillment request is invalid")
        }
        FulfillmentError::ProviderResultInvalid(_) => PortError::conflict(
            "fulfillment.reconciliation_required",
            "fulfillment provider result requires reconciliation",
        ),
        FulfillmentError::ShippingOptionNotFound(_) | FulfillmentError::FulfillmentNotFound(_) => {
            PortError::not_found(
                "fulfillment.not_found",
                "fulfillment resource was not found",
            )
        }
        FulfillmentError::InvalidTransition { .. } => PortError::conflict(
            "fulfillment.invalid_transition",
            "fulfillment operation conflicts with the current state",
        ),
        FulfillmentError::ShippingOptionTranslationRevisionConflict(_) => PortError::conflict(
            "fulfillment.shipping_option_translation_revision_conflict",
            "shipping option translation revision conflicts with the current state",
        ),
        FulfillmentError::Database(_) => PortError::unavailable(
            "fulfillment.database_unavailable",
            "fulfillment storage is temporarily unavailable",
        ),
    }
}
