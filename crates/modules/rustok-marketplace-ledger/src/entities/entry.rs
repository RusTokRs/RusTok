use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "marketplace_ledger_entries")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub transaction_id: Uuid,
    pub order_id: Uuid,
    pub assessment_id: Uuid,
    pub allocation_id: Uuid,
    pub order_line_item_id: Uuid,
    pub seller_id: Option<Uuid>,
    pub account_code: String,
    pub direction: String,
    pub currency_code: String,
    pub amount: i64,
    pub metadata: Json,
    pub created_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
