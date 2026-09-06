mod graphql_adapter;

use crate::model::{OrderAdminBootstrap, OrderDetail, OrderDetailEnvelope, OrderList};
pub use graphql_adapter::ApiError;

pub async fn fetch_bootstrap(
    token: Option<String>,
    tenant_slug: Option<String>,
) -> Result<OrderAdminBootstrap, ApiError> {
    graphql_adapter::fetch_bootstrap(token, tenant_slug).await
}

pub async fn fetch_orders(
    token: Option<String>,
    tenant_slug: Option<String>,
    tenant_id: String,
    status: Option<String>,
    page: u64,
    per_page: u64,
) -> Result<OrderList, ApiError> {
    graphql_adapter::fetch_orders(token, tenant_slug, tenant_id, status, page, per_page).await
}

pub async fn fetch_order_detail(
    token: Option<String>,
    tenant_slug: Option<String>,
    tenant_id: String,
    id: String,
) -> Result<Option<OrderDetailEnvelope>, ApiError> {
    graphql_adapter::fetch_order_detail(token, tenant_slug, tenant_id, id).await
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
    graphql_adapter::mark_order_paid(
        token,
        tenant_slug,
        tenant_id,
        user_id,
        id,
        payment_id,
        payment_method,
    )
    .await
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
    graphql_adapter::ship_order(
        token,
        tenant_slug,
        tenant_id,
        user_id,
        id,
        tracking_number,
        carrier,
    )
    .await
}

pub async fn deliver_order(
    token: Option<String>,
    tenant_slug: Option<String>,
    tenant_id: String,
    user_id: String,
    id: String,
    delivered_signature: Option<String>,
) -> Result<OrderDetail, ApiError> {
    graphql_adapter::deliver_order(
        token,
        tenant_slug,
        tenant_id,
        user_id,
        id,
        delivered_signature,
    )
    .await
}

pub async fn cancel_order(
    token: Option<String>,
    tenant_slug: Option<String>,
    tenant_id: String,
    user_id: String,
    id: String,
    reason: Option<String>,
) -> Result<OrderDetail, ApiError> {
    graphql_adapter::cancel_order(token, tenant_slug, tenant_id, user_id, id, reason).await
}
