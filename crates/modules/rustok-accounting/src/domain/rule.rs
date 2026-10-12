use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PostingRule {
    pub document_type: String,
    pub operation_type: String,
    pub debit_account_code: String,
    pub credit_account_code: String,
    pub description_template: Option<String>,
}
