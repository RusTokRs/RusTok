use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "translation_policies")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub tenant_id: Uuid,
    pub required_target_locales: Json,
    pub tenant_locale_policy_revision: i64,
    pub revision: i64,
    pub last_idempotency_key: String,
    pub last_request_hash: String,
    pub updated_by_actor_kind: String,
    pub updated_by_actor_id: String,
    pub updated_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
