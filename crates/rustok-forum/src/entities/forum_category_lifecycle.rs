use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "forum_category_lifecycle")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub category_id: Uuid,
    pub tenant_id: Uuid,
    pub archived_at: DateTimeWithTimeZone,
    pub updated_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::forum_category::Entity",
        from = "Column::CategoryId",
        to = "super::forum_category::Column::Id",
        on_update = "Cascade",
        on_delete = "Cascade"
    )]
    Category,
}

impl Related<super::forum_category::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Category.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
