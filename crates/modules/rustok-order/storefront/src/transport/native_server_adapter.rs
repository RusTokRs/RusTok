mod native_client_error_safety;
mod server_functions;

use super::{CheckoutCompletion, CheckoutCompletionTransportError, CompleteCheckoutRequest};

pub async fn complete_checkout(
    request: CompleteCheckoutRequest,
) -> Result<CheckoutCompletion, CheckoutCompletionTransportError> {
    server_functions::complete_checkout_server(request).await
}

pub async fn fetch_order(
    order_id: String,
) -> Result<Option<crate::model::StorefrontOrder>, CheckoutCompletionTransportError> {
    server_functions::fetch_order_server(order_id).await
}

pub async fn fetch_orders(
    page: Option<u64>,
    per_page: Option<u64>,
    status: Option<String>,
) -> Result<crate::model::StorefrontOrdersResponse, CheckoutCompletionTransportError> {
    server_functions::fetch_orders_server(page, per_page, status).await
}
