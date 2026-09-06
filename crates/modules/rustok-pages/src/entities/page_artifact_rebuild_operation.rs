use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "page_artifact_rebuild_operations")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub page_id: Uuid,
    pub source_id: Uuid,
    pub source_publish_operation_id: Uuid,
    pub locale: String,
    pub idempotency_key: String,
    pub request_hash: String,
    pub expected_provenance_hash: String,
    pub review_hash: String,
    pub artifact_instance_key: String,
    pub source_artifact_id: Uuid,
    pub rebuilt_artifact_id: Uuid,
    pub rebuilt_artifact_hash: String,
    pub rebuilt_materialization_hash: String,
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
        belongs_to = "super::page_publish_rebuild_source::Entity",
        from = "Column::SourceId",
        to = "super::page_publish_rebuild_source::Column::Id"
    )]
    Source,
}

impl Related<super::page::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Page.def()
    }
}

impl Related<super::page_publish_rebuild_source::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Source.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
