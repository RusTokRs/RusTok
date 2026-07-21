use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "marketplace_reversal_event_inbox")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub provider_event_id: Uuid,
    pub event_source: String,
    pub event_id: String,
    pub event_hash: String,
    pub reversal_kind: String,
    pub source_id: Uuid,
    pub order_id: Uuid,
    pub payment_collection_id: Uuid,
    pub occurred_at: DateTimeWithTimeZone,
    pub currency_code: String,
    pub currency_exponent: i16,
    pub total_amount: i64,
    pub lines_json: Json,
    pub status: String,
    pub attempt_count: i32,
    pub lease_owner: Option<String>,
    pub lease_expires_at: Option<DateTimeWithTimeZone>,
    pub reversal_id: Option<Uuid>,
    pub ledger_transaction_id: Option<Uuid>,
    pub last_error_code: Option<String>,
    pub last_error_message: Option<String>,
    pub created_at: DateTimeWithTimeZone,
    pub updated_at: DateTimeWithTimeZone,
    pub processed_at: Option<DateTimeWithTimeZone>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
