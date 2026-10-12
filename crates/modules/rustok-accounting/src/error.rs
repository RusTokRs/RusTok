use thiserror::Error;
use uuid::Uuid;

#[derive(Debug, Error)]
pub enum AccountingError {
    #[error("Accounting invariant violation: {0}")]
    InvariantViolation(String),

    #[error("Unbalanced journal transaction: total debits ({debit_total_minor}) != total credits ({credit_total_minor}) in currency {currency_code}")]
    UnbalancedTransaction {
        debit_total_minor: i64,
        credit_total_minor: i64,
        currency_code: String,
    },

    #[error("Account not found for code: {0}")]
    AccountNotFound(String),

    #[error("Account is inactive or blocked: {0}")]
    AccountInactive(String),

    #[error("Invalid currency code: {0}")]
    InvalidCurrency(String),

    #[error("Transaction not found for ID: {0}")]
    TransactionNotFound(Uuid),

    #[error("Database error: {0}")]
    Database(#[from] sea_orm::DbErr),

    #[error("Validation error: {0}")]
    Validation(String),
}

pub type AccountingResult<T> = Result<T, AccountingError>;
