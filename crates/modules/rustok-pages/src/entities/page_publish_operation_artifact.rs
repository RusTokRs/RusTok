use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "page_publish_operation_artifacts")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub operation_id: Uuid,
    pub tenant_id: Uuid,
    pub page_id: Uuid,
    pub locale: String,
    pub artifact_id: Uuid,
    pub artifact_hash: String,
    pub materialization_hash: Option<String>,
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
    #[sea_orm(
        belongs_to = "super::page_static_landing_artifact::Entity",
        from = "Column::ArtifactId",
        to = "super::page_static_landing_artifact::Column::Id"
    )]
    Artifact,
}

impl Related<super::page_publish_operation::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::PublishOperation.def()
    }
}

impl Related<super::page_static_landing_artifact::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Artifact.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
