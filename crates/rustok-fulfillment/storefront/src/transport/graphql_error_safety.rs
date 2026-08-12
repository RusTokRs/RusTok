use std::str::FromStr;

use rustok_graphql::GraphqlHttpError;
use uuid::Uuid;

use super::{SelectShippingOptionRequest, ShippingSelectionTransportError};

const FULFILLMENT_STOREFRONT_GRAPHQL_OWNER: &str = "rustok_fulfillment.storefront";
const FULFILLMENT_STOREFRONT_GRAPHQL_OPERATION: &str = "select_storefront_shipping_option";
const FULFILLMENT_STOREFRONT_GRAPHQL_BOUNDARY: &str = "fulfillment_storefront_graphql_transport";

pub(super) struct GraphqlCallContext {
    correlation_id: String,
    tenant_slug_length: Option<usize>,
    cart_id_length: usize,
    delivery_group_count: usize,
    shipping_profile_slug_length: usize,
    seller_id_present: bool,
    shipping_option_id_present: bool,
}

impl GraphqlCallContext {
    pub(super) fn new(request: &SelectShippingOptionRequest) -> Self {
        Self {
            correlation_id: format!(
                "fulfillment-storefront-graphql:{}:{}",
                FULFILLMENT_STOREFRONT_GRAPHQL_OPERATION,
                Uuid::new_v4()
            ),
            tenant_slug_length: configured_tenant_slug_length(),
            cart_id_length: request.cart_id.chars().count(),
            delivery_group_count: request.delivery_groups.len(),
            shipping_profile_slug_length: request.shipping_profile_slug.chars().count(),
            seller_id_present: request.seller_id.is_some(),
            shipping_option_id_present: request.shipping_option_id.is_some(),
        }
    }

    pub(super) fn map_error(
        &self,
        error: ShippingSelectionTransportError,
    ) -> ShippingSelectionTransportError {
        let ShippingSelectionTransportError::Graphql(raw_error) = error else {
            return error;
        };
        let raw_error_present = !raw_error.trim().is_empty();
        let raw_error_length = raw_error.chars().count();
        let parsed_error = GraphqlHttpError::from_str(raw_error.as_str());
        let parsed_error_valid = parsed_error.is_ok();
        let (error_kind, code, public_message, technical_failure) = match &parsed_error {
            Ok(GraphqlHttpError::Network) => (
                "network",
                "fulfillment.storefront_graphql_network_unavailable",
                "Shipping selection is temporarily unavailable",
                true,
            ),
            Ok(GraphqlHttpError::Http(_)) => (
                "http",
                "fulfillment.storefront_graphql_http_unavailable",
                "Shipping selection is temporarily unavailable",
                true,
            ),
            Ok(GraphqlHttpError::Unauthorized) => (
                "unauthorized",
                "fulfillment.storefront_graphql_authentication_required",
                "Shipping selection authentication is required",
                false,
            ),
            Ok(GraphqlHttpError::Graphql(_)) => (
                "graphql",
                "fulfillment.storefront_graphql_request_rejected",
                "Shipping selection request could not be completed",
                false,
            ),
            Err(_) => (
                "unknown",
                "fulfillment.storefront_graphql_unknown_failure",
                "Shipping selection request could not be completed",
                true,
            ),
        };

        if technical_failure {
            tracing::error!(
                raw_error_present,
                raw_error_length,
                parsed_error_valid,
                owner = FULFILLMENT_STOREFRONT_GRAPHQL_OWNER,
                owner_operation = FULFILLMENT_STOREFRONT_GRAPHQL_OPERATION,
                correlation_id = %self.correlation_id,
                tenant_slug_configured = self.tenant_slug_length.is_some(),
                tenant_slug_length = ?self.tenant_slug_length,
                cart_id_length = self.cart_id_length,
                delivery_group_count = self.delivery_group_count,
                shipping_profile_slug_length = self.shipping_profile_slug_length,
                seller_id_present = self.seller_id_present,
                shipping_option_id_present = self.shipping_option_id_present,
                error_kind,
                code,
                boundary = FULFILLMENT_STOREFRONT_GRAPHQL_BOUNDARY,
                "fulfillment storefront GraphQL transport failed"
            );
        } else {
            tracing::warn!(
                raw_error_present,
                raw_error_length,
                parsed_error_valid,
                owner = FULFILLMENT_STOREFRONT_GRAPHQL_OWNER,
                owner_operation = FULFILLMENT_STOREFRONT_GRAPHQL_OPERATION,
                correlation_id = %self.correlation_id,
                tenant_slug_configured = self.tenant_slug_length.is_some(),
                tenant_slug_length = ?self.tenant_slug_length,
                cart_id_length = self.cart_id_length,
                delivery_group_count = self.delivery_group_count,
                shipping_profile_slug_length = self.shipping_profile_slug_length,
                seller_id_present = self.seller_id_present,
                shipping_option_id_present = self.shipping_option_id_present,
                error_kind,
                code,
                boundary = FULFILLMENT_STOREFRONT_GRAPHQL_BOUNDARY,
                "fulfillment storefront GraphQL request was rejected"
            );
        }

        ShippingSelectionTransportError::Graphql(public_message.to_string())
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
