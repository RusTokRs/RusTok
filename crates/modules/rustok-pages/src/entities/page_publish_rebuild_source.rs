use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "page_publish_rebuild_sources")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub operation_id: Uuid,
    pub tenant_id: Uuid,
    pub page_id: Uuid,
    pub page_body_id: Uuid,
    pub locale: String,
    pub artifact_id: Uuid,
    pub source_format: String,
    pub source_revision: String,
    pub sanitized_project: Json,
    pub sanitized_hash: String,
    pub source_hash: String,
    pub review_hash: String,
    pub artifact_hash: String,
    pub materialization_hash: String,
    pub materialization_identity: Json,
    pub runtime_snapshots: Json,
    pub provenance_hash: String,
    pub created_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::page_publish_operation::Entity",
        from = "Column::OperationId",
        to = "super::page_publish_operation::Column::Id"
    )]
    PublishOperation,
}

impl Related<super::page_publish_operation::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::PublishOperation.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
