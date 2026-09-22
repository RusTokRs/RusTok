use rust_decimal::Decimal;
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "order_line_items")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub order_id: Uuid,
    pub product_id: Option<Uuid>,
    pub variant_id: Option<Uuid>,
    pub shipping_profile_slug: String,
    pub seller_id: Option<String>,
    pub sku: Option<String>,
    pub quantity: i32,
    pub unit_price: Decimal,
    pub total_price: Decimal,
    pub currency_code: String,
    pub metadata: Json,
    pub created_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::order::Entity",
        from = "Column::OrderId",
        to = "super::order::Column::Id"
    )]
    Order,
    #[sea_orm(has_many = "super::order_adjustment::Entity")]
    Adjustments,
    #[sea_orm(has_many = "super::order_tax_line::Entity")]
    TaxLines,
    #[sea_orm(has_many = "super::order_line_item_translation::Entity")]
    Translations,
}

impl Related<super::order::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Order.def()
    }
}

impl Related<super::order_adjustment::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Adjustments.def()
    }
}

impl Related<super::order_tax_line::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::TaxLines.def()
    }
}

impl Related<super::order_line_item_translation::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Translations.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
