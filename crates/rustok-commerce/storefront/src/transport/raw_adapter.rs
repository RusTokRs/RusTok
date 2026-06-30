use leptos::prelude::*;
use leptos_graphql::{execute as execute_graphql, GraphqlHttpError, GraphqlRequest};
use serde::{Deserialize, Serialize};
use std::fmt::{Display, Formatter};
use std::str::FromStr;
use uuid::Uuid;

#[cfg(feature = "ssr")]
use crate::model::StorefrontCheckoutPaymentCollection;
use crate::model::{
    StorefrontCheckoutAdjustment, StorefrontCheckoutCart, StorefrontCheckoutDeliveryGroup,
    StorefrontCheckoutShippingOption, StorefrontCheckoutWorkspace, StorefrontCommerceData,
    StorefrontOrderRefundSummary,
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

impl From<GraphqlHttpError> for ApiError {
    fn from(value: GraphqlHttpError) -> Self {
        Self::Graphql(value.to_string())
    }
}

impl From<ServerFnError> for ApiError {
    fn from(value: ServerFnError) -> Self {
        Self::ServerFn(value.to_string())
    }
}

#[allow(dead_code)]
const STOREFRONT_REFUNDS_QUERY: &str = "query StorefrontRefundsSummary($orderId: UUID!, $filter: StorefrontRefundsFilter) { storefrontRefunds(orderId: $orderId, filter: $filter) { total items { amount status } } }";

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
struct StorefrontRefundsSummaryResponse {
    #[serde(rename = "storefrontRefunds")]
    storefront_refunds: GraphqlRefundList,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
struct GraphqlRefundList {
    total: u64,
    items: Vec<GraphqlRefundItem>,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
struct GraphqlRefundItem {
    amount: String,
    status: String,
}

#[allow(dead_code)]
#[derive(Debug, Serialize)]
struct StorefrontRefundsSummaryVariables {
    #[serde(rename = "orderId")]
    order_id: Uuid,
    filter: StorefrontRefundsSummaryFilter,
}

#[allow(dead_code)]
#[derive(Debug, Serialize)]
struct StorefrontRefundsSummaryFilter {
    page: u64,
    #[serde(rename = "perPage")]
    per_page: u64,
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

fn normalize_optional(value: Option<String>) -> Option<String> {
    value.and_then(|value| {
        let trimmed = value.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_string())
        }
    })
}

#[allow(dead_code)]
fn resolve_requested_locale(
    requested: Option<String>,
    request_context_locale: Option<&str>,
    tenant_default_locale: &str,
) -> String {
    normalize_optional(requested)
        .or_else(|| {
            request_context_locale.and_then(|value| normalize_optional(Some(value.to_string())))
        })
        .or_else(|| normalize_optional(Some(tenant_default_locale.to_string())))
        .unwrap_or_default()
}

fn normalize_cart_id(value: Option<String>) -> Option<String> {
    normalize_optional(value)
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

fn graphql_url() -> String {
    if let Ok(url) = std::env::var("RUSTOK_GRAPHQL_URL") {
        return url;
    }

    #[cfg(target_arch = "wasm32")]
    {
        let origin = web_sys::window()
            .and_then(|window| window.location().origin().ok())
            .unwrap_or_else(|| "http://localhost:5150".to_string());
        format!("{origin}/api/graphql")
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let base =
            std::env::var("RUSTOK_API_URL").unwrap_or_else(|_| "http://localhost:5150".to_string());
        format!("{base}/api/graphql")
    }
}

async fn request<V, T>(query: &str, variables: V) -> Result<T, ApiError>
where
    V: Serialize,
    T: for<'de> Deserialize<'de>,
{
    execute_graphql(
        &graphql_url(),
        GraphqlRequest::new(query, Some(variables)),
        None,
        configured_tenant_slug(),
        None,
    )
    .await
    .map_err(ApiError::from)
}

fn fallback_storefront_commerce(
    selected_cart_id: Option<String>,
    locale: Option<String>,
) -> StorefrontCommerceData {
    let effective_locale = normalize_optional(locale).unwrap_or_default();
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

#[cfg(feature = "ssr")]
fn map_native_payment_collection(
    value: rustok_payment::dto::PaymentCollectionResponse,
) -> StorefrontCheckoutPaymentCollection {
    StorefrontCheckoutPaymentCollection {
        id: value.id.to_string(),
        status: value.status,
        currency_code: value.currency_code,
        amount: value.amount.normalize().to_string(),
        authorized_amount: value.authorized_amount.normalize().to_string(),
        captured_amount: value.captured_amount.normalize().to_string(),
        order_id: value.order_id.map(|value| value.to_string()),
        provider_id: value.provider_id,
        payment_count: value.payments.len() as u64,
        created_at: value.created_at.to_rfc3339(),
        updated_at: value.updated_at.to_rfc3339(),
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

    data.selected_cart_id = cart_data.selected_cart_id;
    data.checkout = Some(StorefrontCheckoutWorkspace {
        cart: cart_data.cart.map(map_cart_checkout_cart),
        payment_collection: None,
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
        use leptos::prelude::expect_context;
        use loco_rs::app::AppContext;

        let app_ctx = expect_context::<AppContext>();
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

        let Some((normalized_cart_id, cart_id)) =
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
            rustok_payment::PaymentService::new(app_ctx.db.clone())
                .find_reusable_collection_by_cart(tenant.id, cart_id)
                .await
                .map_err(|err| ServerFnError::new(err.to_string()))?
        } else {
            None
        };

        data.selected_cart_id = Some(normalized_cart_id);
        data.checkout = Some(StorefrontCheckoutWorkspace {
            cart: cart_data.cart.map(map_cart_checkout_cart),
            payment_collection: payment_collection.map(map_native_payment_collection),
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

#[allow(dead_code)]
fn summarize_storefront_refunds(
    items: &[GraphqlRefundItem],
    total: u64,
) -> StorefrontOrderRefundSummary {
    let refunded_amount = items
        .iter()
        .filter_map(|item| rust_decimal::Decimal::from_str(item.amount.trim()).ok())
        .fold(rust_decimal::Decimal::ZERO, |acc, value| acc + value);

    StorefrontOrderRefundSummary {
        total,
        refunded_amount: if total == 0 {
            None
        } else {
            Some(refunded_amount.normalize().to_string())
        },
        latest_status: items.first().map(|item| item.status.clone()),
    }
}

#[allow(dead_code)]
pub async fn fetch_storefront_order_refunds_summary(
    order_id: String,
) -> Result<StorefrontOrderRefundSummary, ApiError> {
    let order_id = Uuid::parse_str(order_id.trim())
        .map_err(|_| ApiError::Validation("order_id must be a valid UUID".to_string()))?;

    let response: StorefrontRefundsSummaryResponse = request(
        STOREFRONT_REFUNDS_QUERY,
        StorefrontRefundsSummaryVariables {
            order_id,
            filter: StorefrontRefundsSummaryFilter {
                page: 1,
                per_page: 50,
            },
        },
    )
    .await?;

    Ok(summarize_storefront_refunds(
        &response.storefront_refunds.items,
        response.storefront_refunds.total,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn summarize_storefront_refunds_uses_decimal_safe_total() {
        let summary = summarize_storefront_refunds(
            &[
                GraphqlRefundItem {
                    amount: "0.10".to_string(),
                    status: "pending".to_string(),
                },
                GraphqlRefundItem {
                    amount: "0.20".to_string(),
                    status: "refunded".to_string(),
                },
            ],
            2,
        );

        assert_eq!(summary.total, 2);
        assert_eq!(summary.refunded_amount.as_deref(), Some("0.3"));
        assert_eq!(summary.latest_status.as_deref(), Some("pending"));
    }

    #[test]
    fn summarize_storefront_refunds_ignores_invalid_rows_and_handles_empty_total() {
        let summary = summarize_storefront_refunds(
            &[GraphqlRefundItem {
                amount: "invalid".to_string(),
                status: "pending".to_string(),
            }],
            0,
        );

        assert_eq!(summary.total, 0);
        assert_eq!(summary.refunded_amount, None);
        assert_eq!(summary.latest_status.as_deref(), Some("pending"));
    }

    #[test]
    fn summarize_storefront_refunds_non_zero_total_with_invalid_amounts_returns_zero_string() {
        let summary = summarize_storefront_refunds(
            &[
                GraphqlRefundItem {
                    amount: "invalid".to_string(),
                    status: "pending".to_string(),
                },
                GraphqlRefundItem {
                    amount: "NaN".to_string(),
                    status: "failed".to_string(),
                },
            ],
            2,
        );

        assert_eq!(summary.total, 2);
        assert_eq!(summary.refunded_amount.as_deref(), Some("0"));
        assert_eq!(summary.latest_status.as_deref(), Some("pending"));
    }

    #[tokio::test]
    async fn fetch_storefront_order_refunds_summary_rejects_invalid_uuid() {
        let result = fetch_storefront_order_refunds_summary("not-a-uuid".to_string()).await;

        match result {
            Err(ApiError::Validation(message)) => {
                assert_eq!(message, "order_id must be a valid UUID".to_string());
            }
            other => panic!("expected validation error, got {:?}", other),
        }
    }
}
