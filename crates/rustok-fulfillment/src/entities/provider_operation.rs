use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "fulfillment_provider_operations")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub fulfillment_id: Uuid,
    pub operation: String,
    pub provider_id: String,
    pub idempotency_key: String,
    pub status: String,
    pub request_payload: Json,
    pub provider_reference: Option<String>,
    pub provider_result: Option<Json>,
    pub error_message: Option<String>,
    pub created_at: DateTimeWithTimeZone,
    pub updated_at: DateTimeWithTimeZone,
    pub provider_completed_at: Option<DateTimeWithTimeZone>,
    pub committed_at: Option<DateTimeWithTimeZone>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
