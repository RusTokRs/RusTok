use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "blog_post_tags")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub post_id: Uuid,
    #[sea_orm(primary_key, auto_increment = false)]
    pub tag_id: Uuid,
    pub tenant_id: Uuid,
    pub created_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::blog_post::Entity",
        from = "Column::PostId",
        to = "super::blog_post::Column::Id"
    )]
    Post,
}

impl Related<super::blog_post::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Post.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
