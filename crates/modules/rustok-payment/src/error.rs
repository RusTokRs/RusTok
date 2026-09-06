use sea_orm::DbErr;
use thiserror::Error;
use uuid::Uuid;

pub type PaymentResult<T> = Result<T, PaymentError>;

#[derive(Debug, Error)]
pub enum PaymentError {
    #[error("validation failed: {0}")]
    Validation(String),
    #[error("payment collection {0} not found")]
    PaymentCollectionNotFound(Uuid),
    #[error("payment for collection {0} not found")]
    PaymentNotFound(Uuid),
    #[error("refund {0} not found")]
    RefundNotFound(Uuid),
    #[error("invalid payment transition from `{from}` to `{to}`")]
    InvalidTransition { from: String, to: String },
    #[error("payment provider `{provider_id}` is unavailable for `{operation}`")]
    ProviderUnavailable {
        provider_id: String,
        operation: String,
    },
    #[error("payment provider `{provider_id}` rejected `{operation}`")]
    ProviderRejected {
        provider_id: String,
        operation: String,
    },
    #[error("payment provider `{provider_id}` returned an invalid response for `{operation}`")]
    ProviderInvalidResponse {
        provider_id: String,
        operation: String,
    },
    #[error("payment provider `{provider_id}` outcome is unknown for `{operation}`")]
    ProviderOutcomeUnknown {
        provider_id: String,
        operation: String,
    },
    #[error("payment provider `{provider_id}` is not configured")]
    ProviderConfiguration { provider_id: String },
    #[error(transparent)]
    Database(#[from] DbErr),
}

impl PaymentError {
    pub fn provider_unavailable(provider_id: &str, operation: &str) -> Self {
        Self::ProviderUnavailable {
            provider_id: provider_id.to_string(),
            operation: operation.to_string(),
        }
    }

    pub fn provider_rejected(provider_id: &str, operation: &str) -> Self {
        Self::ProviderRejected {
            provider_id: provider_id.to_string(),
            operation: operation.to_string(),
        }
    }

    pub fn provider_invalid_response(provider_id: &str, operation: &str) -> Self {
        Self::ProviderInvalidResponse {
            provider_id: provider_id.to_string(),
            operation: operation.to_string(),
        }
    }

    pub fn provider_outcome_unknown(provider_id: &str, operation: &str) -> Self {
        Self::ProviderOutcomeUnknown {
            provider_id: provider_id.to_string(),
            operation: operation.to_string(),
        }
    }

    pub fn provider_configuration(provider_id: &str) -> Self {
        Self::ProviderConfiguration {
            provider_id: provider_id.to_string(),
        }
    }

    /// An unknown outcome or a malformed successful response must not be retried
    /// automatically: the provider may already have committed the external effect.
    pub fn requires_provider_reconciliation(&self) -> bool {
        matches!(
            self,
            Self::ProviderOutcomeUnknown { .. } | Self::ProviderInvalidResponse { .. }
        )
    }

    pub fn is_provider_retryable(&self) -> bool {
        matches!(self, Self::ProviderUnavailable { .. })
    }
}
