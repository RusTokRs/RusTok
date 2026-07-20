use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "marketplace_commission_assessments")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub allocation_id: Uuid,
    pub order_id: Uuid,
    pub order_line_item_id: Uuid,
    pub seller_id: Uuid,
    pub listing_id: Uuid,
    pub rule_id: Uuid,
    pub rule_key: Uuid,
    pub rule_version: i32,
    pub currency_code: String,
    pub allocation_total_amount: i64,
    pub rate_bps: i32,
    pub fixed_amount: i64,
    pub commission_amount: i64,
    pub seller_proceeds_amount: i64,
    pub status: String,
    pub metadata: Json,
    pub assessed_at: DateTimeWithTimeZone,
    pub created_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
