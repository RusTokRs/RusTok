use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "blog_post_translations")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub post_id: Uuid,
    pub locale: String,
    pub title: String,
    pub excerpt: Option<String>,
    pub seo_title: Option<String>,
    pub seo_description: Option<String>,
    pub body: String,
    pub created_at: DateTimeWithTimeZone,
    pub updated_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::blog_post::Entity",
        from = "Column::PostId",
        to = "super::blog_post::Column::Id",
        on_update = "Cascade",
        on_delete = "Cascade"
    )]
    Post,
}

impl Related<super::blog_post::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Post.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
