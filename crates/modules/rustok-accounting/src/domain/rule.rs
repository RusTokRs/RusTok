use crate::error::{AccountingError, AccountingResult};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct PostingRule {
    pub document_type: String,
    pub operation_type: String,
    pub debit_account_code: String,
    pub credit_account_code: String,
    pub description_template: Option<String>,
}

impl PostingRule {
    pub fn validate(&self) -> AccountingResult<()> {
        let trimmed_doc = self.document_type.trim();
        if trimmed_doc.is_empty() {
            return Err(AccountingError::Validation(
                "Posting rule document_type cannot be empty".to_string(),
            ));
        }

        let trimmed_op = self.operation_type.trim();
        if trimmed_op.is_empty() {
            return Err(AccountingError::Validation(
                "Posting rule operation_type cannot be empty".to_string(),
            ));
        }

        let trimmed_debit = self.debit_account_code.trim();
        if trimmed_debit.is_empty() {
            return Err(AccountingError::Validation(
                "Posting rule debit_account_code cannot be empty".to_string(),
            ));
        }

        let trimmed_credit = self.credit_account_code.trim();
        if trimmed_credit.is_empty() {
            return Err(AccountingError::Validation(
                "Posting rule credit_account_code cannot be empty".to_string(),
            ));
        }

        if trimmed_debit == trimmed_credit {
            return Err(AccountingError::Validation(format!(
                "Posting rule cannot have identical debit and credit account codes: {trimmed_debit}"
            )));
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_posting_rule_validation() {
        let rule = PostingRule {
            document_type: "sales_invoice".into(),
            operation_type: "standard_sale".into(),
            debit_account_code: "62.01".into(),
            credit_account_code: "90.01".into(),
            description_template: Some("Sale to {{ party_name }}".into()),
        };
        assert!(rule.validate().is_ok());

        let mut empty_doc = rule.clone();
        empty_doc.document_type = "  ".into();
        assert!(empty_doc.validate().is_err());

        let mut empty_op = rule.clone();
        empty_op.operation_type = "".into();
        assert!(empty_op.validate().is_err());

        let mut empty_debit = rule.clone();
        empty_debit.debit_account_code = "".into();
        assert!(empty_debit.validate().is_err());

        let mut empty_credit = rule.clone();
        empty_credit.credit_account_code = "".into();
        assert!(empty_credit.validate().is_err());

        let mut identical_accounts = rule;
        identical_accounts.credit_account_code = "62.01".into();
        assert!(identical_accounts.validate().is_err());
    }
}
