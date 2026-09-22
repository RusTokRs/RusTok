use sea_orm::DatabaseConnection;
use serde_json::Value;
use std::collections::BTreeSet;
use uuid::Uuid;

use crate::{
    CommerceResult,
    dto::{CartResponse, CartShippingOptionSummary, ShippingOptionResponse},
};
use rustok_fulfillment::{FulfillmentError, FulfillmentResult, FulfillmentService};

const DEFAULT_SHIPPING_PROFILE_SLUG: &str = "default";
const STOREFRONT_SHIPPING_ENRICHMENT_BOUNDARY: &str = "commerce_storefront_shipping_enrichment";

struct StorefrontShippingDiagnosticError;

impl std::fmt::Debug for StorefrontShippingDiagnosticError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("redacted")
    }
}

fn uuid_shape(value: Uuid) -> &'static str {
    if value.is_nil() { "nil" } else { "non_nil" }
}

fn optional_text_shape(value: Option<&str>) -> &'static str {
    match value {
        None => "absent",
        Some("") => "empty",
        Some(_) => "present",
    }
}

pub fn normalize_shipping_profile_slug(value: &str) -> Option<String> {
    let normalized = value.trim().to_ascii_lowercase();
    if normalized.is_empty() {
        None
    } else {
        Some(normalized)
    }
}

pub fn shipping_profile_slug_from_product_metadata(metadata: &Value) -> String {
    metadata
        .get("shipping_profile")
        .and_then(|profile| profile.get("slug"))
        .and_then(Value::as_str)
        .and_then(normalize_shipping_profile_slug)
        .or_else(|| {
            metadata
                .get("shipping_profile_slug")
                .and_then(Value::as_str)
                .and_then(normalize_shipping_profile_slug)
        })
        .unwrap_or_else(|| DEFAULT_SHIPPING_PROFILE_SLUG.to_string())
}

pub fn product_shipping_profile_slug(
    product_shipping_profile_slug: Option<&str>,
    product_metadata: &Value,
) -> String {
    product_shipping_profile_slug
        .and_then(normalize_shipping_profile_slug)
        .unwrap_or_else(|| shipping_profile_slug_from_product_metadata(product_metadata))
}

pub fn effective_shipping_profile_slug(
    product_default_shipping_profile_slug: Option<&str>,
    product_metadata: &Value,
    variant_shipping_profile_slug: Option<&str>,
) -> String {
    variant_shipping_profile_slug
        .and_then(normalize_shipping_profile_slug)
        .unwrap_or_else(|| {
            product_shipping_profile_slug(product_default_shipping_profile_slug, product_metadata)
        })
}

pub fn is_shipping_option_compatible_with_profiles(
    option: &ShippingOptionResponse,
    required_profiles: &BTreeSet<String>,
) -> bool {
    if required_profiles.is_empty() {
        return true;
    }

    let Some(allowed_profiles) = allowed_shipping_profile_slugs_from_option(option) else {
        return true;
    };

    required_profiles
        .iter()
        .all(|profile_slug| allowed_profiles.contains(profile_slug))
}

fn allowed_shipping_profile_slugs_from_option(
    option: &ShippingOptionResponse,
) -> Option<BTreeSet<String>> {
    option
        .allowed_shipping_profile_slugs
        .as_ref()
        .map(|values| {
            values
                .iter()
                .filter_map(|value| normalize_shipping_profile_slug(value))
                .collect()
        })
        .or_else(|| extract_allowed_shipping_profile_slugs_from_metadata(&option.metadata))
}

pub async fn load_cart_shipping_profile_slugs(
    _db: &DatabaseConnection,
    _tenant_id: Uuid,
    cart: &CartResponse,
) -> CommerceResult<BTreeSet<String>> {
    Ok(cart
        .line_items
        .iter()
        .filter_map(|item| normalize_shipping_profile_slug(item.shipping_profile_slug.as_str()))
        .collect())
}

pub fn map_shipping_option_summary(option: &ShippingOptionResponse) -> CartShippingOptionSummary {
    CartShippingOptionSummary {
        id: option.id,
        name: option.name.clone(),
        currency_code: option.currency_code.clone(),
        amount: option.amount,
        provider_id: option.provider_id.clone(),
        active: option.active,
        metadata: option.metadata.clone(),
    }
}

pub fn enrich_cart_delivery_groups_from_options(
    mut cart: CartResponse,
    mut options: Vec<ShippingOptionResponse>,
    public_channel_slug: Option<&str>,
) -> CartResponse {
    options.retain(|option| {
        option
            .currency_code
            .eq_ignore_ascii_case(&cart.currency_code)
    });
    options.retain(|option| {
        crate::storefront_channel::is_metadata_visible_for_public_channel(
            &option.metadata,
            public_channel_slug,
        )
    });

    for delivery_group in &mut cart.delivery_groups {
        let required_profiles = BTreeSet::from([delivery_group.shipping_profile_slug.clone()]);
        delivery_group.available_shipping_options = options
            .iter()
            .filter(|option| {
                is_shipping_option_compatible_with_profiles(option, &required_profiles)
            })
            .map(map_shipping_option_summary)
            .collect();
        if delivery_group.selected_shipping_option_id.is_none()
            && let Some(selected_id) = cart.selected_shipping_option_id
            && delivery_group
                .available_shipping_options
                .iter()
                .any(|opt| opt.id == selected_id)
        {
            delivery_group.selected_shipping_option_id = Some(selected_id);
        }
    }
    cart.selected_shipping_option_id = if cart.delivery_groups.len() == 1 {
        cart.delivery_groups[0].selected_shipping_option_id
    } else {
        None
    };

    cart
}

pub async fn enrich_cart_delivery_groups_typed(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    cart: CartResponse,
    public_channel_slug: Option<&str>,
    requested_locale: Option<&str>,
    tenant_default_locale: Option<&str>,
) -> FulfillmentResult<CartResponse> {
    let options = FulfillmentService::new(db.clone())
        .list_shipping_options(tenant_id, requested_locale, tenant_default_locale)
        .await?;
    Ok(enrich_cart_delivery_groups_from_options(
        cart,
        options,
        public_channel_slug,
    ))
}

pub async fn enrich_cart_delivery_groups(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    cart: CartResponse,
    public_channel_slug: Option<&str>,
    requested_locale: Option<&str>,
    tenant_default_locale: Option<&str>,
) -> CommerceResult<CartResponse> {
    let cart_id = cart.id;
    enrich_cart_delivery_groups_typed(
        db,
        tenant_id,
        cart,
        public_channel_slug,
        requested_locale,
        tenant_default_locale,
    )
    .await
    .map_err(|error| {
        log_cart_delivery_group_enrichment_error(
            &error,
            tenant_id,
            cart_id,
            public_channel_slug,
            requested_locale,
            tenant_default_locale,
        );
        crate::CommerceError::Validation(
            "Cart shipping details are temporarily unavailable".to_string(),
        )
    })
}

fn log_cart_delivery_group_enrichment_error(
    error: &FulfillmentError,
    tenant_id: Uuid,
    cart_id: Uuid,
    public_channel_slug: Option<&str>,
    requested_locale: Option<&str>,
    tenant_default_locale: Option<&str>,
) {
    let (owner_code, owner_kind, owner_retryable) = match error {
        FulfillmentError::Validation(_) => ("fulfillment.validation", "validation", false),
        FulfillmentError::ShippingOptionNotFound(_) => {
            ("fulfillment.shipping_option_not_found", "not_found", false)
        }
        FulfillmentError::FulfillmentNotFound(_) => {
            ("fulfillment.fulfillment_not_found", "not_found", false)
        }
        FulfillmentError::InvalidTransition { .. } => {
            ("fulfillment.invalid_transition", "conflict", false)
        }
        FulfillmentError::Database(_) => ("fulfillment.database_unavailable", "unavailable", true),
    };
    let technical = matches!(error, FulfillmentError::Database(_));
    let tenant_id_shape = uuid_shape(tenant_id);
    let cart_id_shape = uuid_shape(cart_id);
    let public_channel_slug_shape = optional_text_shape(public_channel_slug);
    let requested_locale_shape = optional_text_shape(requested_locale);
    let tenant_default_locale_shape = optional_text_shape(tenant_default_locale);
    let error = StorefrontShippingDiagnosticError;

    if technical {
        tracing::error!(
            error = ?error,
            owner = "rustok_fulfillment",
            tenant_id_shape,
            cart_id_shape,
            public_channel_slug_shape,
            requested_locale_shape,
            tenant_default_locale_shape,
            operation = "list_shipping_options",
            owner_code,
            owner_kind,
            owner_retryable,
            boundary = STOREFRONT_SHIPPING_ENRICHMENT_BOUNDARY,
            "storefront cart shipping enrichment owner read failed"
        );
    } else {
        tracing::warn!(
            error = ?error,
            owner = "rustok_fulfillment",
            tenant_id_shape,
            cart_id_shape,
            public_channel_slug_shape,
            requested_locale_shape,
            tenant_default_locale_shape,
            operation = "list_shipping_options",
            owner_code,
            owner_kind,
            owner_retryable,
            boundary = STOREFRONT_SHIPPING_ENRICHMENT_BOUNDARY,
            "storefront cart shipping enrichment owner read was rejected"
        );
    }
}

fn extract_allowed_shipping_profile_slugs_from_metadata(
    metadata: &Value,
) -> Option<BTreeSet<String>> {
    metadata
        .get("shipping_profiles")
        .and_then(|profiles| profiles.get("allowed_slugs"))
        .and_then(Value::as_array)
        .map(|values| {
            values
                .iter()
                .filter_map(Value::as_str)
                .filter_map(normalize_shipping_profile_slug)
                .collect()
        })
}

#[cfg(test)]
mod tests {
    use super::{effective_shipping_profile_slug, is_shipping_option_compatible_with_profiles};
    use crate::dto::ShippingOptionResponse;
    use chrono::Utc;
    use rust_decimal::Decimal;
    use std::collections::BTreeSet;
    use uuid::Uuid;

    #[test]
    fn shipping_option_compatibility_uses_typed_allowed_profiles() {
        let option = ShippingOptionResponse {
            id: Uuid::new_v4(),
            tenant_id: Uuid::new_v4(),
            name: "Bulky Freight".to_string(),
            currency_code: "EUR".to_string(),
            amount: Decimal::new(2999, 2),
            provider_id: "manual".to_string(),
            active: true,
            allowed_shipping_profile_slugs: Some(vec![" bulky ".to_string()]),
            metadata: serde_json::json!({}),
            created_at: Utc::now(),
            updated_at: Utc::now(),
            requested_locale: Some("en".to_string()),
            effective_locale: Some("en".to_string()),
            available_locales: vec!["en".to_string()],
            translations: vec![crate::dto::ShippingOptionTranslationResponse {
                locale: "en".to_string(),
                name: "Bulky Freight".to_string(),
            }],
        };
        let required_profiles = BTreeSet::from([String::from("bulky")]);

        assert!(is_shipping_option_compatible_with_profiles(
            &option,
            &required_profiles,
        ));
    }

    #[test]
    fn effective_shipping_profile_prefers_variant_then_product_then_default() {
        let product_metadata = serde_json::json!({
            "shipping_profile": { "slug": "bulky" }
        });

        assert_eq!(
            effective_shipping_profile_slug(Some("cold-chain"), &product_metadata, Some("frozen")),
            "frozen"
        );
        assert_eq!(
            effective_shipping_profile_slug(Some("cold-chain"), &product_metadata, None),
            "cold-chain"
        );
        assert_eq!(
            effective_shipping_profile_slug(None, &product_metadata, None),
            "bulky"
        );
        assert_eq!(
            effective_shipping_profile_slug(None, &serde_json::json!({}), None),
            "default"
        );
    }
}
