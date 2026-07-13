use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use utoipa::ToSchema;
use uuid::Uuid;
use validator::{Validate, ValidationError};

#[derive(Debug, Clone, Serialize, Deserialize, Validate, ToSchema)]
pub struct CreateCartInput {
    pub customer_id: Option<Uuid>,
    #[validate(length(max = 255))]
    #[validate(email)]
    pub email: Option<String>,
    pub region_id: Option<Uuid>,
    #[validate(custom(function = "validate_country_code"))]
    pub country_code: Option<String>,
    #[validate(custom(function = "validate_locale_code"))]
    pub locale_code: Option<String>,
    pub selected_shipping_option_id: Option<Uuid>,
    #[validate(custom(function = "validate_currency_code"))]
    pub currency_code: String,
    pub metadata: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, Validate, ToSchema)]
pub struct AddCartLineItemInput {
    pub product_id: Option<Uuid>,
    pub variant_id: Option<Uuid>,
    #[validate(length(min = 1, max = 100))]
    pub shipping_profile_slug: Option<String>,
    #[validate(length(max = 100))]
    pub sku: Option<String>,
    #[validate(length(min = 1, max = 255))]
    pub title: String,
    #[validate(range(min = 1))]
    pub quantity: i32,
    #[validate(custom(function = "validate_non_negative_decimal"))]
    pub unit_price: Decimal,
    pub metadata: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, Validate, ToSchema)]
pub struct UpdateCartContextInput {
    #[validate(length(max = 255))]
    #[validate(email)]
    pub email: Option<String>,
    pub region_id: Option<Uuid>,
    #[validate(custom(function = "validate_country_code"))]
    pub country_code: Option<String>,
    #[validate(custom(function = "validate_locale_code"))]
    pub locale_code: Option<String>,
    pub selected_shipping_option_id: Option<Uuid>,
    #[validate(nested)]
    pub shipping_selections: Option<Vec<CartShippingSelectionInput>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Validate, ToSchema)]
pub struct SetCartAdjustmentInput {
    pub line_item_id: Option<Uuid>,
    #[validate(length(min = 1, max = 64))]
    pub source_type: String,
    #[validate(length(max = 191))]
    pub source_id: Option<String>,
    #[validate(custom(function = "validate_positive_decimal"))]
    pub amount: Decimal,
    pub metadata: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct CartResponse {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub channel_id: Option<Uuid>,
    pub channel_slug: Option<String>,
    pub customer_id: Option<Uuid>,
    pub email: Option<String>,
    pub region_id: Option<Uuid>,
    pub country_code: Option<String>,
    pub locale_code: Option<String>,
    pub selected_shipping_option_id: Option<Uuid>,
    pub status: String,
    pub currency_code: String,
    pub subtotal_amount: Decimal,
    pub adjustment_total: Decimal,
    pub shipping_total: Decimal,
    pub total_amount: Decimal,
    pub tax_total: Decimal,
    pub metadata: Value,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
    pub line_items: Vec<CartLineItemResponse>,
    pub adjustments: Vec<CartAdjustmentResponse>,
    pub tax_lines: Vec<CartTaxLineResponse>,
    pub delivery_groups: Vec<CartDeliveryGroupResponse>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct CartLineItemResponse {
    pub id: Uuid,
    pub cart_id: Uuid,
    pub product_id: Option<Uuid>,
    pub variant_id: Option<Uuid>,
    pub shipping_profile_slug: String,
    pub seller_id: Option<String>,
    pub seller_scope: Option<String>,
    pub sku: Option<String>,
    pub title: String,
    pub quantity: i32,
    pub unit_price: Decimal,
    pub total_price: Decimal,
    pub currency_code: String,
    pub metadata: Value,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct CartAdjustmentResponse {
    pub id: Uuid,
    pub cart_id: Uuid,
    pub line_item_id: Option<Uuid>,
    pub source_type: String,
    pub source_id: Option<String>,
    pub amount: Decimal,
    pub currency_code: String,
    pub metadata: Value,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct CartTaxLineResponse {
    pub id: Uuid,
    pub cart_id: Uuid,
    pub line_item_id: Option<Uuid>,
    pub shipping_option_id: Option<Uuid>,
    pub description: Option<String>,
    pub provider_id: String,
    pub rate: Decimal,
    pub amount: Decimal,
    pub currency_code: String,
    pub metadata: Value,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Validate, ToSchema)]
pub struct CartShippingSelectionInput {
    #[validate(length(min = 1, max = 100))]
    pub shipping_profile_slug: String,
    #[validate(length(max = 100))]
    pub seller_id: Option<String>,
    #[validate(length(min = 1, max = 100))]
    pub seller_scope: Option<String>,
    pub selected_shipping_option_id: Option<Uuid>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct CartShippingOptionSummary {
    pub id: Uuid,
    pub name: String,
    pub currency_code: String,
    pub amount: Decimal,
    pub provider_id: String,
    pub active: bool,
    pub metadata: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct CartDeliveryGroupResponse {
    pub shipping_profile_slug: String,
    pub seller_id: Option<String>,
    pub seller_scope: Option<String>,
    pub line_item_ids: Vec<Uuid>,
    pub selected_shipping_option_id: Option<Uuid>,
    pub available_shipping_options: Vec<CartShippingOptionSummary>,
}

fn validate_currency_code(value: &str) -> Result<(), ValidationError> {
    let value = value.trim();
    if value.len() == 3 && value.chars().all(|ch| ch.is_ascii_alphabetic()) {
        Ok(())
    } else {
        Err(ValidationError::new("currency_code"))
    }
}

fn validate_country_code(value: &str) -> Result<(), ValidationError> {
    let value = value.trim();
    if value.len() == 2 && value.chars().all(|ch| ch.is_ascii_alphabetic()) {
        Ok(())
    } else {
        Err(ValidationError::new("country_code"))
    }
}

fn validate_locale_code(value: &str) -> Result<(), ValidationError> {
    let normalized = value.trim().replace('_', "-");
    let mut segments = normalized.split('-');
    let Some(language) = segments.next() else {
        return Err(ValidationError::new("locale_code"));
    };
    if !(2..=3).contains(&language.len())
        || !language.chars().all(|ch| ch.is_ascii_alphabetic())
    {
        return Err(ValidationError::new("locale_code"));
    }
    if segments.any(|segment| {
        segment.is_empty()
            || segment.len() > 8
            || !segment.chars().all(|ch| ch.is_ascii_alphanumeric())
    }) {
        return Err(ValidationError::new("locale_code"));
    }
    if normalized.len() > 35 {
        return Err(ValidationError::new("locale_code"));
    }
    Ok(())
}

fn validate_non_negative_decimal(value: &Decimal) -> Result<(), ValidationError> {
    if *value >= Decimal::ZERO {
        Ok(())
    } else {
        Err(ValidationError::new("non_negative_decimal"))
    }
}

fn validate_positive_decimal(value: &Decimal) -> Result<(), ValidationError> {
    if *value > Decimal::ZERO {
        Ok(())
    } else {
        Err(ValidationError::new("positive_decimal"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_symbolic_currency_and_country_codes() {
        let input = CreateCartInput {
            customer_id: None,
            email: None,
            region_id: None,
            country_code: Some("1$".to_string()),
            locale_code: Some("en-US".to_string()),
            selected_shipping_option_id: None,
            currency_code: "12$".to_string(),
            metadata: Value::Null,
        };
        assert!(input.validate().is_err());
    }

    #[test]
    fn rejects_invalid_nested_shipping_selection() {
        let input = UpdateCartContextInput {
            email: None,
            region_id: None,
            country_code: None,
            locale_code: None,
            selected_shipping_option_id: None,
            shipping_selections: Some(vec![CartShippingSelectionInput {
                shipping_profile_slug: String::new(),
                seller_id: None,
                seller_scope: None,
                selected_shipping_option_id: None,
            }]),
        };
        assert!(input.validate().is_err());
    }

    #[test]
    fn accepts_normalized_locale_shapes() {
        assert!(validate_locale_code("en-US").is_ok());
        assert!(validate_locale_code("pt_BR").is_ok());
        assert!(validate_locale_code("$$").is_err());
    }
}
