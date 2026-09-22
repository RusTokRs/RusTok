use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "translation_job_items")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub job_id: Uuid,
    pub owner_slug: String,
    pub resource_kind: String,
    pub resource_id: String,
    pub subresource_key: String,
    pub resource_revision: String,
    pub source_revision: String,
    pub target_revision: Option<String>,
    pub source_snapshot: Json,
    pub source_digest: String,
    pub status: String,
    pub current_proposal_id: Option<Uuid>,
    pub active_apply_operation_id: Option<Uuid>,
    pub assigned_actor_kind: Option<String>,
    pub assigned_actor_id: Option<String>,
    pub idempotency_key: String,
    pub request_hash: String,
    pub revision: i64,
    pub created_at: DateTimeWithTimeZone,
    pub updated_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
