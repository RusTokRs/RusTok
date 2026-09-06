use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "marketplace_order_allocations")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub order_id: Uuid,
    pub order_line_item_id: Uuid,
    pub seller_id: Uuid,
    pub listing_id: Uuid,
    pub master_product_id: Uuid,
    pub master_variant_id: Uuid,
    pub quantity: i64,
    pub currency_code: String,
    pub unit_amount: i64,
    pub subtotal_amount: i64,
    pub discount_amount: i64,
    pub tax_amount: i64,
    pub total_amount: i64,
    pub listing_terms_version: i32,
    pub pricing_reference: Option<String>,
    pub inventory_reference: Option<String>,
    pub fulfillment_profile_slug: Option<String>,
    pub status: String,
    pub metadata: Json,
    pub created_at: DateTimeWithTimeZone,
    pub updated_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
