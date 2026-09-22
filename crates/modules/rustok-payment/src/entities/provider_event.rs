use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "payment_provider_events")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub provider_id: String,
    pub delivery_id: String,
    pub idempotency_key: String,
    pub payload_hash: String,
    pub signature_verified: bool,
    pub status: String,
    pub event_type: Option<String>,
    pub external_reference: Option<String>,
    pub event_metadata: Option<Json>,
    pub attempt_count: i32,
    pub lease_owner: Option<String>,
    pub lease_expires_at: Option<DateTimeWithTimeZone>,
    pub error_code: Option<String>,
    pub error_message: Option<String>,
    pub received_at: DateTimeWithTimeZone,
    pub updated_at: DateTimeWithTimeZone,
    pub processed_at: Option<DateTimeWithTimeZone>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
