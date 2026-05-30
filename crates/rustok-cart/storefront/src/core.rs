use serde_json::Value;
use std::fmt::{Display, Formatter};
use uuid::Uuid;

use crate::model::{
    StorefrontCart, StorefrontCartAdjustment, StorefrontCartDeliveryGroup, StorefrontCartLineItem,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CartCoreError {
    Validation(String),
}

impl Display for CartCoreError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Validation(error) => write!(f, "{error}"),
        }
    }
}

impl std::error::Error for CartCoreError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CartLineItemQuantityCommand {
    Remove,
    Update { next_quantity: i32 },
}

pub fn normalize_cart_id(value: Option<String>) -> Option<String> {
    value.and_then(|value| {
        let trimmed = value.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_string())
        }
    })
}

pub fn parse_cart_id(value: Option<String>) -> Result<Option<(String, Uuid)>, CartCoreError> {
    match normalize_cart_id(value) {
        Some(cart_id) => {
            let parsed = Uuid::parse_str(cart_id.as_str()).map_err(|_| {
                CartCoreError::Validation("cart_id must be a valid UUID".to_string())
            })?;
            Ok(Some((cart_id, parsed)))
        }
        None => Ok(None),
    }
}

pub fn parse_line_item_id(value: String) -> Result<(String, Uuid), CartCoreError> {
    let normalized = value.trim().to_string();
    if normalized.is_empty() {
        return Err(CartCoreError::Validation(
            "line_item_id must not be empty".to_string(),
        ));
    }

    let parsed = Uuid::parse_str(normalized.as_str())
        .map_err(|_| CartCoreError::Validation("line_item_id must be a valid UUID".to_string()))?;
    Ok((normalized, parsed))
}

pub fn parse_adjustment_scope(metadata: &str) -> Option<String> {
    serde_json::from_str::<Value>(metadata)
        .ok()
        .and_then(|value| {
            value
                .get("scope")
                .and_then(Value::as_str)
                .map(str::to_string)
        })
}

#[allow(dead_code)]
pub fn normalize_public_channel_slug(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| value.to_ascii_lowercase())
}

pub fn decrement_quantity_command(current_quantity: i32) -> CartLineItemQuantityCommand {
    if current_quantity <= 1 {
        CartLineItemQuantityCommand::Remove
    } else {
        CartLineItemQuantityCommand::Update {
            next_quantity: current_quantity - 1,
        }
    }
}

pub fn error_with_context(context: &str, error: &str) -> String {
    format!("{}: {}", context, error)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CartDisplayFallbacks {
    pub empty: String,
    pub guest: String,
}

impl CartDisplayFallbacks {
    pub fn new(empty: String, guest: String) -> Self {
        Self { empty, guest }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CartSummaryViewModel {
    pub id: String,
    pub status: String,
    pub subtotal: String,
    pub adjustments: String,
    pub shipping: String,
    pub total: String,
    pub email: String,
    pub channel: String,
    pub customer: String,
    pub region: String,
    pub country: String,
    pub locale: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CartAdjustmentViewModel {
    pub source_type: String,
    pub source: String,
    pub scope: String,
    pub line_item: String,
    pub amount: String,
    pub metadata: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CartDeliveryGroupViewModel {
    pub shipping_profile_slug: String,
    pub seller_identity: Option<String>,
    pub line_item_count: String,
    pub selected_shipping_option: String,
    pub available_option_count: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CartLineItemViewModel {
    pub id: String,
    pub title: String,
    pub sku: String,
    pub quantity: i32,
    pub quantity_label: String,
    pub unit_price: String,
    pub total_price: String,
    pub shipping_profile_slug: String,
    pub seller_identity: String,
}

fn optional_display(value: Option<String>, fallback: &str) -> String {
    value
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| fallback.to_string())
}

fn optional_identity(primary: Option<String>, secondary: Option<String>) -> Option<String> {
    primary
        .or(secondary)
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

pub fn money_value(currency_code: &str, amount: &str) -> String {
    format!("{} {}", currency_code, amount)
}

pub fn cart_summary_view_model(
    cart: &StorefrontCart,
    fallbacks: &CartDisplayFallbacks,
) -> CartSummaryViewModel {
    CartSummaryViewModel {
        id: cart.id.clone(),
        status: cart.status.clone(),
        subtotal: money_value(&cart.currency_code, &cart.subtotal_amount),
        adjustments: money_value(&cart.currency_code, &cart.adjustment_total),
        shipping: money_value(&cart.currency_code, &cart.shipping_total),
        total: money_value(&cart.currency_code, &cart.total_amount),
        email: optional_display(cart.email.clone(), &fallbacks.empty),
        channel: optional_display(cart.channel_slug.clone(), &fallbacks.empty),
        customer: optional_display(cart.customer_id.clone(), &fallbacks.guest),
        region: optional_display(cart.region_id.clone(), &fallbacks.empty),
        country: optional_display(cart.country_code.clone(), &fallbacks.empty),
        locale: optional_display(cart.locale_code.clone(), &fallbacks.empty),
    }
}

pub fn cart_adjustment_view_model(
    adjustment: StorefrontCartAdjustment,
    fallbacks: &CartDisplayFallbacks,
) -> CartAdjustmentViewModel {
    CartAdjustmentViewModel {
        source_type: adjustment.source_type,
        source: optional_display(adjustment.source_id, &fallbacks.empty),
        scope: optional_display(adjustment.scope, &fallbacks.empty),
        line_item: optional_display(adjustment.line_item_id, &fallbacks.empty),
        amount: money_value(&adjustment.currency_code, &adjustment.amount),
        metadata: adjustment.metadata,
    }
}

pub fn cart_delivery_group_view_model(
    group: StorefrontCartDeliveryGroup,
    fallbacks: &CartDisplayFallbacks,
) -> CartDeliveryGroupViewModel {
    CartDeliveryGroupViewModel {
        shipping_profile_slug: group.shipping_profile_slug,
        seller_identity: optional_identity(group.seller_id, group.seller_scope),
        line_item_count: group.line_item_count.to_string(),
        selected_shipping_option: optional_display(
            group.selected_shipping_option_id,
            &fallbacks.empty,
        ),
        available_option_count: group.available_option_count.to_string(),
    }
}

pub fn cart_line_item_view_model(
    item: StorefrontCartLineItem,
    fallbacks: &CartDisplayFallbacks,
) -> CartLineItemViewModel {
    CartLineItemViewModel {
        id: item.id,
        title: item.title,
        sku: optional_display(item.sku, &fallbacks.empty),
        quantity: item.quantity,
        quantity_label: item.quantity.to_string(),
        unit_price: money_value(&item.currency_code, &item.unit_price),
        total_price: money_value(&item.currency_code, &item.total_price),
        shipping_profile_slug: item.shipping_profile_slug,
        seller_identity: optional_identity(item.seller_id, item.seller_scope)
            .unwrap_or_else(|| fallbacks.empty.clone()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_cart_id_trims_and_drops_empty_values() {
        assert_eq!(normalize_cart_id(None), None);
        assert_eq!(normalize_cart_id(Some("  ".to_string())), None);
        assert_eq!(
            normalize_cart_id(Some("  550e8400-e29b-41d4-a716-446655440000  ".to_string())),
            Some("550e8400-e29b-41d4-a716-446655440000".to_string())
        );
    }

    #[test]
    fn parse_cart_id_validates_uuid_after_normalization() {
        assert!(parse_cart_id(Some("not-a-uuid".to_string())).is_err());
        let parsed = parse_cart_id(Some(" 550e8400-e29b-41d4-a716-446655440000 ".to_string()))
            .expect("cart id should parse")
            .expect("cart id should be present");

        assert_eq!(parsed.0, "550e8400-e29b-41d4-a716-446655440000");
    }

    #[test]
    fn parse_line_item_id_rejects_empty_and_invalid_values() {
        assert!(parse_line_item_id("  ".to_string()).is_err());
        assert!(parse_line_item_id("abc".to_string()).is_err());
    }

    #[test]
    fn parse_adjustment_scope_reads_language_neutral_metadata() {
        assert_eq!(
            parse_adjustment_scope(r#"{"scope":"line_item","label":"ignored"}"#),
            Some("line_item".to_string())
        );
        assert_eq!(parse_adjustment_scope("not json"), None);
    }

    #[test]
    fn decrement_quantity_command_keeps_write_policy_out_of_ui() {
        assert_eq!(
            decrement_quantity_command(0),
            CartLineItemQuantityCommand::Remove
        );
        assert_eq!(
            decrement_quantity_command(1),
            CartLineItemQuantityCommand::Remove
        );
        assert_eq!(
            decrement_quantity_command(3),
            CartLineItemQuantityCommand::Update { next_quantity: 2 }
        );
    }

    #[test]
    fn normalize_public_channel_slug_trims_and_lowercases() {
        assert_eq!(normalize_public_channel_slug(None), None);
        assert_eq!(normalize_public_channel_slug(Some("  ")), None);
        assert_eq!(
            normalize_public_channel_slug(Some("  Web-Store  ")),
            Some("web-store".to_string())
        );
    }

    #[test]
    fn cart_view_models_apply_display_fallbacks_and_money_labels() {
        let fallbacks = CartDisplayFallbacks::new("not set".to_string(), "guest".to_string());
        let cart = StorefrontCart {
            id: "cart-1".to_string(),
            status: "active".to_string(),
            currency_code: "USD".to_string(),
            subtotal_amount: "10".to_string(),
            adjustment_total: "1".to_string(),
            shipping_total: "2".to_string(),
            total_amount: "11".to_string(),
            channel_slug: None,
            email: Some(" shopper@example.test ".to_string()),
            customer_id: None,
            region_id: Some("".to_string()),
            country_code: Some("US".to_string()),
            locale_code: None,
            line_items: Vec::new(),
            adjustments: Vec::new(),
            delivery_groups: Vec::new(),
        };

        let view_model = cart_summary_view_model(&cart, &fallbacks);

        assert_eq!(view_model.subtotal, "USD 10");
        assert_eq!(view_model.email, "shopper@example.test");
        assert_eq!(view_model.channel, "not set");
        assert_eq!(view_model.customer, "guest");
        assert_eq!(view_model.region, "not set");
        assert_eq!(view_model.country, "US");
    }

    #[test]
    fn line_item_view_model_prefers_seller_id_then_scope() {
        let fallbacks = CartDisplayFallbacks::new("not set".to_string(), "guest".to_string());
        let item = StorefrontCartLineItem {
            id: "line-1".to_string(),
            title: "Item".to_string(),
            sku: None,
            quantity: 2,
            unit_price: "5".to_string(),
            total_price: "10".to_string(),
            currency_code: "USD".to_string(),
            shipping_profile_slug: "default".to_string(),
            seller_id: None,
            seller_scope: Some(" marketplace ".to_string()),
        };

        let view_model = cart_line_item_view_model(item, &fallbacks);

        assert_eq!(view_model.sku, "not set");
        assert_eq!(view_model.seller_identity, "marketplace");
        assert_eq!(view_model.quantity_label, "2");
        assert_eq!(view_model.unit_price, "USD 5");
        assert_eq!(view_model.total_price, "USD 10");
    }

    #[test]
    fn adjustment_and_delivery_group_view_models_are_language_neutral() {
        let fallbacks = CartDisplayFallbacks::new("not set".to_string(), "guest".to_string());
        let adjustment = StorefrontCartAdjustment {
            id: "adj-1".to_string(),
            line_item_id: None,
            source_type: "promotion".to_string(),
            source_id: Some("promo-1".to_string()),
            scope: None,
            amount: "3".to_string(),
            currency_code: "USD".to_string(),
            metadata: r#"{"scope":"cart"}"#.to_string(),
        };
        let group = StorefrontCartDeliveryGroup {
            shipping_profile_slug: "default".to_string(),
            seller_id: Some("seller-1".to_string()),
            seller_scope: Some("scope".to_string()),
            line_item_count: 3,
            selected_shipping_option_id: None,
            available_option_count: 2,
        };

        let adjustment_view_model = cart_adjustment_view_model(adjustment, &fallbacks);
        let group_view_model = cart_delivery_group_view_model(group, &fallbacks);

        assert_eq!(adjustment_view_model.source_type, "promotion");
        assert_eq!(adjustment_view_model.source, "promo-1");
        assert_eq!(adjustment_view_model.scope, "not set");
        assert_eq!(adjustment_view_model.amount, "USD 3");
        assert_eq!(
            group_view_model.seller_identity,
            Some("seller-1".to_string())
        );
        assert_eq!(group_view_model.selected_shipping_option, "not set");
        assert_eq!(group_view_model.available_option_count, "2");
    }
}
