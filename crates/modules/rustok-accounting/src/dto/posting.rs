use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

use crate::domain::{PostingDirection, validate_transaction_balance};
use crate::error::{AccountingError, AccountingResult};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct JournalPostingItem {
    pub account_code: String,
    pub direction: PostingDirection,
    pub amount_minor: i64,
    pub currency_code: String,
    pub party_id: Option<Uuid>,
    pub contract_id: Option<Uuid>,
    pub item_id: Option<Uuid>,
    pub warehouse_id: Option<Uuid>,
    pub description: Option<String>,
}

impl JournalPostingItem {
    pub fn validate(&self) -> AccountingResult<()> {
        let trimmed_account = self.account_code.trim();
        if trimmed_account.is_empty() {
            return Err(AccountingError::Validation(
                "Journal posting item account_code cannot be empty".to_string(),
            ));
        }

        let trimmed_currency = self.currency_code.trim();
        if trimmed_currency.is_empty() {
            return Err(AccountingError::InvalidCurrency(
                "Journal posting item currency_code cannot be empty".to_string(),
            ));
        }

        if self.amount_minor <= 0 {
            return Err(AccountingError::Validation(format!(
                "Posting amount must be strictly positive, got {} for account {}",
                self.amount_minor, trimmed_account
            )));
        }

        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct PostJournalTransactionInput {
    pub transaction_number: Option<String>,
    pub document_type: String,
    pub document_id: Option<Uuid>,
    pub description: Option<String>,
    pub posted_at: Option<DateTime<Utc>>,
    pub lines: Vec<JournalPostingItem>,
}

impl PostJournalTransactionInput {
    pub fn validate(&self) -> AccountingResult<()> {
        let trimmed_doc = self.document_type.trim();
        if trimmed_doc.is_empty() {
            return Err(AccountingError::Validation(
                "document_type cannot be empty".to_string(),
            ));
        }

        if let Some(ref num) = self.transaction_number {
            if num.trim().is_empty() {
                return Err(AccountingError::Validation(
                    "transaction_number cannot be blank if provided".to_string(),
                ));
            }
        }

        for line in &self.lines {
            line.validate()?;
        }

        validate_transaction_balance(self.lines.iter().map(|line| {
            (
                line.currency_code.as_str(),
                line.direction,
                line.amount_minor,
            )
        }))?;

        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct JournalPostingResponse {
    pub id: Uuid,
    pub line_number: i32,
    pub account_code: String,
    pub direction: PostingDirection,
    pub amount_minor: i64,
    pub currency_code: String,
    pub party_id: Option<Uuid>,
    pub contract_id: Option<Uuid>,
    pub item_id: Option<Uuid>,
    pub warehouse_id: Option<Uuid>,
    pub description: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct JournalTransactionResponse {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub transaction_number: String,
    pub document_type: String,
    pub document_id: Option<Uuid>,
    pub description: Option<String>,
    pub posted_at: DateTime<Utc>,
    pub is_storno: bool,
    pub postings: Vec<JournalPostingResponse>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_journal_posting_item_validation() {
        let item = JournalPostingItem {
            account_code: "51.01".into(),
            direction: PostingDirection::Debit,
            amount_minor: 10_000,
            currency_code: "RUB".into(),
            party_id: None,
            contract_id: None,
            item_id: None,
            warehouse_id: None,
            description: Some("Deposit".into()),
        };
        assert!(item.validate().is_ok());

        let mut empty_account = item.clone();
        empty_account.account_code = "".into();
        assert!(empty_account.validate().is_err());

        let mut empty_cur = item.clone();
        empty_cur.currency_code = "  ".into();
        assert!(empty_cur.validate().is_err());

        let mut zero_amount = item;
        zero_amount.amount_minor = 0;
        assert!(zero_amount.validate().is_err());
    }

    #[test]
    fn test_post_journal_transaction_input_validation() {
        let valid_tx = PostJournalTransactionInput {
            transaction_number: Some("TX-001".into()),
            document_type: "bank_receipt".into(),
            document_id: Some(Uuid::new_v4()),
            description: Some("Incoming bank payment".into()),
            posted_at: Some(Utc::now()),
            lines: vec![
                JournalPostingItem {
                    account_code: "51.01".into(),
                    direction: PostingDirection::Debit,
                    amount_minor: 50_000,
                    currency_code: "RUB".into(),
                    party_id: None,
                    contract_id: None,
                    item_id: None,
                    warehouse_id: None,
                    description: None,
                },
                JournalPostingItem {
                    account_code: "62.01".into(),
                    direction: PostingDirection::Credit,
                    amount_minor: 50_000,
                    currency_code: "RUB".into(),
                    party_id: None,
                    contract_id: None,
                    item_id: None,
                    warehouse_id: None,
                    description: None,
                },
            ],
        };
        assert!(valid_tx.validate().is_ok());

        let mut empty_doc = valid_tx.clone();
        empty_doc.document_type = " ".into();
        assert!(empty_doc.validate().is_err());

        let mut blank_tx_num = valid_tx.clone();
        blank_tx_num.transaction_number = Some("   ".into());
        assert!(blank_tx_num.validate().is_err());

        let mut unbalanced = valid_tx;
        unbalanced.lines[1].amount_minor = 49_999;
        assert!(unbalanced.validate().is_err());
    }
}
