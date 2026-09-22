use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "forum_topic_merge_solution_resolutions")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub tenant_id: Uuid,
    #[sea_orm(primary_key, auto_increment = false)]
    pub operation_id: Uuid,
    pub source_solution_reply_id: Uuid,
    pub target_solution_reply_id: Uuid,
    pub selected_solution_reply_id: Uuid,
    pub rejected_solution_reply_id: Uuid,
    pub rejected_solution_author_id: Option<Uuid>,
    pub resolved_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
