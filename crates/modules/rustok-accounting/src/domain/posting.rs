use crate::error::{AccountingError, AccountingResult};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum PostingDirection {
    Debit,
    Credit,
}

impl PostingDirection {
    #[inline]
    pub fn is_debit(&self) -> bool {
        matches!(self, Self::Debit)
    }

    #[inline]
    pub fn is_credit(&self) -> bool {
        matches!(self, Self::Credit)
    }

    /// Reverses the posting direction (Debit -> Credit, Credit -> Debit).
    /// Used for storno and balance reversals.
    #[inline]
    pub fn reverse(&self) -> Self {
        match self {
            Self::Debit => Self::Credit,
            Self::Credit => Self::Debit,
        }
    }
}

/// Validates that within a single journal transaction:
/// 1. The transaction contains at least two postings (at least one debit and one credit).
/// 2. For every currency involved, total debits equal total credits.
/// 3. All posting amounts are strictly positive integers in currency minor units.
/// 4. Currency codes are valid non-empty strings.
pub fn validate_transaction_balance<'a, I>(lines: I) -> AccountingResult<()>
where
    I: IntoIterator<Item = (&'a str, PostingDirection, i64)>,
{
    use std::collections::HashMap;

    let mut line_count: usize = 0;
    // currency -> (debit_sum, credit_sum)
    let mut sums: HashMap<&'a str, (i64, i64)> = HashMap::new();

    for (currency, direction, amount) in lines {
        let trimmed_currency = currency.trim();
        if trimmed_currency.is_empty() {
            return Err(AccountingError::InvalidCurrency(
                "Posting currency code cannot be empty".to_string(),
            ));
        }

        if amount <= 0 {
            return Err(AccountingError::Validation(format!(
                "Posting amount must be strictly positive, got {amount} for currency {trimmed_currency}"
            )));
        }

        line_count = line_count.saturating_add(1);

        let entry = sums.entry(trimmed_currency).or_insert((0, 0));
        match direction {
            PostingDirection::Debit => {
                entry.0 = entry.0.checked_add(amount).ok_or_else(|| {
                    AccountingError::InvariantViolation("Debit sum overflow".into())
                })?;
            }
            PostingDirection::Credit => {
                entry.1 = entry.1.checked_add(amount).ok_or_else(|| {
                    AccountingError::InvariantViolation("Credit sum overflow".into())
                })?;
            }
        }
    }

    if line_count < 2 {
        return Err(AccountingError::Validation(
            "Journal transaction must contain at least two postings (at least one debit and one credit)".to_string(),
        ));
    }

    for (currency, (debits, credits)) in sums {
        if debits != credits {
            return Err(AccountingError::UnbalancedTransaction {
                debit_total_minor: debits,
                credit_total_minor: credits,
                currency_code: currency.to_string(),
            });
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_posting_direction_helpers() {
        assert!(PostingDirection::Debit.is_debit());
        assert!(!PostingDirection::Debit.is_credit());
        assert_eq!(PostingDirection::Debit.reverse(), PostingDirection::Credit);

        assert!(PostingDirection::Credit.is_credit());
        assert!(!PostingDirection::Credit.is_debit());
        assert_eq!(PostingDirection::Credit.reverse(), PostingDirection::Debit);
    }

    #[test]
    fn test_balanced_transaction() {
        let lines = vec![
            ("RUB", PostingDirection::Debit, 10_000),
            ("RUB", PostingDirection::Credit, 10_000),
        ];
        assert!(validate_transaction_balance(lines).is_ok());
    }

    #[test]
    fn test_compound_balanced_transaction() {
        // 1 debit of 100, 2 credits of 60 and 40
        let lines = vec![
            ("RUB", PostingDirection::Debit, 100),
            ("RUB", PostingDirection::Credit, 60),
            ("RUB", PostingDirection::Credit, 40),
        ];
        assert!(validate_transaction_balance(lines).is_ok());
    }

    #[test]
    fn test_unbalanced_transaction_rejected() {
        let lines = vec![
            ("RUB", PostingDirection::Debit, 10_000),
            ("RUB", PostingDirection::Credit, 9_000),
        ];
        assert!(validate_transaction_balance(lines).is_err());
    }

    #[test]
    fn test_empty_transaction_rejected() {
        let lines: Vec<(&str, PostingDirection, i64)> = vec![];
        let err = validate_transaction_balance(lines).unwrap_err();
        assert!(matches!(err, AccountingError::Validation(_)));
    }

    #[test]
    fn test_single_posting_transaction_rejected() {
        let lines = vec![("RUB", PostingDirection::Debit, 10_000)];
        let err = validate_transaction_balance(lines).unwrap_err();
        assert!(matches!(err, AccountingError::Validation(_)));
    }

    #[test]
    fn test_non_positive_amounts_rejected() {
        let zero_lines = vec![
            ("RUB", PostingDirection::Debit, 0),
            ("RUB", PostingDirection::Credit, 0),
        ];
        assert!(validate_transaction_balance(zero_lines).is_err());

        let neg_lines = vec![
            ("RUB", PostingDirection::Debit, -100),
            ("RUB", PostingDirection::Credit, -100),
        ];
        assert!(validate_transaction_balance(neg_lines).is_err());
    }

    #[test]
    fn test_blank_currency_code_rejected() {
        let lines = vec![
            ("   ", PostingDirection::Debit, 100),
            ("   ", PostingDirection::Credit, 100),
        ];
        let err = validate_transaction_balance(lines).unwrap_err();
        assert!(matches!(err, AccountingError::InvalidCurrency(_)));
    }

    #[test]
    fn test_multi_currency_balanced() {
        let lines = vec![
            ("RUB", PostingDirection::Debit, 10_000),
            ("RUB", PostingDirection::Credit, 10_000),
            ("USD", PostingDirection::Debit, 5_000),
            ("USD", PostingDirection::Credit, 5_000),
        ];
        assert!(validate_transaction_balance(lines).is_ok());
    }

    #[test]
    fn test_multi_currency_with_one_unbalanced_rejected() {
        let lines = vec![
            ("RUB", PostingDirection::Debit, 10_000),
            ("RUB", PostingDirection::Credit, 10_000),
            ("USD", PostingDirection::Debit, 5_000),
            ("USD", PostingDirection::Credit, 4_999),
        ];
        let err = validate_transaction_balance(lines).unwrap_err();
        match err {
            AccountingError::UnbalancedTransaction {
                debit_total_minor,
                credit_total_minor,
                currency_code,
            } => {
                assert_eq!(currency_code, "USD");
                assert_eq!(debit_total_minor, 5000);
                assert_eq!(credit_total_minor, 4999);
            }
            other => panic!("Expected UnbalancedTransaction, got {other:?}"),
        }
    }

    #[test]
    fn test_overflow_protection() {
        let lines = vec![
            ("RUB", PostingDirection::Debit, i64::MAX),
            ("RUB", PostingDirection::Debit, 1),
            ("RUB", PostingDirection::Credit, 10),
        ];
        let err = validate_transaction_balance(lines).unwrap_err();
        assert!(matches!(err, AccountingError::InvariantViolation(_)));
    }
}
