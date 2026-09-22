use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "marketplace_ledger_transactions")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub source_kind: String,
    pub source_id: Uuid,
    pub order_id: Uuid,
    pub currency_code: String,
    pub debit_total_amount: i64,
    pub credit_total_amount: i64,
    pub status: String,
    pub posted_at: DateTimeWithTimeZone,
    pub metadata: Json,
    pub created_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
