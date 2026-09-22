use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "translation_workflow_notes")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub job_id: Uuid,
    pub item_id: Option<Uuid>,
    pub body: String,
    pub created_by_actor_kind: String,
    pub created_by_actor_id: String,
    pub idempotency_key: String,
    pub request_hash: String,
    pub revision: i64,
    pub resolved_at: Option<DateTimeWithTimeZone>,
    pub resolved_by_actor_kind: Option<String>,
    pub resolved_by_actor_id: Option<String>,
    pub resolution_idempotency_key: Option<String>,
    pub resolution_request_hash: Option<String>,
    pub created_at: DateTimeWithTimeZone,
    pub updated_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
