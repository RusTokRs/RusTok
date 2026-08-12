use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Forum-owned typed relation from a category to one Media asset.
///
/// `media_id` deliberately has no database foreign key into Media: Forum owns
/// only the reference while Media owns asset lifecycle and delivery eligibility.
#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "forum_category_covers")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub category_id: Uuid,
    pub tenant_id: Uuid,
    pub media_id: Uuid,
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
