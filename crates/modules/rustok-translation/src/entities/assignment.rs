use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "translation_item_assignments")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub item_id: Uuid,
    pub operation: String,
    pub assignee_actor_kind: Option<String>,
    pub assignee_actor_id: Option<String>,
    pub requested_by_actor_kind: String,
    pub requested_by_actor_id: String,
    pub idempotency_key: String,
    pub request_hash: String,
    pub resulting_item_revision: i64,
    pub created_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
