use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "marketplace_seller_balance_projections")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub seller_id: Uuid,
    pub currency_code: String,
    pub pending_amount: i64,
    pub available_amount: i64,
    pub reserved_amount: i64,
    pub paid_amount: i64,
    pub negative_amount: i64,
    pub source_entry_count: i64,
    pub last_entry_id: Option<Uuid>,
    pub last_entry_created_at: Option<DateTimeWithTimeZone>,
    pub rebuilt_at: DateTimeWithTimeZone,
    pub updated_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
