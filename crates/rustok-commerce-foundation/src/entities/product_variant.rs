use rust_decimal::Decimal;
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "product_variants")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub product_id: Uuid,
    pub tenant_id: Uuid,
    pub sku: Option<String>,
    pub barcode: Option<String>,
    pub ean: Option<String>,
    pub upc: Option<String>,
    pub inventory_policy: String,
    pub inventory_management: String,
    pub inventory_quantity: i32,
    pub weight: Option<Decimal>,
    pub weight_unit: Option<String>,
    pub option1: Option<String>,
    pub option2: Option<String>,
    pub option3: Option<String>,
    pub position: i32,
    pub created_at: DateTimeWithTimeZone,
    pub updated_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::product::Entity",
        from = "Column::ProductId",
        to = "super::product::Column::Id"
    )]
    Product,
    #[sea_orm(has_many = "super::price::Entity")]
    Prices,
    #[sea_orm(has_many = "super::variant_translation::Entity")]
    Translations,
}

impl Related<super::product::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Product.def()
    }
}

impl Related<super::price::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Prices.def()
    }
}

impl Related<super::variant_translation::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Translations.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
