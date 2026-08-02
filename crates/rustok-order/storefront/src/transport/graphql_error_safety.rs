use std::str::FromStr;

use rustok_graphql::GraphqlHttpError;
use uuid::Uuid;

use super::{CheckoutCompletionTransportError, CompleteCheckoutRequest};

const ORDER_STOREFRONT_GRAPHQL_OWNER: &str = "rustok_order.storefront";
const ORDER_STOREFRONT_GRAPHQL_OPERATION: &str = "complete_storefront_checkout";
const ORDER_STOREFRONT_GRAPHQL_BOUNDARY: &str = "order_storefront_graphql_transport";

pub(super) struct GraphqlCallContext {
    correlation_id: String,
    tenant_slug_length: Option<usize>,
    cart_id_length: usize,
    idempotency_key_length: usize,
    create_fulfillment: bool,
}

impl GraphqlCallContext {
    pub(super) fn new(request: &CompleteCheckoutRequest) -> Self {
        Self {
            correlation_id: format!(
                "order-storefront-graphql:{}:{}",
                ORDER_STOREFRONT_GRAPHQL_OPERATION,
                Uuid::new_v4()
            ),
            tenant_slug_length: configured_tenant_slug_length(),
            cart_id_length: request.cart_id.chars().count(),
            idempotency_key_length: request.idempotency_key.chars().count(),
            create_fulfillment: request.metadata.create_fulfillment,
        }
    }

    pub(super) fn map_error(
        &self,
        error: CheckoutCompletionTransportError,
    ) -> CheckoutCompletionTransportError {
        let CheckoutCompletionTransportError::Graphql(raw_error) = error else {
            return error;
        };
        let raw_error_present = !raw_error.trim().is_empty();
        let raw_error_length = raw_error.chars().count();
        let parsed_error = GraphqlHttpError::from_str(raw_error.as_str());
        let parsed_error_valid = parsed_error.is_ok();
        let (error_kind, code, public_message, technical_failure) = match &parsed_error {
            Ok(GraphqlHttpError::Network) => (
                "network",
                "order.storefront_graphql_network_unavailable",
                "Checkout completion is temporarily unavailable",
                true,
            ),
            Ok(GraphqlHttpError::Http(_)) => (
                "http",
                "order.storefront_graphql_http_unavailable",
                "Checkout completion is temporarily unavailable",
                true,
            ),
            Ok(GraphqlHttpError::Unauthorized) => (
                "unauthorized",
                "order.storefront_graphql_authentication_required",
                "Checkout authentication is required",
                false,
            ),
            Ok(GraphqlHttpError::Graphql(_)) => (
                "graphql",
                "order.storefront_graphql_request_rejected",
                "Checkout request could not be completed",
                false,
            ),
            Err(_) => (
                "unknown",
                "order.storefront_graphql_unknown_failure",
                "Checkout request could not be completed",
                true,
            ),
        };

        if technical_failure {
            tracing::error!(
                raw_error_present,
                raw_error_length,
                parsed_error_valid,
                owner = ORDER_STOREFRONT_GRAPHQL_OWNER,
                owner_operation = ORDER_STOREFRONT_GRAPHQL_OPERATION,
                correlation_id = %self.correlation_id,
                tenant_slug_configured = self.tenant_slug_length.is_some(),
                tenant_slug_length = ?self.tenant_slug_length,
                cart_id_length = self.cart_id_length,
                idempotency_key_length = self.idempotency_key_length,
                create_fulfillment = self.create_fulfillment,
                error_kind,
                code,
                boundary = ORDER_STOREFRONT_GRAPHQL_BOUNDARY,
                "order storefront GraphQL transport failed"
            );
        } else {
            tracing::warn!(
                raw_error_present,
                raw_error_length,
                parsed_error_valid,
                owner = ORDER_STOREFRONT_GRAPHQL_OWNER,
                owner_operation = ORDER_STOREFRONT_GRAPHQL_OPERATION,
                correlation_id = %self.correlation_id,
                tenant_slug_configured = self.tenant_slug_length.is_some(),
                tenant_slug_length = ?self.tenant_slug_length,
                cart_id_length = self.cart_id_length,
                idempotency_key_length = self.idempotency_key_length,
                create_fulfillment = self.create_fulfillment,
                error_kind,
                code,
                boundary = ORDER_STOREFRONT_GRAPHQL_BOUNDARY,
                "order storefront GraphQL request was rejected"
            );
        }

        CheckoutCompletionTransportError::Graphql(public_message.to_string())
    }
}

fn configured_tenant_slug_length() -> Option<usize> {
    [
        "RUSTOK_TENANT_SLUG",
        "NEXT_PUBLIC_TENANT_SLUG",
        "NEXT_PUBLIC_DEFAULT_TENANT_SLUG",
    ]
    .into_iter()
    .find_map(|key| {
        std::env::var(key).ok().and_then(|value| {
            let value = value.trim();
            (!value.is_empty()).then_some(value.chars().count())
        })
    })
}
