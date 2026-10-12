use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "docs_pages")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub parent_id: Option<Uuid>,
    pub slug: String,
    pub title: String,
    pub icon: Option<String>,
    pub cover_image: Option<String>,
    pub is_archived: bool,
    pub created_at: DateTimeWithTimeZone,
    pub updated_at: DateTimeWithTimeZone,
    pub deleted_at: Option<DateTimeWithTimeZone>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "Entity",
        from = "Column::ParentId",
        to = "Column::Id",
        on_update = "NoAction",
        on_delete = "SetNull"
    )]
    Parent,
    #[sea_orm(has_many = "Entity")]
    Children,
    #[sea_orm(has_many = "super::snapshot::Entity")]
    Snapshots,
    #[sea_orm(has_many = "super::update::Entity")]
    Updates,
    #[sea_orm(has_many = "super::entity_link::Entity")]
    EntityLinks,
}

impl Related<super::snapshot::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Snapshots.def()
    }
}

impl Related<super::update::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Updates.def()
    }
}

impl Related<super::entity_link::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::EntityLinks.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
