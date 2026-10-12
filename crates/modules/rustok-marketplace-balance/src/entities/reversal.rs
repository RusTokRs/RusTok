use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "marketplace_ledger_reversals")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub transaction_id: Uuid,
    pub reversed_transaction_id: Uuid,
    pub reversal_kind: String,
    pub source_id: Uuid,
    pub order_id: Uuid,
    pub currency_code: String,
    pub total_amount: i64,
    pub reversed_at: DateTimeWithTimeZone,
    pub metadata: Json,
    pub created_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
