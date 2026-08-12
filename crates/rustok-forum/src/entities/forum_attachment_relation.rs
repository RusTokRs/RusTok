use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// One ordered Media reference in an immutable Forum attachment projection.
///
/// There is deliberately no database foreign key into Media-owned tables.
/// Referenceability is an owner decision and must be admitted through the Media
/// port before the Forum persistence entrypoint is called.
#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "forum_attachment_relations")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub tenant_id: Uuid,
    #[sea_orm(primary_key, auto_increment = false)]
    pub target_kind: String,
    #[sea_orm(primary_key, auto_increment = false)]
    pub target_id: Uuid,
    #[sea_orm(primary_key, auto_increment = false)]
    pub source_revision: i64,
    #[sea_orm(primary_key, auto_increment = false)]
    pub locale: String,
    #[sea_orm(primary_key, auto_increment = false)]
    pub position: i32,
    pub media_id: Uuid,
    pub usage: String,
    pub caption: Option<String>,
    pub created_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
