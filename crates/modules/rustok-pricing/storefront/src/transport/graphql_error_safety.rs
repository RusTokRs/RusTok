use std::str::FromStr;

use rustok_graphql::GraphqlHttpError;
use uuid::Uuid;

use crate::core::StorefrontPricingQuery;

use super::native_server_adapter::ApiError;

const PRICING_STOREFRONT_GRAPHQL_OWNER: &str = "rustok_pricing.storefront";
const PRICING_STOREFRONT_GRAPHQL_OPERATION: &str = "fetch_storefront_pricing";
const PRICING_STOREFRONT_GRAPHQL_BOUNDARY: &str = "pricing_storefront_graphql_transport";

pub(super) struct GraphqlCallContext {
    correlation_id: String,
    tenant_slug_length: Option<usize>,
    selected_handle_length: Option<usize>,
    locale_length: Option<usize>,
    currency_code_length: Option<usize>,
    region_id_length: Option<usize>,
    price_list_id_length: Option<usize>,
    channel_id_length: Option<usize>,
    channel_slug_length: Option<usize>,
    quantity_present: bool,
}

impl GraphqlCallContext {
    pub(super) fn new(query: &StorefrontPricingQuery) -> Self {
        Self {
            correlation_id: format!(
                "pricing-storefront-graphql:{PRICING_STOREFRONT_GRAPHQL_OPERATION}:{}",
                Uuid::new_v4()
            ),
            tenant_slug_length: configured_tenant_slug_length(),
            selected_handle_length: text_length(query.selected_handle.as_deref()),
            locale_length: text_length(query.locale.as_deref()),
            currency_code_length: text_length(query.currency_code.as_deref()),
            region_id_length: text_length(query.region_id.as_deref()),
            price_list_id_length: text_length(query.price_list_id.as_deref()),
            channel_id_length: text_length(query.channel_id.as_deref()),
            channel_slug_length: text_length(query.channel_slug.as_deref()),
            quantity_present: query.quantity.is_some(),
        }
    }

    pub(super) fn map_error(&self, error: ApiError) -> ApiError {
        let ApiError::Graphql(raw_error) = error else {
            return error;
        };
        let raw_error_present = !raw_error.trim().is_empty();
        let raw_error_length = raw_error.chars().count();
        let parsed_error = GraphqlHttpError::from_str(raw_error.as_str());
        let parsed_error_valid = parsed_error.is_ok();
        let (error_kind, code, public_message, technical_failure) = match &parsed_error {
            Ok(GraphqlHttpError::Network) => (
                "network",
                "pricing.storefront_graphql_network_unavailable",
                "Storefront pricing is temporarily unavailable",
                true,
            ),
            Ok(GraphqlHttpError::Http(_)) => (
                "http",
                "pricing.storefront_graphql_http_unavailable",
                "Storefront pricing is temporarily unavailable",
                true,
            ),
            Ok(GraphqlHttpError::Unauthorized) => (
                "unauthorized",
                "pricing.storefront_graphql_authentication_required",
                "Pricing storefront authentication is required",
                false,
            ),
            Ok(GraphqlHttpError::Graphql(_)) => (
                "graphql",
                "pricing.storefront_graphql_request_rejected",
                "Pricing storefront request could not be completed",
                false,
            ),
            Err(_) => (
                "unknown",
                "pricing.storefront_graphql_unknown_failure",
                "Pricing storefront request could not be completed",
                true,
            ),
        };

        if technical_failure {
            tracing::error!(
                raw_error_present,
                raw_error_length,
                parsed_error_valid,
                owner = PRICING_STOREFRONT_GRAPHQL_OWNER,
                owner_operation = PRICING_STOREFRONT_GRAPHQL_OPERATION,
                correlation_id = %self.correlation_id,
                tenant_slug_configured = self.tenant_slug_length.is_some(),
                tenant_slug_length = ?self.tenant_slug_length,
                selected_handle_present = self.selected_handle_length.is_some(),
                selected_handle_length = ?self.selected_handle_length,
                locale_present = self.locale_length.is_some(),
                locale_length = ?self.locale_length,
                currency_code_present = self.currency_code_length.is_some(),
                currency_code_length = ?self.currency_code_length,
                region_id_present = self.region_id_length.is_some(),
                region_id_length = ?self.region_id_length,
                price_list_id_present = self.price_list_id_length.is_some(),
                price_list_id_length = ?self.price_list_id_length,
                channel_id_present = self.channel_id_length.is_some(),
                channel_id_length = ?self.channel_id_length,
                channel_slug_present = self.channel_slug_length.is_some(),
                channel_slug_length = ?self.channel_slug_length,
                quantity_present = self.quantity_present,
                error_kind,
                code,
                boundary = PRICING_STOREFRONT_GRAPHQL_BOUNDARY,
                "pricing storefront GraphQL transport failed"
            );
        } else {
            tracing::warn!(
                raw_error_present,
                raw_error_length,
                parsed_error_valid,
                owner = PRICING_STOREFRONT_GRAPHQL_OWNER,
                owner_operation = PRICING_STOREFRONT_GRAPHQL_OPERATION,
                correlation_id = %self.correlation_id,
                tenant_slug_configured = self.tenant_slug_length.is_some(),
                tenant_slug_length = ?self.tenant_slug_length,
                selected_handle_present = self.selected_handle_length.is_some(),
                selected_handle_length = ?self.selected_handle_length,
                locale_present = self.locale_length.is_some(),
                locale_length = ?self.locale_length,
                currency_code_present = self.currency_code_length.is_some(),
                currency_code_length = ?self.currency_code_length,
                region_id_present = self.region_id_length.is_some(),
                region_id_length = ?self.region_id_length,
                price_list_id_present = self.price_list_id_length.is_some(),
                price_list_id_length = ?self.price_list_id_length,
                channel_id_present = self.channel_id_length.is_some(),
                channel_id_length = ?self.channel_id_length,
                channel_slug_present = self.channel_slug_length.is_some(),
                channel_slug_length = ?self.channel_slug_length,
                quantity_present = self.quantity_present,
                error_kind,
                code,
                boundary = PRICING_STOREFRONT_GRAPHQL_BOUNDARY,
                "pricing storefront GraphQL request was rejected"
            );
        }

        ApiError::Graphql(public_message.to_string())
    }
}

fn text_length(value: Option<&str>) -> Option<usize> {
    value.map(|value| value.chars().count())
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
