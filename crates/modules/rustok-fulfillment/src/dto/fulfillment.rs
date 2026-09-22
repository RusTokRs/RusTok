use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use rustok_api::TenantLocale;
use serde::{Deserialize, Deserializer, Serialize};
use serde_json::Value;
use utoipa::ToSchema;
use uuid::Uuid;
use validator::{Validate, ValidationError};

fn deserialize_tenant_locale<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: Deserializer<'de>,
{
    let raw = String::deserialize(deserializer)?;
    TenantLocale::new(&raw)
        .map(TenantLocale::into_inner)
        .map_err(|error| serde::de::Error::custom(error.to_string()))
}

fn validate_tenant_locale(locale: &str) -> Result<(), ValidationError> {
    match TenantLocale::new(locale) {
        Ok(canonical) if canonical.as_str() == locale => Ok(()),
        _ => Err(ValidationError::new("tenant_locale")),
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Validate, ToSchema)]
pub struct CreateShippingOptionInput {
    #[validate(length(min = 1, message = "At least one translation required"))]
    #[validate(nested)]
    pub translations: Vec<ShippingOptionTranslationInput>,
    #[validate(length(equal = 3))]
    pub currency_code: String,
    pub amount: Decimal,
    #[validate(length(min = 1, max = 100))]
    pub provider_id: Option<String>,
    pub allowed_shipping_profile_slugs: Option<Vec<String>>,
    pub metadata: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, Validate, ToSchema)]
pub struct UpdateShippingOptionInput {
    #[validate(nested)]
    pub translations: Option<Vec<ShippingOptionTranslationInput>>,
    #[validate(length(equal = 3))]
    pub currency_code: Option<String>,
    pub amount: Option<Decimal>,
    #[validate(length(min = 1, max = 100))]
    pub provider_id: Option<String>,
    pub allowed_shipping_profile_slugs: Option<Vec<String>>,
    pub metadata: Option<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Validate, ToSchema)]
pub struct ShippingOptionTranslationInput {
    #[serde(deserialize_with = "deserialize_tenant_locale")]
    #[validate(custom(function = "validate_tenant_locale"))]
    pub locale: String,
    #[validate(length(min = 1, max = 120))]
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ListFulfillmentsInput {
    pub page: u64,
    pub per_page: u64,
    pub status: Option<String>,
    pub order_id: Option<Uuid>,
    pub customer_id: Option<Uuid>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Validate, ToSchema)]
pub struct CreateFulfillmentInput {
    pub order_id: Uuid,
    pub shipping_option_id: Option<Uuid>,
    pub customer_id: Option<Uuid>,
    #[validate(length(max = 100))]
    pub carrier: Option<String>,
    #[validate(length(max = 100))]
    pub tracking_number: Option<String>,
    pub items: Option<Vec<CreateFulfillmentItemInput>>,
    pub metadata: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, Validate, ToSchema)]
pub struct CreateFulfillmentItemInput {
    pub order_line_item_id: Uuid,
    #[validate(range(min = 1))]
    pub quantity: i32,
    pub metadata: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, Validate, ToSchema)]
pub struct FulfillmentItemQuantityInput {
    pub fulfillment_item_id: Uuid,
    #[validate(range(min = 1))]
    pub quantity: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize, Validate, ToSchema)]
pub struct ShipFulfillmentInput {
    #[validate(length(min = 1, max = 100))]
    pub carrier: String,
    #[validate(length(min = 1, max = 100))]
    pub tracking_number: String,
    pub items: Option<Vec<FulfillmentItemQuantityInput>>,
    pub metadata: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct DeliverFulfillmentInput {
    pub delivered_note: Option<String>,
    pub items: Option<Vec<FulfillmentItemQuantityInput>>,
    pub metadata: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ReopenFulfillmentInput {
    pub items: Option<Vec<FulfillmentItemQuantityInput>>,
    pub metadata: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, Validate, ToSchema)]
pub struct ReshipFulfillmentInput {
    #[validate(length(min = 1, max = 100))]
    pub carrier: String,
    #[validate(length(min = 1, max = 100))]
    pub tracking_number: String,
    pub items: Option<Vec<FulfillmentItemQuantityInput>>,
    pub metadata: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct CancelFulfillmentInput {
    pub reason: Option<String>,
    pub metadata: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ShippingOptionResponse {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub name: String,
    pub currency_code: String,
    pub amount: Decimal,
    pub provider_id: String,
    pub active: bool,
    pub allowed_shipping_profile_slugs: Option<Vec<String>>,
    pub metadata: Value,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub requested_locale: Option<String>,
    pub effective_locale: Option<String>,
    pub available_locales: Vec<String>,
    pub translations: Vec<ShippingOptionTranslationResponse>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ShippingOptionTranslationResponse {
    pub locale: String,
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct FulfillmentResponse {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub order_id: Uuid,
    pub shipping_option_id: Option<Uuid>,
    pub customer_id: Option<Uuid>,
    pub status: String,
    pub carrier: Option<String>,
    pub tracking_number: Option<String>,
    pub delivered_note: Option<String>,
    pub cancellation_reason: Option<String>,
    pub items: Vec<FulfillmentItemResponse>,
    pub metadata: Value,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub shipped_at: Option<DateTime<Utc>>,
    pub delivered_at: Option<DateTime<Utc>>,
    pub cancelled_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct FulfillmentItemResponse {
    pub id: Uuid,
    pub fulfillment_id: Uuid,
    pub order_line_item_id: Uuid,
    pub quantity: i32,
    pub shipped_quantity: i32,
    pub delivered_quantity: i32,
    pub metadata: Value,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn shipping_translation_deserialization_canonicalizes_tenant_locale() {
        let input: ShippingOptionTranslationInput = serde_json::from_value(json!({
            "locale": " zh_hant_tw ",
            "name": "Express"
        }))
        .expect("valid tenant locale");

        assert_eq!(input.locale, "zh-Hant-TW");
    }

    #[test]
    fn shipping_translation_deserialization_rejects_storage_only_und() {
        let result = serde_json::from_value::<ShippingOptionTranslationInput>(json!({
            "locale": "und",
            "name": "Express"
        }));

        assert!(result.is_err());
    }

    #[test]
    fn shipping_translation_validation_rejects_direct_und() {
        let input = ShippingOptionTranslationInput {
            locale: "und".to_string(),
            name: "Express".to_string(),
        };

        assert!(input.validate().is_err());
    }

    #[test]
    fn shipping_translation_validation_rejects_noncanonical_direct_locale() {
        let input = ShippingOptionTranslationInput {
            locale: "zh_hant_tw".to_string(),
            name: "Express".to_string(),
        };

        assert!(input.validate().is_err());
    }
}
