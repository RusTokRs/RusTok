use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;
use validator::Validate;

use super::AddCartLineItemInput;

#[derive(Debug, Clone, Serialize, Deserialize, Validate, ToSchema, PartialEq, Eq)]
pub struct MarketplaceCartLineSnapshotInput {
    pub seller_id: Uuid,
    pub listing_id: Uuid,
    pub master_product_id: Uuid,
    pub master_variant_id: Uuid,
    #[validate(range(min = 1))]
    pub listing_terms_version: i32,
    #[validate(length(equal = 3))]
    pub currency_code: String,
    #[validate(range(min = 0, max = 9))]
    pub currency_exponent: i16,
    #[validate(range(min = 0))]
    pub unit_amount: i64,
    #[validate(range(min = 0))]
    pub subtotal_amount: i64,
    #[validate(range(min = 0))]
    pub discount_amount: i64,
    #[validate(range(min = 0))]
    pub tax_amount: i64,
    #[validate(range(min = 0))]
    pub total_amount: i64,
    #[validate(length(max = 191))]
    pub pricing_reference: Option<String>,
    #[validate(length(max = 191))]
    pub inventory_reference: Option<String>,
    #[validate(length(min = 1, max = 100))]
    pub fulfillment_profile_slug: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Validate, ToSchema)]
pub struct AddMarketplaceCartLineItemInput {
    #[validate(nested)]
    pub line_item: AddCartLineItemInput,
    #[validate(nested)]
    pub marketplace: MarketplaceCartLineSnapshotInput,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, PartialEq, Eq)]
pub struct CartMarketplaceLineSnapshot {
    pub cart_line_item_id: Uuid,
    pub seller_id: Uuid,
    pub listing_id: Uuid,
    pub master_product_id: Uuid,
    pub master_variant_id: Uuid,
    pub listing_terms_version: i32,
    pub currency_code: String,
    pub currency_exponent: i16,
    pub unit_amount: i64,
    pub subtotal_amount: i64,
    pub discount_amount: i64,
    pub tax_amount: i64,
    pub total_amount: i64,
    pub pricing_reference: Option<String>,
    pub inventory_reference: Option<String>,
    pub fulfillment_profile_slug: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ListCartMarketplaceLineSnapshotsRequest {
    pub cart_id: Uuid,
}