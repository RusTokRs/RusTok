use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "marketplace_ledger_reversal_lines")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub reversal_id: Uuid,
    pub entry_id: Uuid,
    pub reversed_entry_id: Uuid,
    pub seller_id: Option<Uuid>,
    pub assessment_id: Uuid,
    pub allocation_id: Uuid,
    pub order_line_item_id: Uuid,
    pub account_code: String,
    pub direction: String,
    pub seller_balance_bucket: Option<String>,
    pub amount: i64,
    pub created_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
