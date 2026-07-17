use thiserror::Error;
use uuid::Uuid;

#[derive(Debug, Error)]
pub enum MarketplaceListingError {
    #[error("marketplace listing {0} not found")]
    ListingNotFound(Uuid),
    #[error("marketplace listing terms version {version} for listing {listing_id} not found")]
    TermsNotFound { listing_id: Uuid, version: i32 },
    #[error("marketplace listing seller scope is not available: {0}")]
    SellerUnavailable(String),
    #[error("marketplace listing product scope is not available: {0}")]
    ProductUnavailable(String),
    #[error("marketplace listing identity already exists")]
    DuplicateScope,
    #[error("marketplace listing seller SKU `{0}` is already in use")]
    DuplicateSellerSku(String),
    #[error("marketplace listing idempotency key is already bound to another command")]
    IdempotencyConflict,
    #[error("marketplace listing command receipt requires operator review")]
    CommandReceiptCorrupt,
    #[error("marketplace listing event contract invariant failed: {0}")]
    EventContractInvariant(String),
    #[error("marketplace listing transactional event publication is unavailable")]
    EventPublicationUnavailable,
    #[error("marketplace listing validation failed: {0}")]
    Validation(String),
    #[error("marketplace listing transition from `{from}` to `{to}` is not allowed")]
    InvalidTransition { from: String, to: String },
    #[error("marketplace listing storage unavailable: {0}")]
    Database(#[from] sea_orm::DbErr),
}

pub type MarketplaceListingResult<T> = Result<T, MarketplaceListingError>;
