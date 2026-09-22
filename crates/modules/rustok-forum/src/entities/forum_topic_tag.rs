use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "forum_topic_tags")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub topic_id: Uuid,
    pub term_id: Uuid,
    pub tenant_id: Uuid,
    pub created_at: DateTimeWithTimeZone,
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
