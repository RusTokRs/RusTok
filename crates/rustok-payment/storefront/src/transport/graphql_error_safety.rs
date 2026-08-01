use std::str::FromStr;

use rustok_graphql::GraphqlHttpError;
use uuid::Uuid;

use super::PaymentTransportError;

const PAYMENT_STOREFRONT_GRAPHQL_OWNER: &str = "rustok_payment.storefront";
const PAYMENT_STOREFRONT_GRAPHQL_BOUNDARY: &str = "payment_storefront_graphql_transport";

pub(super) struct GraphqlCallContext {
    owner_operation: &'static str,
    correlation_id: String,
    tenant_slug_length: Option<usize>,
}

impl GraphqlCallContext {
    pub(super) fn new(owner_operation: &'static str) -> Self {
        Self {
            owner_operation,
            correlation_id: format!(
                "payment-storefront-graphql:{owner_operation}:{}",
                Uuid::new_v4()
            ),
            tenant_slug_length: configured_tenant_slug_length(),
        }
    }

    pub(super) fn map_error(&self, error: PaymentTransportError) -> PaymentTransportError {
        let PaymentTransportError::Graphql(raw_error) = error else {
            return error;
        };
        let raw_error_present = !raw_error.trim().is_empty();
        let raw_error_length = raw_error.chars().count();
        let parsed_error = GraphqlHttpError::from_str(raw_error.as_str());
        let parsed_error_valid = parsed_error.is_ok();
        let (error_kind, code, public_message, technical_failure) = match &parsed_error {
            Ok(GraphqlHttpError::Network) => (
                "network",
                "payment.storefront_graphql_network_unavailable",
                "Payment storefront is temporarily unavailable",
                true,
            ),
            Ok(GraphqlHttpError::Http(_)) => (
                "http",
                "payment.storefront_graphql_http_unavailable",
                "Payment storefront is temporarily unavailable",
                true,
            ),
            Ok(GraphqlHttpError::Unauthorized) => (
                "unauthorized",
                "payment.storefront_graphql_authentication_required",
                "Payment storefront authentication is required",
                false,
            ),
            Ok(GraphqlHttpError::Graphql(_)) => (
                "graphql",
                "payment.storefront_graphql_request_rejected",
                "Payment storefront request could not be completed",
                false,
            ),
            Err(_) => (
                "unknown",
                "payment.storefront_graphql_unknown_failure",
                "Payment storefront request could not be completed",
                true,
            ),
        };

        if technical_failure {
            tracing::error!(
                raw_error_present,
                raw_error_length,
                parsed_error_valid,
                owner = PAYMENT_STOREFRONT_GRAPHQL_OWNER,
                owner_operation = self.owner_operation,
                correlation_id = %self.correlation_id,
                tenant_slug_configured = self.tenant_slug_length.is_some(),
                tenant_slug_length = ?self.tenant_slug_length,
                error_kind,
                code,
                boundary = PAYMENT_STOREFRONT_GRAPHQL_BOUNDARY,
                "payment storefront GraphQL transport failed"
            );
        } else {
            tracing::warn!(
                raw_error_present,
                raw_error_length,
                parsed_error_valid,
                owner = PAYMENT_STOREFRONT_GRAPHQL_OWNER,
                owner_operation = self.owner_operation,
                correlation_id = %self.correlation_id,
                tenant_slug_configured = self.tenant_slug_length.is_some(),
                tenant_slug_length = ?self.tenant_slug_length,
                error_kind,
                code,
                boundary = PAYMENT_STOREFRONT_GRAPHQL_BOUNDARY,
                "payment storefront GraphQL request was rejected"
            );
        }

        PaymentTransportError::Graphql(public_message.to_string())
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
