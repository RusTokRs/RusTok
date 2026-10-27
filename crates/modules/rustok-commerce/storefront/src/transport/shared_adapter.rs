use rustok_ui_core::normalize_optional_ui_text;
use serde::{Deserialize, Serialize};
use std::fmt::{Display, Formatter};
#[cfg(feature = "ssr")]
use uuid::Uuid;

use crate::model::{
    StorefrontCheckoutAdjustment, StorefrontCheckoutCart, StorefrontCheckoutDeliveryGroup,
    StorefrontCheckoutShippingOption, StorefrontCheckoutWorkspace, StorefrontCommerceData,
};

const STOREFRONT_COMMERCE_TRANSPORT_BOUNDARY: &str = "commerce_storefront_aggregate_transport";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ApiError {
    Graphql(String),
    ServerFn(String),
    Validation(String),
}

impl Display for ApiError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Graphql(error) => write!(f, "{error}"),
            Self::ServerFn(error) => write!(f, "{error}"),
            Self::Validation(error) => write!(f, "{error}"),
        }
    }
}

impl std::error::Error for ApiError {}

fn configured_tenant_slug() -> Option<String> {
    [
        "RUSTOK_TENANT_SLUG",
        "NEXT_PUBLIC_TENANT_SLUG",
        "NEXT_PUBLIC_DEFAULT_TENANT_SLUG",
    ]
    .into_iter()
    .find_map(|key| {
        std::env::var(key).ok().and_then(|value| {
            let trimmed = value.trim().to_string();
            if trimmed.is_empty() {
                None
            } else {
                Some(trimmed)
            }
        })
    })
}

#[cfg(feature = "ssr")]
pub(super) fn resolve_requested_locale(
    requested: Option<String>,
    request_context_locale: Option<&str>,
    tenant_default_locale: &str,
) -> String {
    normalize_optional_ui_text(requested)
        .or_else(|| {
            request_context_locale
                .and_then(|value| normalize_optional_ui_text(Some(value.to_string())))
        })
        .or_else(|| normalize_optional_ui_text(Some(tenant_default_locale.to_string())))
        .unwrap_or_default()
}

pub(super) fn normalize_cart_id(value: Option<String>) -> Option<String> {
    normalize_optional_ui_text(value)
}

#[cfg(feature = "ssr")]
pub(super) fn parse_cart_id(value: Option<String>) -> Result<Option<(String, Uuid)>, ApiError> {
    match normalize_cart_id(value) {
        Some(cart_id) => {
            let parsed = Uuid::parse_str(cart_id.as_str())
                .map_err(|_| ApiError::Validation("cart_id must be a valid UUID".to_string()))?;
            Ok(Some((cart_id, parsed)))
        }
        None => Ok(None),
    }
}

fn fallback_storefront_commerce(
    selected_cart_id: Option<String>,
    locale: Option<String>,
) -> StorefrontCommerceData {
    let effective_locale = normalize_optional_ui_text(locale).unwrap_or_default();
    let normalized_cart_id = normalize_cart_id(selected_cart_id);

    StorefrontCommerceData {
        effective_locale: effective_locale.clone(),
        tenant_slug: configured_tenant_slug(),
        tenant_default_locale: effective_locale,
        channel_slug: None,
        channel_resolution_source: None,
        selected_cart_id: normalized_cart_id.clone(),
        checkout: normalized_cart_id.map(|_| StorefrontCheckoutWorkspace {
            cart: None,
            payment_collection: None,
        }),
    }
}

fn is_cart_id_validation_error(error: &rustok_ui_transport::UiTransportError) -> bool {
    [
        error.native_error.as_deref(),
        error.graphql_error.as_deref(),
    ]
    .into_iter()
    .flatten()
    .any(|message| {
        message.contains("cart_id must be a valid UUID")
            || message.contains("Invalid cart selection")
    })
}

fn map_cart_transport_error(
    error: rustok_cart_storefront::transport::CartTransportError,
) -> ApiError {
    let failed_path = error.failed_path;
    if is_cart_id_validation_error(&error) {
        tracing::warn!(
            owner = "rustok_cart.storefront",
            owner_operation = "fetch_cart",
            failed_path = failed_path.as_str(),
            fallback_attempted = error.fallback_attempted,
            code = "commerce.storefront_cart_id_invalid",
            boundary = STOREFRONT_COMMERCE_TRANSPORT_BOUNDARY,
            "storefront commerce cart transport validation failed"
        );
        return ApiError::Validation("cart_id must be a valid UUID".to_string());
    }

    tracing::error!(
        owner = "rustok_cart.storefront",
        owner_operation = "fetch_cart",
        failed_path = failed_path.as_str(),
        fallback_attempted = error.fallback_attempted,
        code = "commerce.storefront_cart_unavailable",
        boundary = STOREFRONT_COMMERCE_TRANSPORT_BOUNDARY,
        "storefront commerce cart transport failed"
    );
    ApiError::ServerFn("Storefront cart data is temporarily unavailable".to_string())
}

fn map_payment_transport_error(error: rustok_ui_transport::UiTransportError) -> ApiError {
    let failed_path = error.failed_path;
    tracing::error!(
        owner = "rustok_payment.storefront",
        owner_operation = "fetch_payment_collection",
        failed_path = failed_path.as_str(),
        fallback_attempted = error.fallback_attempted,
        code = "commerce.storefront_payment_collection_unavailable",
        boundary = STOREFRONT_COMMERCE_TRANSPORT_BOUNDARY,
        "storefront commerce payment transport failed"
    );
    match failed_path {
        rustok_ui_transport::UiTransportPath::NativeServer => ApiError::ServerFn(
            "Storefront payment collection is temporarily unavailable".to_string(),
        ),
        rustok_ui_transport::UiTransportPath::Graphql => ApiError::Graphql(
            "Storefront payment collection is temporarily unavailable".to_string(),
        ),
    }
}

fn map_cart_shipping_option(
    value: rustok_cart_storefront::model::StorefrontCartShippingOption,
) -> StorefrontCheckoutShippingOption {
    StorefrontCheckoutShippingOption {
        id: value.id,
        name: value.name,
        currency_code: value.currency_code,
        amount: value.amount,
        provider_id: value.provider_id,
        active: value.active,
    }
}

fn map_cart_delivery_group(
    value: rustok_cart_storefront::model::StorefrontCartDeliveryGroup,
) -> StorefrontCheckoutDeliveryGroup {
    StorefrontCheckoutDeliveryGroup {
        shipping_profile_slug: value.shipping_profile_slug,
        seller_id: value.seller_id,
        line_item_count: value.line_item_count,
        selected_shipping_option_id: value.selected_shipping_option_id,
        available_shipping_options: value
            .available_shipping_options
            .into_iter()
            .map(map_cart_shipping_option)
            .collect(),
    }
}

pub(super) fn map_cart_checkout_cart(
    value: rustok_cart_storefront::model::StorefrontCart,
) -> StorefrontCheckoutCart {
    let adjustments = value
        .adjustments
        .into_iter()
        .map(|adjustment| StorefrontCheckoutAdjustment {
            id: adjustment.id,
            line_item_id: adjustment.line_item_id,
            source_type: adjustment.source_type,
            source_id: adjustment.source_id,
            scope: adjustment.scope,
            amount: adjustment.amount,
            currency_code: adjustment.currency_code,
            metadata: adjustment.metadata,
        })
        .collect::<Vec<_>>();
    let delivery_groups = value
        .delivery_groups
        .into_iter()
        .map(map_cart_delivery_group)
        .collect::<Vec<_>>();
    let delivery_group_count = delivery_groups.len() as u64;

    StorefrontCheckoutCart {
        id: value.id,
        status: value.status,
        currency_code: value.currency_code,
        subtotal_amount: value.subtotal_amount,
        adjustment_total: value.adjustment_total,
        shipping_total: value.shipping_total,
        total_amount: value.total_amount,
        channel_slug: value.channel_slug,
        email: value.email,
        customer_id: value.customer_id,
        region_id: value.region_id,
        country_code: value.country_code,
        locale_code: value.locale_code,
        selected_shipping_option_id: delivery_groups
            .iter()
            .find_map(|group| group.selected_shipping_option_id.clone()),
        line_item_count: value.line_items.len() as u64,
        adjustment_count: adjustments.len() as u64,
        delivery_group_count,
        adjustments,
        delivery_groups,
    }
}

pub async fn fetch_storefront_commerce_graphql(
    selected_cart_id: Option<String>,
    locale: Option<String>,
) -> Result<StorefrontCommerceData, ApiError> {
    let mut data = fallback_storefront_commerce(selected_cart_id.clone(), locale);
    if normalize_cart_id(selected_cart_id.clone()).is_none() {
        return Ok(data);
    }
    let cart_data = rustok_cart_storefront::transport::fetch_cart(
        rustok_cart_storefront::core::build_cart_fetch_request(
            selected_cart_id,
            Some(data.effective_locale.clone()),
        ),
    )
    .await
    .map_err(map_cart_transport_error)?;
    let payment_collection = if let Some(ref cart) = cart_data.cart {
        rustok_payment_storefront::transport::fetch_payment_collection(
            rustok_payment_storefront::transport::build_payment_collection_fetch_request(
                cart.id.clone(),
            ),
        )
        .await
        .map_err(map_payment_transport_error)?
    } else {
        None
    };

    data.selected_cart_id = cart_data.selected_cart_id;
    data.checkout = Some(StorefrontCheckoutWorkspace {
        cart: cart_data.cart.map(map_cart_checkout_cart),
        payment_collection,
    });
    Ok(data)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rustok_ui_transport::{UiTransportError, UiTransportPath};

    #[test]
    fn api_error_displays_correctly() {
        assert_eq!(
            ApiError::Graphql("graphql failure".into()).to_string(),
            "graphql failure"
        );
        assert_eq!(
            ApiError::ServerFn("server fn failure".into()).to_string(),
            "server fn failure"
        );
        assert_eq!(
            ApiError::Validation("validation failure".into()).to_string(),
            "validation failure"
        );
    }

    #[test]
    fn normalize_cart_id_trims_and_discards_blank() {
        assert_eq!(normalize_cart_id(Some("  ".into())), None);
        assert_eq!(normalize_cart_id(Some(" cart-1 ".into())), Some("cart-1".into()));
        assert_eq!(normalize_cart_id(None), None);
    }

    #[test]
    fn fallback_storefront_commerce_constructs_valid_default() {
        let data = fallback_storefront_commerce(Some(" cart-1 ".into()), Some(" en ".into()));
        assert_eq!(data.effective_locale, "en");
        assert_eq!(data.selected_cart_id.as_deref(), Some("cart-1"));
        assert!(data.checkout.is_some());
    }

    #[test]
    fn fallback_storefront_commerce_without_cart_has_no_checkout_workspace() {
        let data = fallback_storefront_commerce(None, None);
        assert_eq!(data.effective_locale, "");
        assert_eq!(data.selected_cart_id, None);
        assert!(data.checkout.is_none());
    }

    #[test]
    fn is_cart_id_validation_error_detects_all_patterns() {
        let err1 = UiTransportError {
            surface: "commerce".into(),
            failed_path: UiTransportPath::Graphql,
            native_error: None,
            graphql_error: Some("cart_id must be a valid UUID".into()),
            fallback_attempted: false,
        };
        assert!(is_cart_id_validation_error(&err1));

        let err2 = UiTransportError {
            surface: "commerce".into(),
            failed_path: UiTransportPath::NativeServer,
            native_error: Some("Invalid cart selection".into()),
            graphql_error: None,
            fallback_attempted: false,
        };
        assert!(is_cart_id_validation_error(&err2));

        let err3 = UiTransportError {
            surface: "commerce".into(),
            failed_path: UiTransportPath::Graphql,
            native_error: None,
            graphql_error: Some("network timeout".into()),
            fallback_attempted: false,
        };
        assert!(!is_cart_id_validation_error(&err3));
    }

    #[test]
    fn map_cart_checkout_cart_maps_all_fields() {
        let cart = rustok_cart_storefront::model::StorefrontCart {
            id: "cart-1".into(),
            status: "active".into(),
            currency_code: "USD".into(),
            subtotal_amount: "100.00".into(),
            adjustment_total: "0.00".into(),
            shipping_total: "10.00".into(),
            total_amount: "110.00".into(),
            channel_slug: Some("web".into()),
            email: Some("user@example.com".into()),
            customer_id: None,
            region_id: None,
            country_code: Some("US".into()),
            locale_code: Some("en".into()),
            delivery_groups: vec![rustok_cart_storefront::model::StorefrontCartDeliveryGroup {
                shipping_profile_slug: "standard".into(),
                seller_id: None,
                line_item_count: 1,
                selected_shipping_option_id: Some("opt-1".into()),
                available_shipping_options: vec![
                    rustok_cart_storefront::model::StorefrontCartShippingOption {
                        id: "opt-1".into(),
                        name: "Standard Ground".into(),
                        currency_code: "USD".into(),
                        amount: "10.00".into(),
                        provider_id: "prov-1".into(),
                        active: true,
                    },
                ],
                available_option_count: 1,
            }],
            line_items: vec![],
            adjustments: vec![],
        };

        let mapped = map_cart_checkout_cart(cart);
        assert_eq!(mapped.id, "cart-1");
        assert_eq!(mapped.selected_shipping_option_id.as_deref(), Some("opt-1"));
        assert_eq!(mapped.delivery_group_count, 1);
        assert_eq!(mapped.delivery_groups[0].shipping_profile_slug, "standard");
        assert_eq!(mapped.delivery_groups[0].available_shipping_options.len(), 1);
    }

    #[cfg(feature = "ssr")]
    #[test]
    fn ssr_cart_id_and_locale_helpers_work_as_expected() {
        assert_eq!(
            resolve_requested_locale(Some(" ru ".into()), Some("en"), "es"),
            "ru"
        );
        assert_eq!(
            resolve_requested_locale(None, Some("en"), "es"),
            "en"
        );
        assert_eq!(
            resolve_requested_locale(None, None, "es"),
            "es"
        );

        let valid_uuid = "a1a2a3a4-b1b2-c1c2-d1d2-d3d4d5d6d7d8";
        let parsed = parse_cart_id(Some(valid_uuid.into())).unwrap();
        assert!(parsed.is_some());
        assert_eq!(parsed.unwrap().0, valid_uuid);

        let invalid = parse_cart_id(Some("not-a-uuid".into()));
        assert!(invalid.is_err());

        let empty = parse_cart_id(Some("  ".into())).unwrap();
        assert!(empty.is_none());
    }
}
