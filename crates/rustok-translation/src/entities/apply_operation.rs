use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "translation_apply_operations")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub item_id: Uuid,
    pub proposal_id: Uuid,
    pub idempotency_key: String,
    pub request_hash: String,
    pub patch: Json,
    pub patch_digest: String,
    pub status: String,
    pub created_by_actor_kind: String,
    pub created_by_actor_id: String,
    pub applying_item_revision: i64,
    pub attempt_count: i64,
    pub last_error_kind: Option<String>,
    pub last_error_code: Option<String>,
    pub last_error_retryable: Option<bool>,
    pub lease_token: Option<Uuid>,
    pub lease_owner_actor_kind: Option<String>,
    pub lease_owner_actor_id: Option<String>,
    pub lease_expires_at: Option<DateTimeWithTimeZone>,
    pub created_at: DateTimeWithTimeZone,
    pub updated_at: DateTimeWithTimeZone,
    pub completed_at: Option<DateTimeWithTimeZone>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
