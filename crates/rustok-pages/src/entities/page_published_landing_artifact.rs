use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "page_published_landing_artifacts")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub page_body_id: Uuid,
    pub artifact_id: Uuid,
    pub published_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::page_body::Entity",
        from = "Column::PageBodyId",
        to = "super::page_body::Column::Id"
    )]
    PageBody,
    #[sea_orm(
        belongs_to = "super::page_static_landing_artifact::Entity",
        from = "Column::ArtifactId",
        to = "super::page_static_landing_artifact::Column::Id"
    )]
    Artifact,
}

impl Related<super::page_body::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::PageBody.def()
    }
}

impl Related<super::page_static_landing_artifact::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Artifact.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
