use rust_decimal::Decimal;
use rustok_api::Patch;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;
use validator::Validate;

use super::{PriceResponse, VariantAxisValueInput, VariantAxisValueResponse};

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
    #[serde(default)]
    // Keep this literal aligned with MAX_VARIANT_AXES in dto/variant_axis.rs.
    #[validate(length(max = 32, message = "A variant can have at most 32 axis assignments"))]
    #[validate(nested)]
    pub axis_values: Vec<VariantAxisValueInput>,
    // Keep this literal aligned with MAX_VARIANT_PRICE_ROWS in dto/mod.rs.
    #[validate(length(max = 64, message = "A variant can have at most 64 price rows"))]
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
        max = 5,
        message = "Currency code must be 3 letters with optional surrounding whitespace"
    ))]
    pub currency_code: String,
    pub channel_id: Option<Uuid>,
    #[validate(length(
        min = 1,
        max = 100,
        message = "Channel slug must be 1-100 characters"
    ))]
    pub channel_slug: Option<String>,
    pub amount: Decimal,
    pub compare_at_amount: Option<Decimal>,
}

/// Nullable fields distinguish omission (Keep), null (Clear), and a concrete value (Set).
#[derive(Debug, Clone, Serialize, Deserialize, Default, ToSchema, Validate)]
pub struct UpdateVariantInput {
    #[serde(default, skip_serializing_if = "Patch::is_keep")]
    #[schema(value_type = Option<String>)]
    pub sku: Patch<String>,
    #[serde(default, skip_serializing_if = "Patch::is_keep")]
    #[schema(value_type = Option<String>)]
    pub barcode: Patch<String>,
    #[serde(default, skip_serializing_if = "Patch::is_keep")]
    #[schema(value_type = Option<String>)]
    pub shipping_profile_slug: Patch<String>,
    // Keep this literal aligned with MAX_VARIANT_PRICE_ROWS in dto/mod.rs.
    #[validate(length(max = 64, message = "A variant can have at most 64 price rows"))]
    #[validate(nested)]
    pub prices: Option<Vec<PriceInput>>,
    pub inventory_quantity: Option<i32>,
    /// Quantity snapshot observed by the caller when inventory_quantity was read.
    pub expected_inventory_quantity: Option<i32>,
    /// ProductVariant business revision for catalog changes; inventory-only writes use their own
    /// quantity snapshot.
    pub expected_revision: Option<String>,
    #[validate(length(
        min = 1,
        max = 32,
        message = "Inventory policy must be 1-32 characters"
    ))]
    pub inventory_policy: Option<String>,
    #[serde(default, skip_serializing_if = "Patch::is_keep")]
    #[schema(value_type = Option<String>)]
    pub weight: Patch<Decimal>,
    #[serde(default, skip_serializing_if = "Patch::is_keep")]
    #[schema(value_type = Option<String>)]
    pub weight_unit: Patch<String>,
    // Keep this literal aligned with MAX_VARIANT_AXES in dto/variant_axis.rs.
    #[validate(length(max = 32, message = "A variant can have at most 32 axis assignments"))]
    #[validate(nested)]
    pub axis_values: Option<Vec<VariantAxisValueInput>>,
}

impl UpdateVariantInput {
    /// Whether the caller supplied a Product-owned catalog mutation.
    ///
    /// Inventory quantity is deliberately excluded: it uses its own expected quantity
    /// snapshot and must not advance ProductVariant business or Index revisions.
    pub fn has_catalog_changes(&self) -> bool {
        self.sku.is_changed()
            || self.barcode.is_changed()
            || self.shipping_profile_slug.is_changed()
            || self.prices.is_some()
            || self.inventory_policy.is_some()
            || self.weight.is_changed()
            || self.weight_unit.is_changed()
            || self.axis_values.is_some()
    }

    /// Whether the caller supplied any owner mutation rather than an empty keep-all patch.
    pub fn has_changes(&self) -> bool {
        self.has_catalog_changes() || self.inventory_quantity.is_some()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct VariantResponse {
    pub id: Uuid,
    /// ProductVariant business revision used by optimistic update/delete commands.
    pub revision: String,
    pub product_id: Uuid,
    pub sku: Option<String>,
    pub barcode: Option<String>,
    pub shipping_profile_slug: Option<String>,
    pub title: String,
    #[serde(default)]
    pub translations: Vec<VariantTranslationResponse>,
    pub combination_identity: Option<String>,
    #[serde(default)]
    pub axis_values: Vec<VariantAxisValueResponse>,
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

#[cfg(test)]
mod patch_tests {
    use super::*;
    use super::super::{MAX_VARIANT_AXES, MAX_VARIANT_PRICE_ROWS};

    fn valid_variant_prices(count: usize) -> Vec<PriceInput> {
        (0..count)
            .map(|index| PriceInput {
                currency_code: "USD".to_owned(),
                channel_id: None,
                channel_slug: Some(format!("channel-{index}")),
                amount: Decimal::new(100, 2),
                compare_at_amount: None,
            })
            .collect()
    }

    fn valid_axis_values(count: usize) -> Vec<VariantAxisValueInput> {
        (0..count)
            .map(|index| VariantAxisValueInput {
                attribute_id: Uuid::from_u128(index as u128 + 1),
                option_id: Uuid::from_u128(index as u128 + 10_000),
            })
            .collect()
    }

    #[test]
    fn create_variant_axis_assignment_count_is_bounded() {
        let create_input = |axis_values| CreateVariantInput {
            sku: None,
            barcode: None,
            shipping_profile_slug: None,
            axis_values,
            prices: Vec::new(),
            inventory_quantity: 0,
            inventory_policy: "deny".to_owned(),
            weight: None,
            weight_unit: None,
        };
        assert!(
            create_input(valid_axis_values(MAX_VARIANT_AXES))
                .validate()
                .is_ok()
        );
        assert!(
            create_input(valid_axis_values(MAX_VARIANT_AXES + 1))
                .validate()
                .is_err()
        );
    }

    #[test]
    fn update_variant_axis_assignment_count_is_bounded() {
        let update_input = |axis_values| UpdateVariantInput {
            axis_values: Some(axis_values),
            ..Default::default()
        };
        assert!(
            update_input(valid_axis_values(MAX_VARIANT_AXES))
                .validate()
                .is_ok()
        );
        assert!(
            update_input(valid_axis_values(MAX_VARIANT_AXES + 1))
                .validate()
                .is_err()
        );
    }

    #[test]
    fn variant_price_row_count_is_bounded_for_create_and_update() {
        let create_input = |prices| CreateVariantInput {
            sku: None,
            barcode: None,
            shipping_profile_slug: None,
            axis_values: Vec::new(),
            prices,
            inventory_quantity: 0,
            inventory_policy: "deny".to_owned(),
            weight: None,
            weight_unit: None,
        };
        assert!(
            create_input(valid_variant_prices(MAX_VARIANT_PRICE_ROWS))
                .validate()
                .is_ok()
        );
        assert!(
            create_input(valid_variant_prices(MAX_VARIANT_PRICE_ROWS + 1))
                .validate()
                .is_err()
        );

        let update_input = |prices| UpdateVariantInput {
            prices: Some(prices),
            ..Default::default()
        };
        assert!(
            update_input(valid_variant_prices(MAX_VARIANT_PRICE_ROWS))
                .validate()
                .is_ok()
        );
        assert!(
            update_input(valid_variant_prices(MAX_VARIANT_PRICE_ROWS + 1))
                .validate()
                .is_err()
        );
    }

    #[test]
    fn price_currency_validation_allows_the_owner_to_normalize_surrounding_whitespace() {
        let price = PriceInput {
            currency_code: " usd ".to_owned(),
            channel_id: None,
            channel_slug: None,
            amount: Decimal::new(100, 2),
            compare_at_amount: None,
        };
        assert!(price.validate().is_ok());
    }

    #[test]
    fn missing_nullable_variant_fields_keep_their_values() {
        let input: UpdateVariantInput = serde_json::from_value(serde_json::json!({})).unwrap();
        assert!(input.sku.is_keep());
        assert!(input.barcode.is_keep());
        assert!(input.shipping_profile_slug.is_keep());
        assert!(input.weight.is_keep());
        assert!(input.weight_unit.is_keep());
        let json = serde_json::to_value(input).unwrap();
        for field in [
            "sku",
            "barcode",
            "shipping_profile_slug",
            "weight",
            "weight_unit",
        ] {
            assert!(
                json.get(field).is_none(),
                "Keep must not serialize as null: {field}"
            );
        }
    }

    #[test]
    fn explicit_nulls_clear_every_nullable_variant_field() {
        let json = serde_json::json!({
            "sku": null, "barcode": null, "shipping_profile_slug": null,
            "weight": null, "weight_unit": null,
        });
        let input: UpdateVariantInput = serde_json::from_value(json).unwrap();
        assert_eq!(input.sku, Patch::Clear);
        assert_eq!(input.barcode, Patch::Clear);
        assert_eq!(input.shipping_profile_slug, Patch::Clear);
        assert_eq!(input.weight, Patch::Clear);
        assert_eq!(input.weight_unit, Patch::Clear);
        let json = serde_json::to_value(input).unwrap();
        for field in [
            "sku",
            "barcode",
            "shipping_profile_slug",
            "weight",
            "weight_unit",
        ] {
            assert_eq!(json.get(field), Some(&serde_json::Value::Null));
        }
    }

    #[test]
    fn inventory_only_update_does_not_count_as_a_catalog_mutation() {
        let input = UpdateVariantInput {
            inventory_quantity: Some(12),
            expected_inventory_quantity: Some(9),
            ..Default::default()
        };

        assert!(input.has_changes());
        assert!(!input.has_catalog_changes());
    }

    #[test]
    fn mixed_catalog_and_inventory_update_counts_as_both_owner_mutations() {
        let input = UpdateVariantInput {
            sku: Patch::Set("SKU-UPDATED".to_owned()),
            inventory_quantity: Some(12),
            expected_inventory_quantity: Some(9),
            ..Default::default()
        };

        assert!(input.has_changes());
        assert!(input.has_catalog_changes());
    }

    #[test]
    fn empty_variant_update_is_a_noop_but_empty_prices_are_an_explicit_change() {
        assert!(!UpdateVariantInput::default().has_changes());

        let clear_prices = UpdateVariantInput {
            prices: Some(Vec::new()),
            ..Default::default()
        };
        assert!(clear_prices.has_changes());
    }

    #[test]
    fn inventory_policy_cannot_be_set_to_an_empty_value() {
        let input = UpdateVariantInput {
            inventory_policy: Some(String::new()),
            ..Default::default()
        };
        assert!(input.validate().is_err());
    }

    #[test]
    fn concrete_nullable_values_survive_owner_roundtrip_and_receipt_identity() {
        let input: UpdateVariantInput = serde_json::from_value(serde_json::json!({
            "sku": "SKU", "barcode": "0123", "shipping_profile_slug": "parcel",
            "weight": "0.000000001", "weight_unit": "kg",
        }))
        .unwrap();
        assert_eq!(input.sku, Patch::Set("SKU".into()));
        assert_eq!(input.barcode, Patch::Set("0123".into()));
        assert_eq!(input.shipping_profile_slug, Patch::Set("parcel".into()));
        assert_eq!(
            input.weight,
            Patch::Set(Decimal::from_str_exact("0.000000001").unwrap())
        );
        assert_eq!(input.weight_unit, Patch::Set("kg".into()));
        let encoded = serde_json::to_value(&input).unwrap();
        let decoded: UpdateVariantInput = serde_json::from_value(encoded).unwrap();
        assert_eq!(input.weight, decoded.weight);
        let keep = serde_json::to_value(UpdateVariantInput::default()).unwrap();
        let clear = serde_json::to_value(UpdateVariantInput {
            sku: Patch::Clear,
            ..Default::default()
        })
        .unwrap();
        let set = serde_json::to_value(UpdateVariantInput {
            sku: Patch::Set("SKU".into()),
            ..Default::default()
        })
        .unwrap();
        assert_ne!(keep, clear);
        assert_ne!(keep, set);
        assert_ne!(clear, set);
    }

    #[test]
    fn variant_response_requires_a_revision_for_optimistic_writes() {
        let value = serde_json::json!({
            "id": "00000000-0000-4000-8000-000000000001",
            "product_id": "00000000-0000-4000-8000-000000000002",
            "title": "Variant",
            "prices": [],
            "inventory_quantity": 0,
            "inventory_policy": "deny",
            "in_stock": false,
            "position": 0
        });

        assert!(
            serde_json::from_value::<VariantResponse>(value).is_err(),
            "a variant response without its row revision must be rejected"
        );
    }
}
