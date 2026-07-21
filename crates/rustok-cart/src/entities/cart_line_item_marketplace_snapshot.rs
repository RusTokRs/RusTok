use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "cart_line_item_marketplace_snapshots")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub cart_line_item_id: Uuid,
    pub seller_id: Uuid,
    pub listing_id: Uuid,
    pub master_product_id: Uuid,
    pub master_variant_id: Uuid,
    pub listing_terms_version: i32,
    pub unit_amount: i64,
    pub subtotal_amount: i64,
    pub discount_amount: i64,
    pub tax_amount: i64,
    pub total_amount: i64,
    pub pricing_reference: Option<String>,
    pub inventory_reference: Option<String>,
    pub fulfillment_profile_slug: Option<String>,
    pub created_at: DateTimeWithTimeZone,
    pub updated_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::cart_line_item::Entity",
        from = "Column::CartLineItemId",
        to = "super::cart_line_item::Column::Id",
        on_update = "Cascade",
        on_delete = "Cascade"
    )]
    CartLineItem,
}

impl Related<super::cart_line_item::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::CartLineItem.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}