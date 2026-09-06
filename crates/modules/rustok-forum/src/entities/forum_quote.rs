use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "forum_quotes")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub tenant_id: Uuid,
    #[sea_orm(primary_key, auto_increment = false)]
    pub source_kind: String,
    #[sea_orm(primary_key, auto_increment = false)]
    pub source_id: Uuid,
    #[sea_orm(primary_key, auto_increment = false)]
    pub source_locale: String,
    #[sea_orm(primary_key, auto_increment = false)]
    pub source_revision_id: i64,
    #[sea_orm(primary_key, auto_increment = false)]
    pub quoted_kind: String,
    #[sea_orm(primary_key, auto_increment = false)]
    pub quoted_id: Uuid,
    #[sea_orm(primary_key, auto_increment = false)]
    pub quoted_revision_id: i64,
    pub created_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
