use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

use crate::domain::AccountType;
use crate::error::{AccountingError, AccountingResult};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct CreateAccountInput {
    pub code: String,
    pub name: String,
    pub account_type: AccountType,
    pub currency_code: Option<String>,
    pub track_party: Option<bool>,
    pub track_contract: Option<bool>,
    pub track_item: Option<bool>,
    pub track_warehouse: Option<bool>,
}

impl CreateAccountInput {
    pub fn validate(&self) -> AccountingResult<()> {
        let trimmed_code = self.code.trim();
        if trimmed_code.is_empty() {
            return Err(AccountingError::Validation(
                "Account code cannot be empty".to_string(),
            ));
        }
        if trimmed_code.len() > crate::domain::MAX_ACCOUNT_CODE_LEN {
            return Err(AccountingError::Validation(format!(
                "Account code exceeds maximum length of {} characters",
                crate::domain::MAX_ACCOUNT_CODE_LEN
            )));
        }
        if !trimmed_code
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '.' || ch == '-' || ch == '_')
        {
            return Err(AccountingError::Validation(format!(
                "Account code contains invalid characters: `{trimmed_code}`"
            )));
        }

        let trimmed_name = self.name.trim();
        if trimmed_name.is_empty() {
            return Err(AccountingError::Validation(
                "Account name cannot be empty".to_string(),
            ));
        }
        if trimmed_name.len() > crate::domain::MAX_ACCOUNT_NAME_LEN {
            return Err(AccountingError::Validation(format!(
                "Account name exceeds maximum length of {} characters",
                crate::domain::MAX_ACCOUNT_NAME_LEN
            )));
        }

        if let Some(ref cur) = self.currency_code {
            let trimmed_cur = cur.trim();
            if trimmed_cur.is_empty() {
                return Err(AccountingError::InvalidCurrency(
                    "Currency code cannot be empty if specified".to_string(),
                ));
            }
            if trimmed_cur.len() < crate::domain::MIN_CURRENCY_CODE_LEN
                || trimmed_cur.len() > crate::domain::MAX_CURRENCY_CODE_LEN
                || !trimmed_cur.chars().all(|ch| ch.is_ascii_alphanumeric())
            {
                return Err(AccountingError::InvalidCurrency(format!(
                    "Currency code must be 3-10 alphanumeric characters, got `{trimmed_cur}`"
                )));
            }
        }

        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct AccountResponse {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub code: String,
    pub name: String,
    pub account_type: AccountType,
    pub currency_code: Option<String>,
    pub is_active: bool,
    pub track_party: bool,
    pub track_contract: bool,
    pub track_item: bool,
    pub track_warehouse: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct TrialBalanceQuery {
    pub currency_code: Option<String>,
    pub party_id: Option<Uuid>,
    pub warehouse_id: Option<Uuid>,
}

impl TrialBalanceQuery {
    pub fn validate(&self) -> AccountingResult<()> {
        if let Some(ref cur) = self.currency_code {
            let trimmed_cur = cur.trim();
            if trimmed_cur.is_empty() {
                return Err(AccountingError::InvalidCurrency(
                    "Currency code cannot be empty if specified".to_string(),
                ));
            }
            if trimmed_cur.len() < crate::domain::MIN_CURRENCY_CODE_LEN
                || trimmed_cur.len() > crate::domain::MAX_CURRENCY_CODE_LEN
                || !trimmed_cur.chars().all(|ch| ch.is_ascii_alphanumeric())
            {
                return Err(AccountingError::InvalidCurrency(format!(
                    "Currency code must be 3-10 alphanumeric characters, got `{trimmed_cur}`"
                )));
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct TrialBalanceLine {
    pub account_code: String,
    pub account_name: String,
    pub account_type: AccountType,
    #[serde(default)]
    pub opening_balance_minor: i64,
    pub debit_turnover_minor: i64,
    pub credit_turnover_minor: i64,
    pub closing_balance_minor: i64,
    pub currency_code: String,
}

impl TrialBalanceLine {
    /// Computes the closing balance from opening balance and turnovers.
    pub fn calculate_closing_balance(&self) -> AccountingResult<i64> {
        self.account_type.calculate_closing_balance(
            self.opening_balance_minor,
            self.debit_turnover_minor,
            self.credit_turnover_minor,
        )
    }

    /// Verifies that closing_balance_minor matches the exact calculated closing balance.
    pub fn verify_closing_balance(&self) -> AccountingResult<()> {
        let expected = self.calculate_closing_balance()?;
        if self.closing_balance_minor != expected {
            return Err(AccountingError::InvariantViolation(format!(
                "Trial balance line for account `{}` has closing balance {} but calculated {}",
                self.account_code, self.closing_balance_minor, expected
            )));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct TrialBalanceResponse {
    pub lines: Vec<TrialBalanceLine>,
    pub total_debit_minor: i64,
    pub total_credit_minor: i64,
}

impl TrialBalanceResponse {
    /// Returns true if overall total debits equal total credits.
    #[inline]
    pub fn is_balanced(&self) -> bool {
        self.total_debit_minor == self.total_credit_minor
    }

    /// Calculates turnover totals grouped by currency code.
    pub fn turnovers_by_currency(&self) -> std::collections::HashMap<String, (i64, i64)> {
        let mut map: std::collections::HashMap<String, (i64, i64)> =
            std::collections::HashMap::new();
        for line in &self.lines {
            let entry = map
                .entry(line.currency_code.clone())
                .or_insert((0i64, 0i64));
            entry.0 = entry.0.saturating_add(line.debit_turnover_minor);
            entry.1 = entry.1.saturating_add(line.credit_turnover_minor);
        }
        map
    }

    /// Validates that within every currency present in the lines, total debits equal total credits.
    pub fn is_balanced_by_currency(&self) -> bool {
        let map = self.turnovers_by_currency();
        map.values().all(|(debits, credits)| debits == credits)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_account_input_validation() {
        let input = CreateAccountInput {
            code: "41.01".into(),
            name: "Товары на складах".into(),
            account_type: AccountType::Asset,
            currency_code: Some("RUB".into()),
            track_party: None,
            track_contract: None,
            track_item: Some(true),
            track_warehouse: Some(true),
        };
        assert!(input.validate().is_ok());

        let mut invalid_code = input.clone();
        invalid_code.code = " ".into();
        assert!(invalid_code.validate().is_err());

        let mut invalid_name = input.clone();
        invalid_name.name = "".into();
        assert!(invalid_name.validate().is_err());

        let mut invalid_cur = input;
        invalid_cur.currency_code = Some("".into());
        assert!(invalid_cur.validate().is_err());
    }

    #[test]
    fn test_trial_balance_response_is_balanced() {
        let balanced = TrialBalanceResponse {
            lines: vec![],
            total_debit_minor: 50_000,
            total_credit_minor: 50_000,
        };
        assert!(balanced.is_balanced());

        let unbalanced = TrialBalanceResponse {
            lines: vec![],
            total_debit_minor: 50_000,
            total_credit_minor: 49_999,
        };
        assert!(!unbalanced.is_balanced());
    }

    #[test]
    fn test_trial_balance_line_closing_calculation_and_verification() {
        let line = TrialBalanceLine {
            account_code: "51.01".into(),
            account_name: "Расчетные счета".into(),
            account_type: AccountType::Asset,
            opening_balance_minor: 10_000,
            debit_turnover_minor: 5_000,
            credit_turnover_minor: 2_000,
            closing_balance_minor: 13_000,
            currency_code: "RUB".into(),
        };
        assert_eq!(line.calculate_closing_balance().unwrap(), 13_000);
        assert!(line.verify_closing_balance().is_ok());

        let mut invalid_line = line;
        invalid_line.closing_balance_minor = 12_999;
        assert!(invalid_line.verify_closing_balance().is_err());
    }

    #[test]
    fn test_trial_balance_response_by_currency() {
        let resp = TrialBalanceResponse {
            lines: vec![
                TrialBalanceLine {
                    account_code: "51.01".into(),
                    account_name: "Bank RUB".into(),
                    account_type: AccountType::Asset,
                    opening_balance_minor: 0,
                    debit_turnover_minor: 10_000,
                    credit_turnover_minor: 0,
                    closing_balance_minor: 10_000,
                    currency_code: "RUB".into(),
                },
                TrialBalanceLine {
                    account_code: "62.01".into(),
                    account_name: "Buyers RUB".into(),
                    account_type: AccountType::Asset,
                    opening_balance_minor: 0,
                    debit_turnover_minor: 0,
                    credit_turnover_minor: 10_000,
                    closing_balance_minor: -10_000,
                    currency_code: "RUB".into(),
                },
                TrialBalanceLine {
                    account_code: "52.01".into(),
                    account_name: "Bank USD".into(),
                    account_type: AccountType::Asset,
                    opening_balance_minor: 0,
                    debit_turnover_minor: 5_000,
                    credit_turnover_minor: 0,
                    closing_balance_minor: 5_000,
                    currency_code: "USD".into(),
                },
                TrialBalanceLine {
                    account_code: "62.01".into(),
                    account_name: "Buyers USD".into(),
                    account_type: AccountType::Asset,
                    opening_balance_minor: 0,
                    debit_turnover_minor: 0,
                    credit_turnover_minor: 5_000,
                    closing_balance_minor: -5_000,
                    currency_code: "USD".into(),
                },
            ],
            total_debit_minor: 15_000,
            total_credit_minor: 15_000,
        };
        assert!(resp.is_balanced());
        assert!(resp.is_balanced_by_currency());

        let turnovers = resp.turnovers_by_currency();
        assert_eq!(turnovers.get("RUB"), Some(&(10_000, 10_000)));
        assert_eq!(turnovers.get("USD"), Some(&(5_000, 5_000)));
    }

    #[test]
    fn test_trial_balance_query_validation() {
        let valid = TrialBalanceQuery {
            currency_code: Some("RUB".into()),
            party_id: None,
            warehouse_id: None,
        };
        assert!(valid.validate().is_ok());

        let empty_cur = TrialBalanceQuery {
            currency_code: Some("".into()),
            party_id: None,
            warehouse_id: None,
        };
        assert!(empty_cur.validate().is_err());

        let short_cur = TrialBalanceQuery {
            currency_code: Some("AB".into()),
            party_id: None,
            warehouse_id: None,
        };
        assert!(short_cur.validate().is_err());
    }
}
