use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "forum_solutions")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub topic_id: Uuid,
    #[sea_orm(primary_key, auto_increment = false)]
    pub tenant_id: Uuid,
    pub reply_id: Uuid,
    pub marked_by_user_id: Option<Uuid>,
    pub marked_at: DateTimeWithTimeZone,
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
    #[sea_orm(
        belongs_to = "super::forum_reply::Entity",
        from = "Column::ReplyId",
        to = "super::forum_reply::Column::Id",
        on_update = "Cascade",
        on_delete = "Cascade"
    )]
    Reply,
}

impl Related<super::forum_topic::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Topic.def()
    }
}

impl Related<super::forum_reply::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Reply.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
