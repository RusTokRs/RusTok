use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "page_rollback_operations")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub page_id: Uuid,
    pub idempotency_key: String,
    pub request_hash: String,
    pub target_publish_operation_id: Uuid,
    pub source_artifact_set_hash: String,
    pub target_artifact_set_hash: String,
    pub result_version: i32,
    pub rolled_back_at: DateTimeWithTimeZone,
    pub created_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::page::Entity",
        from = "Column::PageId",
        to = "super::page::Column::Id"
    )]
    Page,
    #[sea_orm(
        belongs_to = "super::page_publish_operation::Entity",
        from = "Column::TargetPublishOperationId",
        to = "super::page_publish_operation::Column::Id"
    )]
    TargetPublishOperation,
}

impl Related<super::page::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Page.def()
    }
}

impl Related<super::page_publish_operation::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::TargetPublishOperation.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
