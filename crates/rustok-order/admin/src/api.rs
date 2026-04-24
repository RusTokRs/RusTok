#[cfg(target_arch = "wasm32")]
use leptos::web_sys;
use leptos_graphql::{execute as execute_graphql, GraphqlHttpError, GraphqlRequest};
use serde::{Deserialize, Serialize};

use crate::model::{OrderAdminBootstrap, OrderDetail, OrderDetailEnvelope, OrderList};

pub type ApiError = GraphqlHttpError;

const BOOTSTRAP_QUERY: &str =
    "query OrderAdminBootstrap { currentTenant { id slug name } me { id email name } }";
const ORDERS_QUERY: &str = "query OrderAdminOrders($tenantId: UUID!, $filter: OrdersFilter) { orders(tenantId: $tenantId, filter: $filter) { total page perPage hasNext items { id customerId status currencyCode totalAmount trackingNumber carrier createdAt confirmedAt paidAt shippedAt deliveredAt cancelledAt lineItems { id orderId productId variantId shippingProfileSlug sku title quantity unitPrice totalPrice currencyCode createdAt } } } }";
const ORDER_QUERY: &str = "query OrderAdminOrder($tenantId: UUID!, $id: UUID!) { order(tenantId: $tenantId, id: $id) { order { id tenantId channelId channelSlug customerId status currencyCode totalAmount metadata paymentId paymentMethod trackingNumber carrier cancellationReason deliveredSignature createdAt updatedAt confirmedAt paidAt shippedAt deliveredAt cancelledAt lineItems { id orderId productId variantId shippingProfileSlug sku title quantity unitPrice totalPrice currencyCode metadata createdAt } } paymentCollection { id status currencyCode amount authorizedAmount capturedAmount providerId createdAt updatedAt authorizedAt capturedAt cancelledAt payments { id providerId providerPaymentId status currencyCode amount capturedAmount errorMessage createdAt updatedAt authorizedAt capturedAt cancelledAt } } fulfillment { id tenantId orderId shippingOptionId customerId status carrier trackingNumber deliveredNote cancellationReason metadata createdAt updatedAt shippedAt deliveredAt cancelledAt } } }";
const MARK_ORDER_PAID_MUTATION: &str = "mutation OrderAdminMarkOrderPaid($tenantId: UUID!, $userId: UUID!, $id: UUID!, $input: MarkPaidOrderInput!) { markOrderPaid(tenantId: $tenantId, userId: $userId, id: $id, input: $input) { id tenantId channelId channelSlug customerId status currencyCode totalAmount metadata paymentId paymentMethod trackingNumber carrier cancellationReason deliveredSignature createdAt updatedAt confirmedAt paidAt shippedAt deliveredAt cancelledAt lineItems { id orderId productId variantId shippingProfileSlug sku title quantity unitPrice totalPrice currencyCode metadata createdAt } } }";
const SHIP_ORDER_MUTATION: &str = "mutation OrderAdminShipOrder($tenantId: UUID!, $userId: UUID!, $id: UUID!, $input: ShipOrderInput!) { shipOrder(tenantId: $tenantId, userId: $userId, id: $id, input: $input) { id tenantId channelId channelSlug customerId status currencyCode totalAmount metadata paymentId paymentMethod trackingNumber carrier cancellationReason deliveredSignature createdAt updatedAt confirmedAt paidAt shippedAt deliveredAt cancelledAt lineItems { id orderId productId variantId shippingProfileSlug sku title quantity unitPrice totalPrice currencyCode metadata createdAt } } }";
const DELIVER_ORDER_MUTATION: &str = "mutation OrderAdminDeliverOrder($tenantId: UUID!, $userId: UUID!, $id: UUID!, $input: DeliverOrderInput!) { deliverOrder(tenantId: $tenantId, userId: $userId, id: $id, input: $input) { id tenantId channelId channelSlug customerId status currencyCode totalAmount metadata paymentId paymentMethod trackingNumber carrier cancellationReason deliveredSignature createdAt updatedAt confirmedAt paidAt shippedAt deliveredAt cancelledAt lineItems { id orderId productId variantId shippingProfileSlug sku title quantity unitPrice totalPrice currencyCode metadata createdAt } } }";
const CANCEL_ORDER_MUTATION: &str = "mutation OrderAdminCancelOrder($tenantId: UUID!, $userId: UUID!, $id: UUID!, $input: CancelOrderInput!) { cancelOrder(tenantId: $tenantId, userId: $userId, id: $id, input: $input) { id tenantId channelId channelSlug customerId status currencyCode totalAmount metadata paymentId paymentMethod trackingNumber carrier cancellationReason deliveredSignature createdAt updatedAt confirmedAt paidAt shippedAt deliveredAt cancelledAt lineItems { id orderId productId variantId shippingProfileSlug sku title quantity unitPrice totalPrice currencyCode metadata createdAt } } }";

#[derive(Debug, Deserialize)]
struct BootstrapResponse {
    #[serde(rename = "currentTenant")]
    current_tenant: crate::model::CurrentTenant,
    me: crate::model::CurrentUser,
}

#[derive(Debug, Deserialize)]
struct OrdersResponse {
    orders: OrderList,
}

#[derive(Debug, Deserialize)]
struct OrderResponse {
    order: Option<OrderDetailEnvelope>,
}

#[derive(Debug, Deserialize)]
struct MarkOrderPaidResponse {
    #[serde(rename = "markOrderPaid")]
    mark_order_paid: OrderDetail,
}

#[derive(Debug, Deserialize)]
struct ShipOrderResponse {
    #[serde(rename = "shipOrder")]
    ship_order: OrderDetail,
}

#[derive(Debug, Deserialize)]
struct DeliverOrderResponse {
    #[serde(rename = "deliverOrder")]
    deliver_order: OrderDetail,
}

#[derive(Debug, Deserialize)]
struct CancelOrderResponse {
    #[serde(rename = "cancelOrder")]
    cancel_order: OrderDetail,
}

#[derive(Debug, Serialize)]
struct TenantScopedVariables<T> {
    #[serde(rename = "tenantId")]
    tenant_id: String,
    #[serde(flatten)]
    extra: T,
}

#[derive(Debug, Serialize)]
struct TenantUserScopedVariables<T> {
    #[serde(rename = "tenantId")]
    tenant_id: String,
    #[serde(rename = "userId")]
    user_id: String,
    #[serde(flatten)]
    extra: T,
}

#[derive(Debug, Serialize)]
struct OrderVariables {
    id: String,
}

#[derive(Debug, Serialize)]
struct OrdersVariables {
    filter: OrdersFilter,
}

#[derive(Debug, Serialize)]
struct OrderLifecycleVariables<T> {
    id: String,
    input: T,
}

#[derive(Debug, Serialize)]
struct OrdersFilter {
    status: Option<String>,
    page: Option<u64>,
    #[serde(rename = "perPage")]
    per_page: Option<u64>,
}

#[derive(Debug, Serialize)]
struct MarkPaidOrderInput {
    #[serde(rename = "paymentId")]
    payment_id: String,
    #[serde(rename = "paymentMethod")]
    payment_method: String,
}

#[derive(Debug, Serialize)]
struct ShipOrderInput {
    #[serde(rename = "trackingNumber")]
    tracking_number: String,
    carrier: String,
}

#[derive(Debug, Serialize)]
struct DeliverOrderInput {
    #[serde(rename = "deliveredSignature")]
    delivered_signature: Option<String>,
}

#[derive(Debug, Serialize)]
struct CancelOrderInput {
    reason: Option<String>,
}

fn graphql_url() -> String {
    if let Some(url) = option_env!("RUSTOK_GRAPHQL_URL") {
        return url.to_string();
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

async fn request<V, T>(
    query: &str,
    variables: Option<V>,
    token: Option<String>,
    tenant_slug: Option<String>,
) -> Result<T, ApiError>
where
    V: Serialize,
    T: for<'de> Deserialize<'de>,
{
    execute_graphql(
        &graphql_url(),
        GraphqlRequest::new(query, variables),
        token,
        tenant_slug,
        None,
    )
    .await
}

pub async fn fetch_bootstrap(
    token: Option<String>,
    tenant_slug: Option<String>,
) -> Result<OrderAdminBootstrap, ApiError> {
    let response: BootstrapResponse =
        request::<serde_json::Value, BootstrapResponse>(BOOTSTRAP_QUERY, None, token, tenant_slug)
            .await?;
    Ok(OrderAdminBootstrap {
        current_tenant: response.current_tenant,
        me: response.me,
    })
}

pub async fn fetch_orders(
    token: Option<String>,
    tenant_slug: Option<String>,
    tenant_id: String,
    status: Option<String>,
) -> Result<OrderList, ApiError> {
    let response: OrdersResponse = request(
        ORDERS_QUERY,
        Some(TenantScopedVariables {
            tenant_id,
            extra: OrdersVariables {
                filter: OrdersFilter {
                    status,
                    page: Some(1),
                    per_page: Some(24),
                },
            },
        }),
        token,
        tenant_slug,
    )
    .await?;
    Ok(response.orders)
}

pub async fn fetch_order_detail(
    token: Option<String>,
    tenant_slug: Option<String>,
    tenant_id: String,
    id: String,
) -> Result<Option<OrderDetailEnvelope>, ApiError> {
    let response: OrderResponse = request(
        ORDER_QUERY,
        Some(TenantScopedVariables {
            tenant_id,
            extra: OrderVariables { id },
        }),
        token,
        tenant_slug,
    )
    .await?;
    Ok(response.order)
}

pub async fn mark_order_paid(
    token: Option<String>,
    tenant_slug: Option<String>,
    tenant_id: String,
    user_id: String,
    id: String,
    payment_id: String,
    payment_method: String,
) -> Result<OrderDetail, ApiError> {
    let response: MarkOrderPaidResponse = request(
        MARK_ORDER_PAID_MUTATION,
        Some(TenantUserScopedVariables {
            tenant_id,
            user_id,
            extra: OrderLifecycleVariables {
                id,
                input: MarkPaidOrderInput {
                    payment_id,
                    payment_method,
                },
            },
        }),
        token,
        tenant_slug,
    )
    .await?;
    Ok(response.mark_order_paid)
}

pub async fn ship_order(
    token: Option<String>,
    tenant_slug: Option<String>,
    tenant_id: String,
    user_id: String,
    id: String,
    tracking_number: String,
    carrier: String,
) -> Result<OrderDetail, ApiError> {
    let response: ShipOrderResponse = request(
        SHIP_ORDER_MUTATION,
        Some(TenantUserScopedVariables {
            tenant_id,
            user_id,
            extra: OrderLifecycleVariables {
                id,
                input: ShipOrderInput {
                    tracking_number,
                    carrier,
                },
            },
        }),
        token,
        tenant_slug,
    )
    .await?;
    Ok(response.ship_order)
}

pub async fn deliver_order(
    token: Option<String>,
    tenant_slug: Option<String>,
    tenant_id: String,
    user_id: String,
    id: String,
    delivered_signature: Option<String>,
) -> Result<OrderDetail, ApiError> {
    let response: DeliverOrderResponse = request(
        DELIVER_ORDER_MUTATION,
        Some(TenantUserScopedVariables {
            tenant_id,
            user_id,
            extra: OrderLifecycleVariables {
                id,
                input: DeliverOrderInput {
                    delivered_signature,
                },
            },
        }),
        token,
        tenant_slug,
    )
    .await?;
    Ok(response.deliver_order)
}

pub async fn cancel_order(
    token: Option<String>,
    tenant_slug: Option<String>,
    tenant_id: String,
    user_id: String,
    id: String,
    reason: Option<String>,
) -> Result<OrderDetail, ApiError> {
    let response: CancelOrderResponse = request(
        CANCEL_ORDER_MUTATION,
        Some(TenantUserScopedVariables {
            tenant_id,
            user_id,
            extra: OrderLifecycleVariables {
                id,
                input: CancelOrderInput { reason },
            },
        }),
        token,
        tenant_slug,
    )
    .await?;
    Ok(response.cancel_order)
}
