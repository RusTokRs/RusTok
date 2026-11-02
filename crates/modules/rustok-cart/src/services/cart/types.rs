use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct DeliveryGroupKey {
    pub shipping_profile_slug: String,
    pub seller_id: Option<String>,
    pub seller_scope: Option<String>,
}

#[derive(Clone, Debug)]
pub struct DeliveryGroupSnapshot {
    pub key: DeliveryGroupKey,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CartLineItemPricingUpdate {
    pub line_item_id: Uuid,
    /// Quantity for which Pricing supplied this unit-price/adjustment observation.
    pub quantity: i32,
    pub unit_price: Decimal,
    pub pricing_adjustment: Option<CartPricingAdjustmentUpdate>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CartPricingAdjustmentUpdate {
    pub source_id: Option<String>,
    pub amount: Decimal,
    pub metadata: Value,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CartPromotionKind {
    PercentageDiscount,
    FixedDiscount,
}

#[derive(Clone, Debug)]
pub struct CartPromotionPreview {
    pub kind: CartPromotionKind,
    pub line_item_id: Option<Uuid>,
    pub currency_code: String,
    pub base_amount: Decimal,
    pub adjustment_amount: Decimal,
    pub adjusted_amount: Decimal,
}

/// Complete target-context price observation. It never authorizes a Cart mutation by itself.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CartContextPricingPlan {
    pub scope: crate::CartPricingScope,
    pub updates: Vec<CartLineItemPricingUpdate>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CartContextPricing {
    /// Owner checkout/materialization only; a nonempty Cart's unit-price basis must not change.
    Preserve,
    /// Buyer context changes require exact target scope and complete current-line coverage.
    Reprice(CartContextPricingPlan),
}
