use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "order_line_item_translations")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub order_line_item_id: Uuid,
    pub locale: String,
    pub title: String,
    pub created_at: DateTimeWithTimeZone,
    pub updated_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::order_line_item::Entity",
        from = "Column::OrderLineItemId",
        to = "super::order_line_item::Column::Id"
    )]
    LineItem,
}

impl Related<super::order_line_item::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::LineItem.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
