use crate::error::{AccountingError, AccountingResult};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum AccountType {
    /// Assets (Активы, e.g. Cash, Bank, Inventory, Accounts Receivable)
    Asset,
    /// Liabilities (Обязательства, e.g. Accounts Payable, Loans)
    Liability,
    /// Equity (Капитал)
    Equity,
    /// Revenue / Income (Выручка, Доходы)
    Revenue,
    /// Expense / COGS (Себестоимость, Расходы)
    Expense,
}

impl AccountType {
    /// Returns true if this account type has a normal debit balance (Assets, Expenses).
    #[inline]
    pub fn is_debit_normal(&self) -> bool {
        matches!(self, Self::Asset | Self::Expense)
    }

    /// Returns true if this account type has a normal credit balance (Liabilities, Equity, Revenue).
    #[inline]
    pub fn is_credit_normal(&self) -> bool {
        matches!(self, Self::Liability | Self::Equity | Self::Revenue)
    }

    /// Calculates the closing balance from opening balance, debit turnover, and credit turnover.
    ///
    /// For debit-normal accounts (Assets, Expenses):
    /// `closing = opening + debit_turnover - credit_turnover`
    ///
    /// For credit-normal accounts (Liabilities, Equity, Revenue):
    /// `closing = opening + credit_turnover - debit_turnover`
    pub fn calculate_closing_balance(
        &self,
        opening_balance: i64,
        debit_turnover: i64,
        credit_turnover: i64,
    ) -> AccountingResult<i64> {
        if debit_turnover < 0 {
            return Err(AccountingError::Validation(format!(
                "Debit turnover cannot be negative, got {debit_turnover}"
            )));
        }
        if credit_turnover < 0 {
            return Err(AccountingError::Validation(format!(
                "Credit turnover cannot be negative, got {credit_turnover}"
            )));
        }

        if self.is_debit_normal() {
            opening_balance
                .checked_add(debit_turnover)
                .and_then(|val| val.checked_sub(credit_turnover))
                .ok_or_else(|| AccountingError::InvariantViolation("Balance calculation overflow".into()))
        } else {
            opening_balance
                .checked_add(credit_turnover)
                .and_then(|val| val.checked_sub(debit_turnover))
                .ok_or_else(|| AccountingError::InvariantViolation("Balance calculation overflow".into()))
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct AccountDefinition {
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

impl AccountDefinition {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_account_type_normal_balance_and_closing_calculation() {
        assert!(AccountType::Asset.is_debit_normal());
        assert!(!AccountType::Asset.is_credit_normal());
        assert!(AccountType::Expense.is_debit_normal());

        assert!(AccountType::Liability.is_credit_normal());
        assert!(!AccountType::Liability.is_debit_normal());
        assert!(AccountType::Equity.is_credit_normal());
        assert!(AccountType::Revenue.is_credit_normal());

        // Asset (debit normal): opening 1000 + debit 500 - credit 200 = 1300
        let closing_asset = AccountType::Asset
            .calculate_closing_balance(1000, 500, 200)
            .unwrap();
        assert_eq!(closing_asset, 1300);

        // Liability (credit normal): opening 1000 + credit 500 - debit 200 = 1300
        let closing_liability = AccountType::Liability
            .calculate_closing_balance(1000, 200, 500)
            .unwrap();
        assert_eq!(closing_liability, 1300);

        // Negative turnover rejected
        assert!(AccountType::Asset.calculate_closing_balance(1000, -1, 100).is_err());
        assert!(AccountType::Asset.calculate_closing_balance(1000, 100, -1).is_err());

        // Overflow rejected
        assert!(AccountType::Asset.calculate_closing_balance(i64::MAX, 1, 0).is_err());
    }

    #[test]
    fn test_account_definition_validation() {
        let valid = AccountDefinition {
            code: "51.01".into(),
            name: "Расчетные счета".into(),
            account_type: AccountType::Asset,
            currency_code: Some("RUB".into()),
            is_active: true,
            track_party: false,
            track_contract: false,
            track_item: false,
            track_warehouse: false,
        };
        assert!(valid.validate().is_ok());

        let mut empty_code = valid.clone();
        empty_code.code = "   ".into();
        assert!(empty_code.validate().is_err());

        let mut long_code = valid.clone();
        long_code.code = "a".repeat(65);
        assert!(long_code.validate().is_err());

        let mut empty_name = valid.clone();
        empty_name.name = "".into();
        assert!(empty_name.validate().is_err());

        let mut empty_currency = valid;
        empty_currency.currency_code = Some("  ".into());
        assert!(empty_currency.validate().is_err());
    }
}
