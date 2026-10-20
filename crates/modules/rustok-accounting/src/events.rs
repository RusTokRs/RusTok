use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::domain::AccountType;

pub const ACCOUNTING_EVENT_TRANSACTION_POSTED: &str = "accounting.journal.transaction_posted";
pub const ACCOUNTING_EVENT_ACCOUNT_CREATED: &str = "accounting.account.created";
pub const ACCOUNTING_EVENT_ACCOUNT_UPDATED: &str = "accounting.account.updated";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct JournalTransactionPostedEvent {
    pub tenant_id: Uuid,
    pub transaction_id: Uuid,
    pub transaction_number: String,
    pub document_type: String,
    pub document_id: Option<Uuid>,
    pub posted_at: DateTime<Utc>,
    pub is_storno: bool,
    pub currencies: Vec<String>,
    pub posting_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AccountCreatedEvent {
    pub tenant_id: Uuid,
    pub account_id: Uuid,
    pub code: String,
    pub name: String,
    pub account_type: AccountType,
    pub currency_code: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AccountUpdatedEvent {
    pub tenant_id: Uuid,
    pub account_id: Uuid,
    pub code: String,
    pub is_active: bool,
    pub updated_at: DateTime<Utc>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_event_serde_roundtrip() {
        let now = Utc::now();
        let tenant_id = Uuid::new_v4();
        let tx_id = Uuid::new_v4();

        let tx_event = JournalTransactionPostedEvent {
            tenant_id,
            transaction_id: tx_id,
            transaction_number: "TX-100".into(),
            document_type: "bank_receipt".into(),
            document_id: None,
            posted_at: now,
            is_storno: false,
            currencies: vec!["RUB".into()],
            posting_count: 2,
        };

        let json = serde_json::to_string(&tx_event).unwrap();
        let deserialized: JournalTransactionPostedEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(tx_event, deserialized);

        let acct_event = AccountCreatedEvent {
            tenant_id,
            account_id: Uuid::new_v4(),
            code: "51.01".into(),
            name: "Расчетные счета".into(),
            account_type: AccountType::Asset,
            currency_code: Some("RUB".into()),
            created_at: now,
        };

        let json = serde_json::to_string(&acct_event).unwrap();
        let deserialized: AccountCreatedEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(acct_event, deserialized);
    }
}
