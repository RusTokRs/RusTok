use crate::error::{AccountingError, AccountingResult};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum PostingDirection {
    Debit,
    Credit,
}

/// Validates that within a single journal transaction, total debits equal total credits
/// for every involved currency.
pub fn validate_transaction_balance<'a, I>(lines: I) -> AccountingResult<()>
where
    I: IntoIterator<Item = (&'a str, PostingDirection, i64)>,
{
    use std::collections::HashMap;

    // currency -> (debit_sum, credit_sum)
    let mut sums: HashMap<&'a str, (i64, i64)> = HashMap::new();

    for (currency, direction, amount) in lines {
        if amount <= 0 {
            return Err(AccountingError::Validation(format!(
                "Posting amount must be strictly positive, got {amount} for currency {currency}"
            )));
        }

        let entry = sums.entry(currency).or_insert((0, 0));
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
    fn test_balanced_transaction() {
        let lines = vec![
            ("RUB", PostingDirection::Debit, 100_00),
            ("RUB", PostingDirection::Credit, 100_00),
        ];
        assert!(validate_transaction_balance(lines).is_ok());
    }

    #[test]
    fn test_unbalanced_transaction_rejected() {
        let lines = vec![
            ("RUB", PostingDirection::Debit, 100_00),
            ("RUB", PostingDirection::Credit, 90_00),
        ];
        assert!(validate_transaction_balance(lines).is_err());
    }

    #[test]
    fn test_multi_currency_balanced() {
        let lines = vec![
            ("RUB", PostingDirection::Debit, 100_00),
            ("RUB", PostingDirection::Credit, 100_00),
            ("USD", PostingDirection::Debit, 50_00),
            ("USD", PostingDirection::Credit, 50_00),
        ];
        assert!(validate_transaction_balance(lines).is_ok());
    }
}
