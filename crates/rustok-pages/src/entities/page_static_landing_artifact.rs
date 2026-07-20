use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "page_static_landing_artifacts")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub page_id: Uuid,
    pub locale: String,
    pub source_hash: String,
    pub build_hash: String,
    pub artifact_hash: String,
    pub renderer_id: String,
    pub renderer_release: String,
    pub identity: Json,
    pub registry: Json,
    pub page_index: i32,
    pub fly_page_id: Option<String>,
    pub slug: Option<String>,
    pub head: Json,
    pub document_html: String,
    pub body_html: String,
    pub css: String,
    pub content_hash: String,
    pub landing_sections: Json,
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
    #[sea_orm(has_many = "super::page_published_landing_artifact::Entity")]
    PublishedBindings,
}

impl Related<super::page::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Page.def()
    }
}

impl Related<super::page_published_landing_artifact::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::PublishedBindings.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
