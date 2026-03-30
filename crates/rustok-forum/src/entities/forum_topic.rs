use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "forum_topics")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub category_id: Uuid,
    pub author_id: Option<Uuid>,
    pub status: String,
    pub is_pinned: bool,
    pub is_locked: bool,
    pub reply_count: i32,
    pub created_at: DateTimeWithTimeZone,
    pub updated_at: DateTimeWithTimeZone,
    pub last_reply_at: Option<DateTimeWithTimeZone>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::forum_category::Entity",
        from = "Column::CategoryId",
        to = "super::forum_category::Column::Id",
        on_update = "Cascade",
        on_delete = "Cascade"
    )]
    Category,
    #[sea_orm(has_many = "super::forum_topic_translation::Entity")]
    Translations,
    #[sea_orm(has_many = "super::forum_topic_channel_access::Entity")]
    ChannelAccess,
    #[sea_orm(has_many = "super::forum_topic_tag::Entity")]
    TopicTags,
    #[sea_orm(has_many = "super::forum_reply::Entity")]
    Replies,
}

impl Related<super::forum_category::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Category.def()
    }
}

impl Related<super::forum_topic_translation::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Translations.def()
    }
}

impl Related<super::forum_topic_channel_access::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::ChannelAccess.def()
    }
}

impl Related<super::forum_topic_tag::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::TopicTags.def()
    }
}

impl Related<super::forum_reply::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Replies.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
