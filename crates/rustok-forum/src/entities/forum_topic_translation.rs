use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "forum_topic_translations")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub topic_id: Uuid,
    pub tenant_id: Uuid,
    pub locale: String,
    pub title: String,
    pub slug: Option<String>,
    pub body: String,
    pub body_format: String,
    pub created_at: DateTimeWithTimeZone,
    pub updated_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::forum_topic::Entity",
        from = "Column::TopicId",
        to = "super::forum_topic::Column::Id",
        on_update = "Cascade",
        on_delete = "Cascade"
    )]
    Topic,
}

impl Related<super::forum_topic::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Topic.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
