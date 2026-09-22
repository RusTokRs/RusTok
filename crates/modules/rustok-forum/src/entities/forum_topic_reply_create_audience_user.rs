use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::forum_category_audience_user::ForumCategoryAudienceUserEffect;

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "forum_topic_reply_create_audience_users")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub tenant_id: Uuid,
    #[sea_orm(primary_key, auto_increment = false)]
    pub topic_id: Uuid,
    #[sea_orm(primary_key, auto_increment = false)]
    pub user_id: Uuid,
    #[sea_orm(primary_key, auto_increment = false)]
    pub effect: ForumCategoryAudienceUserEffect,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::forum_topic_reply_create_audience_policy::Entity",
        from = "Column::TopicId",
        to = "super::forum_topic_reply_create_audience_policy::Column::TopicId",
        on_update = "Cascade",
        on_delete = "Cascade"
    )]
    Policy,
}

impl Related<super::forum_topic_reply_create_audience_policy::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Policy.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
