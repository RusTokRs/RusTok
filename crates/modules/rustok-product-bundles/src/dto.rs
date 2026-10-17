use chrono::{DateTime, Utc};
use sea_orm::prelude::Decimal;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Deserializes a field whose three states have distinct command semantics:
/// omitted -> `None`, explicit JSON null -> `Some(None)`, and a value -> `Some(Some(value))`.
///
/// Serde's plain `Option<Option<T>>` collapses missing and null while deserializing, so command
/// DTOs use this helper wherever a caller-visible patch must preserve that distinction.
fn deserialize_optional_optional<'de, T, D>(deserializer: D) -> Result<Option<Option<T>>, D::Error>
where
    T: Deserialize<'de>,
    D: serde::Deserializer<'de>,
{
    Option::<T>::deserialize(deserializer).map(Some)
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct BundleTranslationInput {
    pub locale: String,
    pub name: String,
    pub description: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct BundleTranslationDto {
    pub id: Uuid,
    pub locale: String,
    pub name: String,
    pub description: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct BundleItemInput {
    pub product_id: Uuid,
    pub variant_id: Option<Uuid>,
    pub quantity: i32,
    pub is_optional: Option<bool>,
    pub discount_rate: Option<Decimal>,
    pub position: Option<i32>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct BundleItemDto {
    pub id: Uuid,
    pub bundle_id: Uuid,
    pub product_id: Uuid,
    pub variant_id: Option<Uuid>,
    pub quantity: i32,
    pub is_optional: bool,
    pub discount_rate: Option<Decimal>,
    pub position: i32,
    pub created_at: DateTime<Utc>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct CreateBundleInput {
    pub bundle_product_id: Option<Uuid>,
    pub slug: String,
    pub bundle_type: Option<String>,
    pub status: Option<String>,
    pub discount_type: Option<String>,
    pub discount_value: Option<Decimal>,
    pub metadata: Option<serde_json::Value>,
    pub translations: Vec<BundleTranslationInput>,
    pub items: Vec<BundleItemInput>,
}

/// Patch for one localized Bundle translation.
///
/// `None` leaves a field unchanged; `Some(None)` clears an optional description. A new locale
/// must provide `name`, while an existing locale can patch either field independently.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct BundleTranslationUpdateInput {
    pub locale: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(
        default,
        deserialize_with = "deserialize_optional_optional",
        skip_serializing_if = "Option::is_none"
    )]
    pub description: Option<Option<String>>,
}

#[derive(Clone, Debug, Deserialize, Serialize, Default)]
pub struct UpdateBundleInput {
    /// No outer value keeps the owner; `Some(None)` clears it; `Some(Some(id))` assigns it.
    #[serde(
        default,
        deserialize_with = "deserialize_optional_optional",
        skip_serializing_if = "Option::is_none"
    )]
    pub bundle_product_id: Option<Option<Uuid>>,
    pub slug: Option<String>,
    pub bundle_type: Option<String>,
    pub status: Option<String>,
    pub discount_type: Option<String>,
    pub discount_value: Option<Decimal>,
    /// No outer value keeps metadata; `Some(None)` resets it to the default empty object.
    #[serde(
        default,
        deserialize_with = "deserialize_optional_optional",
        skip_serializing_if = "Option::is_none"
    )]
    pub metadata: Option<Option<serde_json::Value>>,
    pub translations: Option<Vec<BundleTranslationUpdateInput>>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct BundleDto {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub bundle_product_id: Option<Uuid>,
    pub slug: String,
    pub name: String,
    pub description: Option<String>,
    pub bundle_type: String,
    pub status: String,
    pub discount_type: String,
    pub discount_value: Decimal,
    pub metadata: serde_json::Value,
    pub translations: Vec<BundleTranslationDto>,
    pub items: Vec<BundleItemDto>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct BundleFilter {
    pub search: Option<String>,
    pub status: Option<String>,
    pub bundle_type: Option<String>,
    /// Restricts the directory to the bundle owned by this Product. Component membership is
    /// intentionally not a directory filter: a Product can be a component of many bundles.
    pub owner_product_id: Option<Uuid>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct BundleListResponse {
    pub items: Vec<BundleDto>,
    pub total: u64,
    pub page: u64,
    pub per_page: u64,
}

#[cfg(test)]
mod tests {
    use super::{BundleTranslationUpdateInput, UpdateBundleInput};
    use uuid::Uuid;

    #[test]
    fn bundle_update_receipt_payload_distinguishes_keep_from_owner_clear() {
        let keep = serde_json::to_value(UpdateBundleInput::default())
            .expect("Bundle update should serialize");
        let clear = serde_json::to_value(UpdateBundleInput {
            bundle_product_id: Some(None),
            ..Default::default()
        })
        .expect("Bundle owner clear should serialize");
        let assign = serde_json::to_value(UpdateBundleInput {
            bundle_product_id: Some(Some(Uuid::from_u128(42))),
            ..Default::default()
        })
        .expect("Bundle owner assignment should serialize");

        assert!(keep.get("bundle_product_id").is_none());
        assert_eq!(
            clear.get("bundle_product_id"),
            Some(&serde_json::Value::Null)
        );
        assert_ne!(clear, assign);

        let deserialized_keep: UpdateBundleInput = serde_json::from_value(serde_json::json!({}))
            .expect("omitted owner should deserialize");
        let deserialized_clear: UpdateBundleInput =
            serde_json::from_value(serde_json::json!({"bundle_product_id": null}))
                .expect("null owner should deserialize");
        assert_eq!(deserialized_keep.bundle_product_id, None);
        assert_eq!(deserialized_clear.bundle_product_id, Some(None));

        let translation_clear: BundleTranslationUpdateInput =
            serde_json::from_value(serde_json::json!({"locale": "en", "description": null}))
                .expect("null translation description should deserialize");
        assert_eq!(translation_clear.description, Some(None));
    }
}
