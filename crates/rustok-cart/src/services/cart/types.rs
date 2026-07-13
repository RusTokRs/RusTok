use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fmt;
use uuid::Uuid;

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum CartStatus {
    Active,
    CheckingOut,
    Completed,
    Abandoned,
}

impl CartStatus {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Active => "active",
            Self::CheckingOut => "checking_out",
            Self::Completed => "completed",
            Self::Abandoned => "abandoned",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "active" => Some(Self::Active),
            "checking_out" => Some(Self::CheckingOut),
            "completed" => Some(Self::Completed),
            "abandoned" => Some(Self::Abandoned),
            _ => None,
        }
    }
}

impl fmt::Display for CartStatus {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

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

#[cfg(test)]
mod tests {
    use super::CartStatus;

    #[test]
    fn cart_status_round_trips_through_storage_and_json_names() {
        for status in [
            CartStatus::Active,
            CartStatus::CheckingOut,
            CartStatus::Completed,
            CartStatus::Abandoned,
        ] {
            assert_eq!(CartStatus::parse(status.as_str()), Some(status));
            assert_eq!(
                serde_json::to_string(&status).expect("status should serialize"),
                format!("\"{}\"", status.as_str())
            );
        }
        assert_eq!(CartStatus::parse("unknown"), None);
    }
}
