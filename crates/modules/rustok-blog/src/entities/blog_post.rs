use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "blog_posts")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub author_id: Uuid,
    pub category_id: Option<Uuid>,
    pub status: String,
    pub slug: String,
    pub metadata: Json,
    pub featured_image_url: Option<String>,
    pub published_at: Option<DateTimeWithTimeZone>,
    pub created_at: DateTimeWithTimeZone,
    pub updated_at: DateTimeWithTimeZone,
    pub archived_at: Option<DateTimeWithTimeZone>,
    pub comment_count: i32,
    pub view_count: i32,
    pub version: i32,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(has_many = "super::blog_post_channel_visibility::Entity")]
    ChannelVisibility,
    #[sea_orm(has_many = "super::blog_post_translation::Entity")]
    Translations,
}

impl Related<super::blog_post_channel_visibility::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::ChannelVisibility.def()
    }
}

impl Related<super::blog_post_translation::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Translations.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
