use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "marketplace_commission_rules")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub rule_key: Uuid,
    pub version: i32,
    pub seller_id: Option<Uuid>,
    pub listing_id: Option<Uuid>,
    pub rate_bps: i32,
    pub fixed_amount: i64,
    pub currency_code: Option<String>,
    pub priority: i32,
    pub effective_from: DateTimeWithTimeZone,
    pub effective_until: Option<DateTimeWithTimeZone>,
    pub status: String,
    pub metadata: Json,
    pub created_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
