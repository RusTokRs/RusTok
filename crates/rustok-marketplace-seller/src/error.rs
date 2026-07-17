use thiserror::Error;
use uuid::Uuid;

#[derive(Debug, Error)]
pub enum MarketplaceSellerError {
    #[error("marketplace seller {0} not found")]
    SellerNotFound(Uuid),
    #[error("marketplace seller {seller_id} has no translation for locale `{locale}`")]
    TranslationNotFound { seller_id: Uuid, locale: String },
    #[error("marketplace seller member {0} not found")]
    MemberNotFound(Uuid),
    #[error("marketplace seller membership for user {user_id} in seller {seller_id} not found")]
    MembershipNotFound { seller_id: Uuid, user_id: Uuid },
    #[error("marketplace seller handle `{0}` is already in use")]
    DuplicateHandle(String),
    #[error("user {user_id} is already a member of seller {seller_id}")]
    DuplicateMembership { seller_id: Uuid, user_id: Uuid },
    #[error("marketplace seller idempotency key `{0}` is already bound to another command")]
    IdempotencyConflict(String),
    #[error("marketplace seller command receipt `{0}` is incomplete or corrupt")]
    CommandReceiptCorrupt(String),
    #[error("marketplace seller validation failed: {0}")]
    Validation(String),
    #[error("marketplace seller lifecycle transition from `{from}` to `{to}` is not allowed")]
    InvalidTransition { from: String, to: String },
    #[error("marketplace seller storage unavailable: {0}")]
    Database(#[from] sea_orm::DbErr),
}

pub type MarketplaceSellerResult<T> = Result<T, MarketplaceSellerError>;
