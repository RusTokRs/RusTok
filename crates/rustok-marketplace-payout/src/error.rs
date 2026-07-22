use thiserror::Error;
use uuid::Uuid;

pub type MarketplacePayoutResult<T> = Result<T, MarketplacePayoutError>;

#[derive(Debug, Error)]
pub enum MarketplacePayoutError {
    #[error("marketplace payout {0} was not found")]
    PayoutNotFound(Uuid),
    #[error("ledger entry {0} was not found in the seller payable projection")]
    LedgerEntryNotFound(Uuid),
    #[error("ledger entry {0} is already assigned to a payout")]
    LedgerEntryAlreadyAssigned(Uuid),
    #[error("marketplace payout idempotency key is already bound to another request")]
    IdempotencyConflict,
    #[error("marketplace payout receipt is incomplete or corrupt")]
    ReceiptCorrupt,
    #[error("marketplace payout ledger writer is not configured")]
    LedgerWriterNotConfigured,
    #[error("marketplace payout operation {0} is already being processed")]
    OperationInProgress(Uuid),
    #[error("marketplace payout operation {operation_id} failed with code {code:?}")]
    OperationFailed {
        operation_id: Uuid,
        code: Option<String>,
    },
    #[error("marketplace payout operation {0} requires compensation retry")]
    CompensationRequired(Uuid),
    #[error("marketplace payout operation {0} requires operator reconciliation")]
    ReconciliationRequired(Uuid),
    #[error("marketplace payout operation {0} is incomplete or corrupt")]
    OperationCorrupt(Uuid),
    #[error("payout provider `{provider_id}` is not configured")]
    ProviderConfiguration { provider_id: String },
    #[error("payout provider `{provider_id}` cannot execute `{operation}`")]
    ProviderUnavailable {
        provider_id: String,
        operation: String,
    },
    #[error("payout provider `{provider_id}` rejected `{operation}` with code `{code}`")]
    ProviderRejected {
        provider_id: String,
        operation: String,
        code: String,
    },
    #[error("payout provider `{provider_id}` returned an invalid `{operation}` response")]
    ProviderInvalidResponse {
        provider_id: String,
        operation: String,
    },
    #[error("payout provider `{provider_id}` outcome for `{operation}` is unknown")]
    ProviderOutcomeUnknown {
        provider_id: String,
        operation: String,
    },
    #[error("marketplace payout validation failed: {0}")]
    Validation(String),
    #[error("ledger boundary `{code}` failed: {message}")]
    LedgerBoundary {
        code: String,
        message: String,
        retryable: bool,
    },
    #[error(transparent)]
    Database(#[from] sea_orm::DbErr),
}
