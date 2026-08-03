use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Copy, Debug, PartialEq, Eq, EnumIter, DeriveActiveEnum, Serialize, Deserialize)]
#[sea_orm(rs_type = "String", db_type = "String(StringLen::N(32))")]
#[serde(rename_all = "snake_case")]
pub enum ForumTopicMergeAudienceOutcome {
    #[sea_orm(string_value = "both_unrestricted")]
    BothUnrestricted,
    #[sea_orm(string_value = "target_only_preserved")]
    TargetOnlyPreserved,
    #[sea_orm(string_value = "source_only_moved")]
    SourceOnlyMoved,
    #[sea_orm(string_value = "equal_layers_deduplicated")]
    EqualLayersDeduplicated,
}

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "forum_topic_merge_audience_reconciliations")]
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
    pub outcome: ForumTopicMergeAudienceOutcome,
    pub event_id: Uuid,
    pub reconciled_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
