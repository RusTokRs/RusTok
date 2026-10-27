use thiserror::Error;
use uuid::Uuid;

#[derive(Debug, Error)]
pub enum AccountingError {
    #[error("Accounting invariant violation: {0}")]
    InvariantViolation(String),

    #[error(
        "Unbalanced journal transaction: total debits ({debit_total_minor}) != total credits ({credit_total_minor}) in currency {currency_code}"
    )]
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_display_formatting() {
        let err = AccountingError::UnbalancedTransaction {
            debit_total_minor: 1000,
            credit_total_minor: 900,
            currency_code: "RUB".into(),
        };
        assert_eq!(
            err.to_string(),
            "Unbalanced journal transaction: total debits (1000) != total credits (900) in currency RUB"
        );

        let err2 = AccountingError::AccountNotFound("51.01".into());
        assert_eq!(err2.to_string(), "Account not found for code: 51.01");

        let err3 = AccountingError::AccountInactive("60.01".into());
        assert_eq!(err3.to_string(), "Account is inactive or blocked: 60.01");

        let err4 = AccountingError::InvalidCurrency("XYZ123".into());
        assert_eq!(err4.to_string(), "Invalid currency code: XYZ123");

        let tx_id = Uuid::new_v4();
        let err5 = AccountingError::TransactionNotFound(tx_id);
        assert_eq!(
            err5.to_string(),
            format!("Transaction not found for ID: {tx_id}")
        );

        let err6 = AccountingError::Validation("invalid input".into());
        assert_eq!(err6.to_string(), "Validation error: invalid input");

        let err7 = AccountingError::InvariantViolation("broken invariant".into());
        assert_eq!(
            err7.to_string(),
            "Accounting invariant violation: broken invariant"
        );
    }
}
