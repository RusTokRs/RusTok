use chrono::{DateTime, FixedOffset};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

pub const MAX_ALLOCATION_LINES_PER_COMMAND: usize = 500;
pub const MAX_ALLOCATIONS_PER_PAGE: u64 = 200;

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum MarketplaceAllocationStatus {
    Allocated,
    Cancelled,
}

impl MarketplaceAllocationStatus {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Allocated => "allocated",
            Self::Cancelled => "cancelled",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "allocated" => Some(Self::Allocated),
            "cancelled" => Some(Self::Cancelled),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq, ToSchema)]
pub struct AllocateMarketplaceOrderLineInput {
    pub order_line_item_id: Uuid,
    pub seller_id: Uuid,
    pub listing_id: Uuid,
    pub master_product_id: Uuid,
    pub master_variant_id: Uuid,
    pub quantity: i64,
    pub unit_amount: i64,
    pub subtotal_amount: i64,
    pub discount_amount: i64,
    pub tax_amount: i64,
    pub total_amount: i64,
    pub listing_terms_version: i32,
    pub pricing_reference: Option<String>,
    pub inventory_reference: Option<String>,
    pub fulfillment_profile_slug: Option<String>,
    #[serde(default = "empty_object")]
    pub metadata: serde_json::Value,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq, ToSchema)]
pub struct AllocateMarketplaceOrderLinesInput {
    pub order_id: Uuid,
    pub currency_code: String,
    pub lines: Vec<AllocateMarketplaceOrderLineInput>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq, ToSchema)]
pub struct MarketplaceOrderAllocationResponse {
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
    pub status: MarketplaceAllocationStatus,
    pub metadata: serde_json::Value,
    pub created_at: DateTime<FixedOffset>,
    pub updated_at: DateTime<FixedOffset>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq, ToSchema)]
pub struct AllocateMarketplaceOrderLinesResponse {
    pub order_id: Uuid,
    pub currency_code: String,
    pub allocations: Vec<MarketplaceOrderAllocationResponse>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq, ToSchema)]
pub struct ReadMarketplaceAllocationByLineRequest {
    pub order_line_item_id: Uuid,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq, ToSchema)]
pub struct ListMarketplaceAllocationsByOrderRequest {
    pub order_id: Uuid,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq, ToSchema)]
pub struct ListMarketplaceAllocationsBySellerRequest {
    pub seller_id: Uuid,
    pub page: u64,
    pub per_page: u64,
    pub status: Option<MarketplaceAllocationStatus>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq, ToSchema)]
pub struct MarketplaceAllocationListResponse {
    pub items: Vec<MarketplaceOrderAllocationResponse>,
    pub total: u64,
    pub page: u64,
    pub per_page: u64,
}

fn empty_object() -> serde_json::Value {
    serde_json::json!({})
}
