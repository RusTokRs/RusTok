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

impl PriceResponse {
    /// Returns true if compare_at_amount exists and is greater than amount.
    #[inline]
    pub fn is_on_sale(&self) -> bool {
        self.compare_at_amount
            .is_some_and(|compare| compare > self.amount)
    }

    /// Calculates absolute discount amount if on sale.
    #[inline]
    pub fn discount_amount(&self) -> Option<Decimal> {
        self.compare_at_amount
            .filter(|compare| *compare > self.amount)
            .map(|compare| compare - self.amount)
    }

    /// Calculates percentage discount if on sale.
    pub fn discount_percentage(&self) -> Option<Decimal> {
        self.compare_at_amount.and_then(|compare| {
            if compare > self.amount && compare > Decimal::ZERO {
                let diff = compare - self.amount;
                Some((diff / compare) * Decimal::from(100))
            } else {
                None
            }
        })
    }
}

impl VariantResponse {
    /// Returns true if variant has positive stock or allows backorders.
    #[inline]
    pub fn is_available_to_sell(&self) -> bool {
        self.in_stock || self.inventory_policy == "continue"
    }

    /// Returns true if variant allows backorders.
    #[inline]
    pub fn allows_backorders(&self) -> bool {
        self.inventory_policy == "continue"
    }

    /// Finds price response for the requested currency code (case-insensitive).
    pub fn price_for_currency(&self, currency: &str) -> Option<&PriceResponse> {
        self.prices
            .iter()
            .find(|p| p.currency_code.eq_ignore_ascii_case(currency))
    }
}

impl PriceInput {
    pub fn validate_invariants(&self) -> Result<(), crate::error::CommerceError> {
        let trimmed_currency = self.currency_code.trim();
        if trimmed_currency.len() != 3
            || !trimmed_currency.chars().all(|c| c.is_ascii_alphabetic())
        {
            return Err(crate::error::CommerceError::validation(format!(
                "currency_code must be a 3-letter ISO-4217 code: '{}'",
                self.currency_code
            )));
        }
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
        if self
            .channel_slug
            .as_ref()
            .is_some_and(|slug| slug.trim().is_empty() || slug.len() > 100)
        {
            return Err(crate::error::CommerceError::validation(
                "channel_slug cannot be empty or exceed 100 characters",
            ));
        }
        Ok(())
    }
}

impl CreateVariantInput {
    pub fn validate_invariants(&self) -> Result<(), crate::error::CommerceError> {
        if self.inventory_quantity < 0 {
            return Err(crate::error::CommerceError::validation(
                "inventory_quantity cannot be negative on creation",
            ));
        }
        if self.inventory_policy.trim().is_empty() || self.inventory_policy.len() > 32 {
            return Err(crate::error::CommerceError::validation(
                "inventory_policy must be between 1 and 32 characters",
            ));
        }
        if self.weight.is_some_and(|w| w < rust_decimal::Decimal::ZERO) {
            return Err(crate::error::CommerceError::validation(
                "weight cannot be negative",
            ));
        }
        if self
            .weight_unit
            .as_ref()
            .is_some_and(|u| u.trim().is_empty() || u.len() > 16)
        {
            return Err(crate::error::CommerceError::validation(
                "weight_unit cannot be empty or exceed 16 characters",
            ));
        }
        if self
            .sku
            .as_ref()
            .is_some_and(|s| s.trim().is_empty() || s.len() > 100)
        {
            return Err(crate::error::CommerceError::validation(
                "sku cannot be blank or exceed 100 characters",
            ));
        }
        if self
            .barcode
            .as_ref()
            .is_some_and(|b| b.trim().is_empty() || b.len() > 100)
        {
            return Err(crate::error::CommerceError::validation(
                "barcode cannot be blank or exceed 100 characters",
            ));
        }
        if self
            .shipping_profile_slug
            .as_ref()
            .is_some_and(|p| p.trim().is_empty() || p.len() > 100)
        {
            return Err(crate::error::CommerceError::validation(
                "shipping_profile_slug cannot be blank or exceed 100 characters",
            ));
        }
        for price in &self.prices {
            price.validate_invariants()?;
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
        if self.adjustment == 0 {
            return Err(crate::error::CommerceError::validation(
                "inventory adjustment cannot be zero",
            ));
        }
        if self
            .reason
            .as_ref()
            .is_some_and(|r| r.trim().is_empty() || r.len() > 255)
        {
            return Err(crate::error::CommerceError::validation(
                "reason cannot be empty or exceed 255 characters",
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

        let invalid_currency = PriceInput {
            currency_code: "US1".to_string(),
            channel_id: None,
            channel_slug: None,
            amount: Decimal::new(1000, 2),
            compare_at_amount: None,
        };
        assert!(invalid_currency.validate_invariants().is_err());

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
    fn test_create_variant_input_validation() {
        let valid_create = CreateVariantInput {
            sku: Some("SKU-100".into()),
            barcode: Some("123456789".into()),
            shipping_profile_slug: Some("standard".into()),
            prices: vec![PriceInput {
                currency_code: "RUB".into(),
                channel_id: None,
                channel_slug: None,
                amount: Decimal::new(1500, 0),
                compare_at_amount: None,
            }],
            inventory_quantity: 10,
            inventory_policy: "deny".into(),
            weight: Some(Decimal::new(500, 0)),
            weight_unit: Some("g".into()),
        };
        assert!(valid_create.validate_invariants().is_ok());

        let negative_qty = CreateVariantInput {
            inventory_quantity: -5,
            ..valid_create.clone()
        };
        assert!(negative_qty.validate_invariants().is_err());

        let negative_weight = CreateVariantInput {
            weight: Some(Decimal::new(-1, 0)),
            ..valid_create
        };
        assert!(negative_weight.validate_invariants().is_err());
    }

    #[test]
    fn test_adjust_inventory_input_validation() {
        let valid = AdjustInventoryInput {
            variant_id: Uuid::new_v4(),
            adjustment: -2,
            reason: Some("damaged".to_string()),
        };
        assert!(valid.validate_invariants().is_ok());

        let zero_adjustment = AdjustInventoryInput {
            variant_id: Uuid::new_v4(),
            adjustment: 0,
            reason: None,
        };
        assert!(zero_adjustment.validate_invariants().is_err());

        let nil_variant = AdjustInventoryInput {
            variant_id: Uuid::nil(),
            adjustment: 5,
            reason: None,
        };
        assert!(nil_variant.validate_invariants().is_err());
    }

    #[test]
    fn test_price_response_sale_and_discount_calculation() {
        let on_sale_price = PriceResponse {
            currency_code: "USD".into(),
            amount: Decimal::new(80, 0),
            compare_at_amount: Some(Decimal::new(100, 0)),
            on_sale: true,
        };
        assert!(on_sale_price.is_on_sale());
        assert_eq!(on_sale_price.discount_amount(), Some(Decimal::new(20, 0)));
        assert_eq!(on_sale_price.discount_percentage(), Some(Decimal::new(20, 0)));

        let regular_price = PriceResponse {
            currency_code: "USD".into(),
            amount: Decimal::new(100, 0),
            compare_at_amount: None,
            on_sale: false,
        };
        assert!(!regular_price.is_on_sale());
        assert_eq!(regular_price.discount_amount(), None);
        assert_eq!(regular_price.discount_percentage(), None);
    }

    #[test]
    fn test_variant_response_helpers() {
        let variant = VariantResponse {
            id: Uuid::new_v4(),
            product_id: Uuid::new_v4(),
            sku: Some("SKU-TEST".into()),
            barcode: None,
            shipping_profile_slug: None,
            title: "Test Variant".into(),
            translations: vec![],
            combination_identity: None,
            prices: vec![
                PriceResponse {
                    currency_code: "USD".into(),
                    amount: Decimal::new(10, 0),
                    compare_at_amount: None,
                    on_sale: false,
                },
                PriceResponse {
                    currency_code: "EUR".into(),
                    amount: Decimal::new(9, 0),
                    compare_at_amount: None,
                    on_sale: false,
                },
            ],
            inventory_quantity: 0,
            inventory_policy: "continue".into(),
            in_stock: false,
            weight: None,
            weight_unit: None,
            position: 1,
        };

        assert!(variant.is_available_to_sell());
        assert!(variant.allows_backorders());
        assert!(variant.price_for_currency("usd").is_some());
        assert_eq!(
            variant.price_for_currency("EUR").unwrap().amount,
            Decimal::new(9, 0)
        );
        assert!(variant.price_for_currency("GBP").is_none());
    }
}
