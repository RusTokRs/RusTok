use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "checkout_operations")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub cart_id: Uuid,
    pub idempotency_key: String,
    pub request_hash: String,
    pub snapshot_hash: Option<String>,
    pub status: String,
    pub stage: String,
    pub order_id: Option<Uuid>,
    pub payment_collection_id: Option<Uuid>,
    pub attempt_count: i32,
    pub lease_owner: Option<String>,
    pub lease_expires_at: Option<DateTimeWithTimeZone>,
    pub last_error_code: Option<String>,
    pub last_error_message: Option<String>,
    pub created_at: DateTimeWithTimeZone,
    pub updated_at: DateTimeWithTimeZone,
    pub completed_at: Option<DateTimeWithTimeZone>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
