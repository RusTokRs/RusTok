use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "pages")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub author_id: Option<Uuid>,
    pub status: String,
    pub template: String,
    pub metadata: Json,
    pub created_at: DateTimeWithTimeZone,
    pub updated_at: DateTimeWithTimeZone,
    pub published_at: Option<DateTimeWithTimeZone>,
    pub archived_at: Option<DateTimeWithTimeZone>,
    pub version: i32,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(has_many = "super::page_translation::Entity")]
    Translations,
    #[sea_orm(has_many = "super::page_body::Entity")]
    Bodies,
    #[sea_orm(has_many = "super::page_channel_visibility::Entity")]
    ChannelVisibility,
    #[sea_orm(has_many = "super::page_static_landing_artifact::Entity")]
    StaticLandingArtifacts,
    #[sea_orm(has_many = "super::page_publish_operation::Entity")]
    PublishOperations,
}

impl Related<super::page_translation::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Translations.def()
    }
}

impl Related<super::page_body::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Bodies.def()
    }
}

impl Related<super::page_channel_visibility::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::ChannelVisibility.def()
    }
}

impl Related<super::page_static_landing_artifact::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::StaticLandingArtifacts.def()
    }
}

impl Related<super::page_publish_operation::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::PublishOperations.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
