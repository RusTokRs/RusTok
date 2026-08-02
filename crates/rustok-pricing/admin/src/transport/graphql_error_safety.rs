use std::str::FromStr;

use rustok_graphql::GraphqlHttpError;
use uuid::Uuid;

use super::native_server_adapter::ApiError;

const PRICING_ADMIN_GRAPHQL_OWNER: &str = "rustok_pricing.admin";
const PRICING_ADMIN_GRAPHQL_BOUNDARY: &str = "pricing_admin_graphql_transport";

pub(super) struct GraphqlCallContext {
    operation: &'static str,
    correlation_id: String,
    tenant_slug_length: Option<usize>,
    tenant_id_length: Option<usize>,
    resource_id_length: Option<usize>,
    locale_length: Option<usize>,
    search_length: Option<usize>,
    status_length: Option<usize>,
    currency_code_length: Option<usize>,
    region_id_length: Option<usize>,
    price_list_id_length: Option<usize>,
    channel_id_length: Option<usize>,
    channel_slug_length: Option<usize>,
    quantity_present: bool,
}

impl GraphqlCallContext {
    pub(super) fn for_bootstrap(tenant_slug: Option<&str>) -> Self {
        Self::new("fetch_bootstrap", tenant_slug)
    }

    pub(super) fn for_active_price_lists(
        tenant_slug: Option<&str>,
        channel_id: Option<&str>,
        channel_slug: Option<&str>,
    ) -> Self {
        let mut context = Self::new("fetch_active_price_lists", tenant_slug);
        context.channel_id_length = text_length(channel_id);
        context.channel_slug_length = text_length(channel_slug);
        context
    }

    pub(super) fn for_products(
        tenant_slug: Option<&str>,
        tenant_id: &str,
        locale: Option<&str>,
        search: Option<&str>,
        status: Option<&str>,
    ) -> Self {
        let mut context = Self::new("fetch_products", tenant_slug);
        context.tenant_id_length = Some(tenant_id.chars().count());
        context.locale_length = text_length(locale);
        context.search_length = text_length(search);
        context.status_length = text_length(status);
        context
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn for_product(
        tenant_slug: Option<&str>,
        tenant_id: &str,
        resource_id: &str,
        locale: Option<&str>,
        currency_code: Option<&str>,
        region_id: Option<&str>,
        price_list_id: Option<&str>,
        channel_id: Option<&str>,
        channel_slug: Option<&str>,
        quantity_present: bool,
    ) -> Self {
        let mut context = Self::new("fetch_product", tenant_slug);
        context.tenant_id_length = Some(tenant_id.chars().count());
        context.resource_id_length = Some(resource_id.chars().count());
        context.locale_length = text_length(locale);
        context.currency_code_length = text_length(currency_code);
        context.region_id_length = text_length(region_id);
        context.price_list_id_length = text_length(price_list_id);
        context.channel_id_length = text_length(channel_id);
        context.channel_slug_length = text_length(channel_slug);
        context.quantity_present = quantity_present;
        context
    }

    fn new(operation: &'static str, tenant_slug: Option<&str>) -> Self {
        Self {
            operation,
            correlation_id: format!("pricing-admin-graphql:{operation}:{}", Uuid::new_v4()),
            tenant_slug_length: text_length(tenant_slug),
            tenant_id_length: None,
            resource_id_length: None,
            locale_length: None,
            search_length: None,
            status_length: None,
            currency_code_length: None,
            region_id_length: None,
            price_list_id_length: None,
            channel_id_length: None,
            channel_slug_length: None,
            quantity_present: false,
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
                "pricing.admin_graphql_network_unavailable",
                "Pricing admin service is temporarily unavailable",
                true,
            ),
            Ok(GraphqlHttpError::Http(_)) => (
                "http",
                "pricing.admin_graphql_http_unavailable",
                "Pricing admin service is temporarily unavailable",
                true,
            ),
            Ok(GraphqlHttpError::Unauthorized) => (
                "unauthorized",
                "pricing.admin_graphql_authentication_required",
                "Pricing admin authentication is required",
                false,
            ),
            Ok(GraphqlHttpError::Graphql(_)) => (
                "graphql",
                "pricing.admin_graphql_request_rejected",
                "Pricing admin request could not be completed",
                false,
            ),
            Err(_) => (
                "unknown",
                "pricing.admin_graphql_unknown_failure",
                "Pricing admin request could not be completed",
                true,
            ),
        };

        if technical_failure {
            tracing::error!(
                raw_error_present,
                raw_error_length,
                parsed_error_valid,
                owner = PRICING_ADMIN_GRAPHQL_OWNER,
                owner_operation = self.operation,
                correlation_id = %self.correlation_id,
                tenant_slug_present = self.tenant_slug_length.is_some(),
                tenant_slug_length = ?self.tenant_slug_length,
                tenant_id_present = self.tenant_id_length.is_some(),
                tenant_id_length = ?self.tenant_id_length,
                resource_id_present = self.resource_id_length.is_some(),
                resource_id_length = ?self.resource_id_length,
                locale_present = self.locale_length.is_some(),
                locale_length = ?self.locale_length,
                search_present = self.search_length.is_some(),
                search_length = ?self.search_length,
                status_present = self.status_length.is_some(),
                status_length = ?self.status_length,
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
                boundary = PRICING_ADMIN_GRAPHQL_BOUNDARY,
                "pricing admin GraphQL transport failed"
            );
        } else {
            tracing::warn!(
                raw_error_present,
                raw_error_length,
                parsed_error_valid,
                owner = PRICING_ADMIN_GRAPHQL_OWNER,
                owner_operation = self.operation,
                correlation_id = %self.correlation_id,
                tenant_slug_present = self.tenant_slug_length.is_some(),
                tenant_slug_length = ?self.tenant_slug_length,
                tenant_id_present = self.tenant_id_length.is_some(),
                tenant_id_length = ?self.tenant_id_length,
                resource_id_present = self.resource_id_length.is_some(),
                resource_id_length = ?self.resource_id_length,
                locale_present = self.locale_length.is_some(),
                locale_length = ?self.locale_length,
                search_present = self.search_length.is_some(),
                search_length = ?self.search_length,
                status_present = self.status_length.is_some(),
                status_length = ?self.status_length,
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
                boundary = PRICING_ADMIN_GRAPHQL_BOUNDARY,
                "pricing admin GraphQL request was rejected"
            );
        }

        ApiError::Graphql(public_message.to_string())
    }
}

fn text_length(value: Option<&str>) -> Option<usize> {
    value.map(|value| value.chars().count())
}
