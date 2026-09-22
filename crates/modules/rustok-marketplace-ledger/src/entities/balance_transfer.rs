use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "marketplace_seller_balance_transfers")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub transaction_id: Uuid,
    pub transfer_kind: String,
    pub source_id: Uuid,
    pub seller_id: Uuid,
    pub currency_code: String,
    pub from_bucket: String,
    pub to_bucket: String,
    pub total_amount: i64,
    pub transferred_at: DateTimeWithTimeZone,
    pub metadata: Json,
    pub created_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
