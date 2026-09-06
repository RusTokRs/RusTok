use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "translation_glossaries")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub name: String,
    pub name_key: String,
    pub description: String,
    pub source_locale: String,
    pub target_locale: String,
    pub owner_slug: String,
    pub resource_kind: String,
    pub field_key: String,
    pub is_active: bool,
    pub revision: i64,
    pub last_idempotency_key: String,
    pub last_request_hash: String,
    pub created_by_actor_kind: String,
    pub created_by_actor_id: String,
    pub updated_by_actor_kind: String,
    pub updated_by_actor_id: String,
    pub created_at: DateTimeWithTimeZone,
    pub updated_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
