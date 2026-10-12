use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

use crate::domain::PostingDirection;

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
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

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct PostJournalTransactionInput {
    pub transaction_number: Option<String>,
    pub document_type: String,
    pub document_id: Option<Uuid>,
    pub description: Option<String>,
    pub posted_at: Option<DateTime<Utc>>,
    pub lines: Vec<JournalPostingItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
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

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
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
