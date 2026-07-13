use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::subscription::ForumSubscriptionLevel;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "forum_subscription_policies")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub tenant_id: Uuid,
    pub auto_subscribe_topic_authors: bool,
    pub topic_author_level: ForumSubscriptionLevel,
    pub auto_subscribe_reply_participants: bool,
    pub reply_participant_level: ForumSubscriptionLevel,
    pub revision: i64,
    pub created_at: DateTimeWithTimeZone,
    pub updated_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
