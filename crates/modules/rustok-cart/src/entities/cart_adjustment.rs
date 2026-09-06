use rust_decimal::Decimal;
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "cart_adjustments")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub cart_id: Uuid,
    pub cart_line_item_id: Option<Uuid>,
    pub source_type: String,
    pub source_id: Option<String>,
    pub amount: Decimal,
    pub currency_code: String,
    pub metadata: Json,
    pub created_at: DateTimeWithTimeZone,
    pub updated_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::cart::Entity",
        from = "Column::CartId",
        to = "super::cart::Column::Id"
    )]
    Cart,
    #[sea_orm(
        belongs_to = "super::cart_line_item::Entity",
        from = "Column::CartLineItemId",
        to = "super::cart_line_item::Column::Id"
    )]
    LineItem,
}

impl Related<super::cart::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Cart.def()
    }
}

impl Related<super::cart_line_item::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::LineItem.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
