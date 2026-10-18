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
        if trimmed_code.len() > 64 {
            return Err(AccountingError::Validation(
                "Account code exceeds maximum length of 64 characters".to_string(),
            ));
        }

        let trimmed_name = self.name.trim();
        if trimmed_name.is_empty() {
            return Err(AccountingError::Validation(
                "Account name cannot be empty".to_string(),
            ));
        }

        if let Some(ref cur) = self.currency_code {
            if cur.trim().is_empty() {
                return Err(AccountingError::InvalidCurrency(
                    "Currency code cannot be empty if specified".to_string(),
                ));
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct TrialBalanceLine {
    pub account_code: String,
    pub account_name: String,
    pub account_type: AccountType,
    pub debit_turnover_minor: i64,
    pub credit_turnover_minor: i64,
    pub closing_balance_minor: i64,
    pub currency_code: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct TrialBalanceResponse {
    pub lines: Vec<TrialBalanceLine>,
    pub total_debit_minor: i64,
    pub total_credit_minor: i64,
}

impl TrialBalanceResponse {
    /// Returns true if total debits equal total credits.
    #[inline]
    pub fn is_balanced(&self) -> bool {
        self.total_debit_minor == self.total_credit_minor
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
}
