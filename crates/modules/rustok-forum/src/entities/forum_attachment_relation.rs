use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "forum_attachment_relations")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub reference_id: Uuid,
    pub tenant_id: Uuid,
    pub target_kind: String,
    pub target_id: Uuid,
    pub locale: String,
    pub position: i32,
    pub media_id: Uuid,
    pub usage: String,
    pub caption: Option<String>,
    pub created_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
