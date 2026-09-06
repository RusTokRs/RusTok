use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "translation_machine_operations")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub item_id: Uuid,
    pub proposal_id: Option<Uuid>,
    pub status: String,
    pub command_hash: String,
    pub machine_request_digest: String,
    pub adapter_slug: String,
    pub provider_slug: Option<String>,
    pub provider_policy_digest: String,
    pub glossary_revision: Option<String>,
    pub glossary_digest: Option<String>,
    pub memory_digest: Option<String>,
    pub execution_id: Option<String>,
    pub execution_request_digest: Option<String>,
    pub prompt_policy_digest: Option<String>,
    pub attempts: Json,
    pub usage: Option<Json>,
    pub diagnostics: Json,
    pub review_required: Option<bool>,
    pub requested_by_actor_kind: String,
    pub requested_by_actor_id: String,
    pub idempotency_key: String,
    pub created_at: DateTimeWithTimeZone,
    pub updated_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
