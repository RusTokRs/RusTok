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
