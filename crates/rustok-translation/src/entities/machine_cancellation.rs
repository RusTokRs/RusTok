use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "translation_machine_cancellations")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub operation_id: Uuid,
    pub reason: String,
    pub requested_by_actor_kind: String,
    pub requested_by_actor_id: String,
    pub idempotency_key: String,
    pub request_hash: String,
    pub provider_execution_id: Option<String>,
    pub provider_status: String,
    pub provider_error_code: Option<String>,
    pub provider_observed_at: DateTimeWithTimeZone,
    pub created_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
