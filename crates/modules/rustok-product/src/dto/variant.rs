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
    #[validate(nested)]
    pub axis_values: Vec<VariantAxisValueInput>,
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
    #[validate(nested)]
    pub prices: Option<Vec<PriceInput>>,
    pub inventory_quantity: Option<i32>,
    #[validate(length(max = 32, message = "Inventory policy must be max 32 characters"))]
    pub inventory_policy: Option<String>,
    #[serde(default, skip_serializing_if = "Patch::is_keep")]
    #[schema(value_type = Option<String>)]
    pub weight: Patch<Decimal>,
    #[serde(default, skip_serializing_if = "Patch::is_keep")]
    #[schema(value_type = Option<String>)]
    pub weight_unit: Patch<String>,
    #[validate(nested)]
    pub axis_values: Option<Vec<VariantAxisValueInput>>,
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
}
