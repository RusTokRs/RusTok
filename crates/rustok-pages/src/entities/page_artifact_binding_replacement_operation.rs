use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "page_artifact_binding_replacement_operations")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub page_id: Uuid,
    pub rebuild_operation_id: Uuid,
    pub page_body_id: Uuid,
    pub locale: String,
    pub idempotency_key: String,
    pub request_hash: String,
    pub expected_version: i32,
    pub expected_current_artifact_id: Uuid,
    pub replacement_artifact_id: Uuid,
    pub replacement_artifact_hash: String,
    pub replacement_materialization_hash: String,
    pub result_version: i32,
    pub replaced_at: DateTimeWithTimeZone,
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
        belongs_to = "super::page_artifact_rebuild_operation::Entity",
        from = "Column::RebuildOperationId",
        to = "super::page_artifact_rebuild_operation::Column::Id"
    )]
    RebuildOperation,
}

impl Related<super::page::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Page.def()
    }
}

impl Related<super::page_artifact_rebuild_operation::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::RebuildOperation.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
