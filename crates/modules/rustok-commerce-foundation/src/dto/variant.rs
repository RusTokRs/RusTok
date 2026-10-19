use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;
use validator::Validate;

/// Scoped price projection shared by variant-level reads.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct PriceResponse {
    pub currency_code: String,
    pub amount: Decimal,
    pub compare_at_amount: Option<Decimal>,
    pub on_sale: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, Validate)]
pub struct CreateVariantInput {
    #[validate(length(max = 100, message = "SKU must be max 100 characters"))]
    pub sku: Option<String>,
    #[validate(length(max = 100, message = "Barcode must be max 100 characters"))]
    pub barcode: Option<String>,
    #[validate(length(
        max = 100,
        message = "Shipping profile slug must be max 100 characters"
    ))]
    pub shipping_profile_slug: Option<String>,
    #[validate(nested)]
    pub prices: Vec<PriceInput>,
    #[serde(default)]
    pub inventory_quantity: i32,
    #[serde(default = "default_inventory_policy")]
    #[validate(length(
        min = 1,
        max = 32,
        message = "Inventory policy must be 1-32 characters"
    ))]
    pub inventory_policy: String,
    pub weight: Option<Decimal>,
    #[validate(length(max = 16, message = "Weight unit must be max 16 characters"))]
    pub weight_unit: Option<String>,
}

fn default_inventory_policy() -> String {
    "deny".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, Validate)]
pub struct PriceInput {
    #[validate(length(
        min = 3,
        max = 3,
        message = "Currency code must be exactly 3 characters (ISO 4217)"
    ))]
    pub currency_code: String,
    pub channel_id: Option<Uuid>,
    #[validate(length(max = 100, message = "Channel slug must be max 100 characters"))]
    pub channel_slug: Option<String>,
    pub amount: Decimal,
    pub compare_at_amount: Option<Decimal>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct VariantResponse {
    pub id: Uuid,
    pub product_id: Uuid,
    pub sku: Option<String>,
    pub barcode: Option<String>,
    pub shipping_profile_slug: Option<String>,
    pub title: String,
    #[serde(default)]
    pub translations: Vec<VariantTranslationResponse>,
    pub combination_identity: Option<String>,
    pub prices: Vec<PriceResponse>,
    pub inventory_quantity: i32,
    pub inventory_policy: String,
    pub in_stock: bool,
    pub weight: Option<Decimal>,
    pub weight_unit: Option<String>,
    pub position: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct VariantTranslationResponse {
    pub locale: String,
    pub title: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, Validate)]
pub struct AdjustInventoryInput {
    pub variant_id: Uuid,
    pub adjustment: i32,
    pub reason: Option<String>,
}

impl PriceInput {
    pub fn validate_invariants(&self) -> Result<(), crate::error::CommerceError> {
        if self.amount < rust_decimal::Decimal::ZERO {
            return Err(crate::error::CommerceError::InvalidPrice(format!(
                "price amount cannot be negative: {}",
                self.amount
            )));
        }
        if let Some(compare_at) = self
            .compare_at_amount
            .filter(|compare_at| *compare_at < rust_decimal::Decimal::ZERO)
        {
            return Err(crate::error::CommerceError::InvalidPrice(format!(
                "compare_at_amount cannot be negative: {compare_at}",
            )));
        }
        if self
            .channel_id
            .is_some_and(|channel_id| channel_id.is_nil())
        {
            return Err(crate::error::CommerceError::validation(
                "channel_id must not be nil UUID",
            ));
        }
        Ok(())
    }
}

impl AdjustInventoryInput {
    pub fn validate_invariants(&self) -> Result<(), crate::error::CommerceError> {
        if self.variant_id.is_nil() {
            return Err(crate::error::CommerceError::validation(
                "variant_id must not be nil UUID",
            ));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_price_input_validation() {
        let valid = PriceInput {
            currency_code: "USD".to_string(),
            channel_id: Some(Uuid::new_v4()),
            channel_slug: None,
            amount: Decimal::new(1999, 2),
            compare_at_amount: Some(Decimal::new(2499, 2)),
        };
        assert!(valid.validate_invariants().is_ok());

        let negative_amount = PriceInput {
            currency_code: "USD".to_string(),
            channel_id: None,
            channel_slug: None,
            amount: Decimal::new(-500, 2),
            compare_at_amount: None,
        };
        assert!(negative_amount.validate_invariants().is_err());

        let nil_channel = PriceInput {
            currency_code: "USD".to_string(),
            channel_id: Some(Uuid::nil()),
            channel_slug: None,
            amount: Decimal::new(1000, 2),
            compare_at_amount: None,
        };
        assert!(nil_channel.validate_invariants().is_err());
    }

    #[test]
    fn test_adjust_inventory_input_validation() {
        let valid = AdjustInventoryInput {
            variant_id: Uuid::new_v4(),
            adjustment: -2,
            reason: Some("damaged".to_string()),
        };
        assert!(valid.validate_invariants().is_ok());

        let nil_variant = AdjustInventoryInput {
            variant_id: Uuid::nil(),
            adjustment: 5,
            reason: None,
        };
        assert!(nil_variant.validate_invariants().is_err());
    }
}
