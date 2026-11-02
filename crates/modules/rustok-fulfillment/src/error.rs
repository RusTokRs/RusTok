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
    #[error("fulfillment owner boundary refused the operation")]
    OwnerPort(#[from] PortError),
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
        FulfillmentError::OwnerPort(error) => owner_port_error(&error),
        FulfillmentError::Database(_) => PortError::unavailable(
            "fulfillment.database_unavailable",
            "fulfillment storage is temporarily unavailable",
        ),
    }
}

/// Public typed stock cause, without formatting opaque peer/storage payloads. An invariant is
/// always reconciliation-only even if an invalid adapter supplied retryable=true.
pub fn shipment_stock_error_to_port_error(error: &crate::ShipmentStockError) -> PortError {
    match error {
        crate::ShipmentStockError::Owner(cause) => {
            let mut cause = cause.clone();
            if cause.kind == rustok_api::PortErrorKind::InvariantViolation {
                cause.retryable = false;
            }
            cause
        }
        crate::ShipmentStockError::NotConfigured => PortError::unavailable(
            "fulfillment.shipment_stock_not_configured",
            "shipment stock capability is not configured",
        )
        .with_retryable(false),
        crate::ShipmentStockError::Rejected(_) => PortError::invariant_violation(
            "fulfillment.shipment_stock_refused",
            "shipment stock requires owner reconciliation",
        ),
        crate::ShipmentStockError::Database(_) => PortError::unavailable(
            "fulfillment.shipment_stock_unavailable",
            "shipment stock storage is temporarily unavailable",
        ),
    }
}

/// Preserve a typed dependency/kernel cause and conservative invariant disposition. Logging
/// sites use a fixed diagnostic class rather than peer-controlled code/message text.
pub fn owner_port_error(error: &PortError) -> PortError {
    let mut cause = error.clone();
    if cause.kind == rustok_api::PortErrorKind::InvariantViolation {
        cause.retryable = false;
    }
    cause
}
