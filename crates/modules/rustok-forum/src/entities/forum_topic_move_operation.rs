use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "forum_topic_move_operations")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub tenant_id: Uuid,
    #[sea_orm(primary_key, auto_increment = false)]
    pub operation_id: Uuid,
    pub topic_id: Uuid,
    pub source_category_id: Uuid,
    pub target_category_id: Uuid,
    pub actor_id: Uuid,
    pub reason: String,
    pub published_reply_count: i32,
    pub event_id: Uuid,
    pub moved_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
