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
        if trimmed_account.len() > crate::domain::MAX_ACCOUNT_CODE_LEN {
            return Err(AccountingError::Validation(format!(
                "Journal posting item account_code exceeds maximum length of {} characters",
                crate::domain::MAX_ACCOUNT_CODE_LEN
            )));
        }
        if !trimmed_account
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '.' || ch == '-' || ch == '_')
        {
            return Err(AccountingError::Validation(format!(
                "Journal posting item account_code contains invalid characters: `{trimmed_account}`"
            )));
        }

        let trimmed_currency = self.currency_code.trim();
        if trimmed_currency.is_empty() {
            return Err(AccountingError::InvalidCurrency(
                "Journal posting item currency_code cannot be empty".to_string(),
            ));
        }
        if trimmed_currency.len() < crate::domain::MIN_CURRENCY_CODE_LEN
            || trimmed_currency.len() > crate::domain::MAX_CURRENCY_CODE_LEN
            || !trimmed_currency.chars().all(|ch| ch.is_ascii_alphanumeric())
        {
            return Err(AccountingError::InvalidCurrency(format!(
                "Journal posting item currency_code must be 3-10 alphanumeric characters, got `{trimmed_currency}`"
            )));
        }

        if self.amount_minor <= 0 {
            return Err(AccountingError::Validation(format!(
                "Posting amount must be strictly positive, got {} for account {}",
                self.amount_minor, trimmed_account
            )));
        }

        if self
            .description
            .as_ref()
            .is_some_and(|desc| desc.trim().len() > 1024)
        {
            return Err(AccountingError::Validation(
                "Journal posting item description exceeds maximum length of 1024 characters".to_string(),
            ));
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
        if trimmed_doc.len() > 64 {
            return Err(AccountingError::Validation(
                "document_type exceeds maximum length of 64 characters".to_string(),
            ));
        }

        if let Some(ref num) = self.transaction_number {
            let trimmed_num = num.trim();
            if trimmed_num.is_empty() {
                return Err(AccountingError::Validation(
                    "transaction_number cannot be blank if provided".to_string(),
                ));
            }
            if trimmed_num.len() > 64 {
                return Err(AccountingError::Validation(
                    "transaction_number exceeds maximum length of 64 characters".to_string(),
                ));
            }
        }

        if self
            .description
            .as_ref()
            .is_some_and(|desc| desc.trim().len() > 1024)
        {
            return Err(AccountingError::Validation(
                "description exceeds maximum length of 1024 characters".to_string(),
            ));
        }

        if self.lines.is_empty() {
            return Err(AccountingError::Validation(
                "Journal transaction must contain at least two postings".to_string(),
            ));
        }
        if self.lines.len() > crate::domain::MAX_TRANSACTION_LINES {
            return Err(AccountingError::Validation(format!(
                "Journal transaction lines ({}) exceeds maximum allowed of {}",
                self.lines.len(),
                crate::domain::MAX_TRANSACTION_LINES
            )));
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

impl JournalTransactionResponse {
    /// Creates a reverse storno transaction input from this transaction response.
    /// Inverts all posting directions (Debit <-> Credit) while preserving amounts, currencies, and subcontos.
    pub fn create_storno_input(
        &self,
        reason: Option<&str>,
    ) -> AccountingResult<PostJournalTransactionInput> {
        let storno_lines = self
            .postings
            .iter()
            .map(|posting| JournalPostingItem {
                account_code: posting.account_code.clone(),
                direction: posting.direction.reverse(),
                amount_minor: posting.amount_minor,
                currency_code: posting.currency_code.clone(),
                party_id: posting.party_id,
                contract_id: posting.contract_id,
                item_id: posting.item_id,
                warehouse_id: posting.warehouse_id,
                description: Some(match (reason, &posting.description) {
                    (Some(r), Some(d)) => format!("STORNO ({r}): {d}"),
                    (Some(r), None) => format!("STORNO: {r}"),
                    (None, Some(d)) => format!("STORNO: {d}"),
                    (None, None) => "STORNO reversal".to_string(),
                }),
            })
            .collect::<Vec<_>>();

        let storno_tx_num = format!("STORNO-{}", self.transaction_number);
        let storno_desc = reason
            .map(|r| format!("Reversal of {}: {r}", self.transaction_number))
            .or_else(|| Some(format!("Reversal of {}", self.transaction_number)));

        let input = PostJournalTransactionInput {
            transaction_number: Some(storno_tx_num),
            document_type: self.document_type.clone(),
            document_id: self.document_id,
            description: storno_desc,
            posted_at: Some(Utc::now()),
            lines: storno_lines,
        };

        input.validate()?;
        Ok(input)
    }
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

        let mut unbalanced = valid_tx.clone();
        unbalanced.lines[1].amount_minor = 49_999;
        assert!(unbalanced.validate().is_err());

        let mut too_many_lines = valid_tx;
        too_many_lines.lines = (0..1001)
            .map(|i| JournalPostingItem {
                account_code: "51.01".into(),
                direction: if i % 2 == 0 {
                    PostingDirection::Debit
                } else {
                    PostingDirection::Credit
                },
                amount_minor: 100,
                currency_code: "RUB".into(),
                party_id: None,
                contract_id: None,
                item_id: None,
                warehouse_id: None,
                description: None,
            })
            .collect();
        assert!(too_many_lines.validate().is_err());
    }

    #[test]
    fn test_create_storno_input_inverts_directions_and_balances() {
        let tx_id = Uuid::new_v4();
        let party_id = Uuid::new_v4();
        let now = Utc::now();

        let original = JournalTransactionResponse {
            id: tx_id,
            tenant_id: Uuid::new_v4(),
            transaction_number: "INV-2026-001".into(),
            document_type: "sales_invoice".into(),
            document_id: Some(Uuid::new_v4()),
            description: Some("Sale of goods".into()),
            posted_at: now,
            is_storno: false,
            postings: vec![
                JournalPostingResponse {
                    id: Uuid::new_v4(),
                    line_number: 1,
                    account_code: "62.01".into(),
                    direction: PostingDirection::Debit,
                    amount_minor: 120_000,
                    currency_code: "RUB".into(),
                    party_id: Some(party_id),
                    contract_id: None,
                    item_id: None,
                    warehouse_id: None,
                    description: Some("Customer receivable".into()),
                },
                JournalPostingResponse {
                    id: Uuid::new_v4(),
                    line_number: 2,
                    account_code: "90.01".into(),
                    direction: PostingDirection::Credit,
                    amount_minor: 120_000,
                    currency_code: "RUB".into(),
                    party_id: None,
                    contract_id: None,
                    item_id: None,
                    warehouse_id: None,
                    description: Some("Sales revenue".into()),
                },
            ],
        };

        let storno = original
            .create_storno_input(Some("Order cancellation"))
            .unwrap();

        assert_eq!(storno.transaction_number, Some("STORNO-INV-2026-001".into()));
        assert_eq!(storno.lines.len(), 2);

        // First line was Debit 62.01, now Credit 62.01
        assert_eq!(storno.lines[0].account_code, "62.01");
        assert_eq!(storno.lines[0].direction, PostingDirection::Credit);
        assert_eq!(storno.lines[0].amount_minor, 120_000);
        assert_eq!(storno.lines[0].party_id, Some(party_id));
        assert!(storno.lines[0].description.as_ref().unwrap().contains("STORNO (Order cancellation)"));

        // Second line was Credit 90.01, now Debit 90.01
        assert_eq!(storno.lines[1].account_code, "90.01");
        assert_eq!(storno.lines[1].direction, PostingDirection::Debit);
        assert_eq!(storno.lines[1].amount_minor, 120_000);

        // Storno input is valid and strictly balanced
        assert!(storno.validate().is_ok());
    }

    #[test]
    fn test_journal_posting_item_bounds() {
        let valid = JournalPostingItem {
            account_code: "51.01".into(),
            direction: PostingDirection::Debit,
            amount_minor: 1_000,
            currency_code: "RUB".into(),
            party_id: None,
            contract_id: None,
            item_id: None,
            warehouse_id: None,
            description: None,
        };

        let mut invalid_account = valid.clone();
        invalid_account.account_code = "51.01 bad!".into();
        assert!(invalid_account.validate().is_err());

        let mut long_account = valid.clone();
        long_account.account_code = "a".repeat(65);
        assert!(long_account.validate().is_err());

        let mut short_cur = valid.clone();
        short_cur.currency_code = "US".into();
        assert!(short_cur.validate().is_err());

        let mut long_desc = valid;
        long_desc.description = Some("d".repeat(1025));
        assert!(long_desc.validate().is_err());
    }
}
