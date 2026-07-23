use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "moderation_decision_effects")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub decision_id: Uuid,
    pub tenant_id: Uuid,
    pub schema_version: i32,
    pub effect_kind: String,
    pub effect_payload: Json,
    pub created_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
