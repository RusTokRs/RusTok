use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "marketplace_financial_operations")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub checkout_operation_id: Uuid,
    pub tenant_id: Uuid,
    pub order_id: Uuid,
    pub payment_collection_id: Uuid,
    pub plan_hash: String,
    pub currency_code: String,
    pub idempotency_key: String,
    pub request_hash: String,
    pub status: String,
    pub stage: String,
    pub attempt_count: i32,
    pub lease_owner: Option<String>,
    pub lease_expires_at: Option<DateTimeWithTimeZone>,
    pub ledger_transaction_id: Option<Uuid>,
    pub ledger_debit_total_amount: Option<i64>,
    pub ledger_credit_total_amount: Option<i64>,
    pub last_error_code: Option<String>,
    pub last_error_message: Option<String>,
    pub created_at: DateTimeWithTimeZone,
    pub updated_at: DateTimeWithTimeZone,
    pub completed_at: Option<DateTimeWithTimeZone>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
