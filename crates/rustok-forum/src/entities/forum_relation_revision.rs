use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "forum_relation_revisions")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub revision_id: i64,
    pub tenant_id: Uuid,
    pub target_kind: String,
    pub target_id: Uuid,
    pub locale: String,
    pub projection_fingerprint: String,
    pub created_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
