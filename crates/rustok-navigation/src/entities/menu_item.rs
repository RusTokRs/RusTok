use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "menu_items")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub menu_id: Uuid,
    pub tenant_id: Uuid,
    pub parent_item_id: Option<Uuid>,
    pub page_id: Option<Uuid>,
    pub position: i32,
    pub url: String,
    pub icon: Option<String>,
    pub created_at: DateTimeWithTimeZone,
    pub updated_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::menu::Entity",
        from = "Column::MenuId",
        to = "super::menu::Column::Id"
    )]
    Menu,
    #[sea_orm(
        belongs_to = "Entity",
        from = "Column::ParentItemId",
        to = "Column::Id"
    )]
    Parent,
    #[sea_orm(has_many = "super::menu_item_translation::Entity")]
    Translations,
}

impl Related<super::menu::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Menu.def()
    }
}

impl Related<super::menu_item_translation::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Translations.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
