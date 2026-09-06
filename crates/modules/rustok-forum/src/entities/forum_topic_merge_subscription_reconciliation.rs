use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "forum_topic_merge_subscription_reconciliations")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub tenant_id: Uuid,
    #[sea_orm(primary_key, auto_increment = false)]
    pub operation_id: Uuid,
    pub merge_operation_id: Uuid,
    pub source_topic_id: Uuid,
    pub target_topic_id: Uuid,
    pub actor_id: Uuid,
    pub reason: String,
    pub source_subscription_count: i32,
    pub moved_source_only_count: i32,
    pub deduplicated_equal_count: i32,
    pub target_authority_conflict_count: i32,
    pub event_id: Uuid,
    pub reconciled_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
