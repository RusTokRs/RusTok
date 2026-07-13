use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "checkout_order_plans")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub checkout_operation_id: Uuid,
    pub tenant_id: Uuid,
    pub snapshot_hash: String,
    pub plan_hash: String,
    pub payload: Json,
    pub created_at: DateTimeWithTimeZone,
    pub updated_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
