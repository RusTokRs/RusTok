use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::subscription::{ForumDigestMode, ForumSubscriptionLevel};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "forum_topic_subscriptions")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub topic_id: Uuid,
    #[sea_orm(primary_key, auto_increment = false)]
    pub user_id: Uuid,
    #[sea_orm(primary_key, auto_increment = false)]
    pub tenant_id: Uuid,
    pub level: ForumSubscriptionLevel,
    pub notify_mentions: bool,
    pub notify_replies: bool,
    pub notify_new_topics: bool,
    pub digest_mode: ForumDigestMode,
    pub last_notified_at: Option<DateTimeWithTimeZone>,
    pub revision: i64,
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
