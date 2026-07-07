use leptos::prelude::*;
use rustok_ui_core::normalize_optional_ui_text;
use serde::{Deserialize, Serialize};
use std::fmt::{Display, Formatter};
#[cfg(feature = "ssr")]
use uuid::Uuid;

use crate::model::{
    StorefrontCheckoutAdjustment, StorefrontCheckoutCart, StorefrontCheckoutDeliveryGroup,
    StorefrontCheckoutShippingOption, StorefrontCheckoutWorkspace, StorefrontCommerceData,
};

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

impl From<ServerFnError> for ApiError {
    fn from(value: ServerFnError) -> Self {
        Self::ServerFn(value.to_string())
    }
}

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

#[allow(dead_code)]
fn resolve_requested_locale(
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

fn normalize_cart_id(value: Option<String>) -> Option<String> {
    normalize_optional_ui_text(value)
}

#[cfg(feature = "ssr")]
fn parse_cart_id(value: Option<String>) -> Result<Option<(String, Uuid)>, ApiError> {
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

fn map_cart_transport_error(
    error: rustok_cart_storefront::transport::CartTransportError,
) -> ApiError {
    let message = error.to_string();
    if message.contains("cart_id must be a valid UUID") {
        ApiError::Validation("cart_id must be a valid UUID".to_string())
    } else {
        ApiError::ServerFn(message)
    }
}

fn map_payment_transport_error(error: rustok_ui_transport::UiTransportError) -> ApiError {
    match error.failed_path {
        rustok_ui_transport::UiTransportPath::NativeServer => ApiError::ServerFn(error.to_string()),
        rustok_ui_transport::UiTransportPath::Graphql => ApiError::Graphql(error.to_string()),
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

fn map_cart_checkout_cart(
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

pub async fn fetch_storefront_commerce_server(
    selected_cart_id: Option<String>,
    locale: Option<String>,
) -> Result<StorefrontCommerceData, ApiError> {
    storefront_commerce_native(selected_cart_id, locale)
        .await
        .map_err(ApiError::from)
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
    let payment_collection = if cart_data.cart.is_some() {
        rustok_payment_storefront::transport::fetch_payment_collection(
            rustok_payment_storefront::transport::build_payment_collection_fetch_request(
                cart_data.selected_cart_id.clone().unwrap_or_default(),
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

#[server(prefix = "/api/fn", endpoint = "commerce/storefront-data")]
async fn storefront_commerce_native(
    selected_cart_id: Option<String>,
    locale: Option<String>,
) -> Result<StorefrontCommerceData, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        let request_context = leptos_axum::extract::<rustok_api::RequestContext>()
            .await
            .map_err(ServerFnError::new)?;
        let tenant = leptos_axum::extract::<rustok_api::TenantContext>()
            .await
            .map_err(ServerFnError::new)?;
        let normalized_locale = resolve_requested_locale(
            locale,
            Some(request_context.locale.as_str()),
            tenant.default_locale.as_str(),
        );
        let mut data = StorefrontCommerceData {
            effective_locale: normalized_locale,
            tenant_slug: Some(tenant.slug),
            tenant_default_locale: tenant.default_locale,
            channel_slug: request_context.channel_slug.clone(),
            channel_resolution_source: request_context
                .channel_resolution_source
                .as_ref()
                .map(|source| source.as_str().to_string()),
            selected_cart_id: None,
            checkout: None,
        };

        let Some((normalized_cart_id, _)) =
            parse_cart_id(selected_cart_id).map_err(|err| ServerFnError::new(err.to_string()))?
        else {
            return Ok(data);
        };
        let cart_data = rustok_cart_storefront::transport::fetch_cart(
            rustok_cart_storefront::core::build_cart_fetch_request(
                Some(normalized_cart_id.clone()),
                Some(data.effective_locale.clone()),
            ),
        )
        .await
        .map_err(|err| ServerFnError::new(err.to_string()))?;
        let payment_collection = if cart_data.cart.is_some() {
            rustok_payment_storefront::transport::fetch_payment_collection(
                rustok_payment_storefront::transport::build_payment_collection_fetch_request(
                    normalized_cart_id.clone(),
                ),
            )
            .await
            .map_err(|err| ServerFnError::new(err.to_string()))?
        } else {
            None
        };

        data.selected_cart_id = Some(normalized_cart_id);
        data.checkout = Some(StorefrontCheckoutWorkspace {
            cart: cart_data.cart.map(map_cart_checkout_cart),
            payment_collection,
        });
        Ok(data)
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = (selected_cart_id, locale);
        Err(ServerFnError::new(
            "commerce/storefront-data requires the `ssr` feature",
        ))
    }
}
