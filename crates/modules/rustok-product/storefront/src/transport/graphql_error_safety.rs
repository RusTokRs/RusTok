use std::str::FromStr;

use rustok_graphql::GraphqlHttpError;
use uuid::Uuid;

use crate::catalog_controls::CatalogListInput;
use crate::core::FetchRequest;

use super::native_server_adapter::ApiError;

const PRODUCT_STOREFRONT_GRAPHQL_OWNER: &str = "rustok_product.storefront";
const PRODUCT_STOREFRONT_GRAPHQL_BOUNDARY: &str = "product_storefront_graphql_transport";

pub(super) struct GraphqlCallContext {
    owner_operation: &'static str,
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
    search_length: Option<usize>,
    category_id_length: Option<usize>,
    sort_by_present: bool,
    sort_direction_present: bool,
    attribute_filter_count: usize,
}

impl GraphqlCallContext {
    pub(super) fn fetch_products(request: &FetchRequest, controls: &CatalogListInput) -> Self {
        Self {
            owner_operation: "fetch_products",
            correlation_id: correlation_id("fetch_products"),
            tenant_slug_length: configured_tenant_slug_length(),
            selected_handle_length: text_length(request.selected_handle.as_deref()),
            locale_length: text_length(request.locale.as_deref()),
            currency_code_length: text_length(request.currency_code.as_deref()),
            region_id_length: text_length(request.region_id.as_deref()),
            price_list_id_length: text_length(request.price_list_id.as_deref()),
            channel_id_length: text_length(request.channel_id.as_deref()),
            channel_slug_length: text_length(request.channel_slug.as_deref()),
            quantity_present: request.quantity.is_some(),
            search_length: text_length(controls.search.as_deref()),
            category_id_length: text_length(controls.category_id.as_deref()),
            sort_by_present: controls.sort_by.is_some(),
            sort_direction_present: controls.sort_direction.is_some(),
            attribute_filter_count: controls.attribute_filters.len(),
        }
    }

    pub(super) fn fetch_catalog_search_options(locale: &str) -> Self {
        Self {
            owner_operation: "fetch_catalog_search_options",
            correlation_id: correlation_id("fetch_catalog_search_options"),
            tenant_slug_length: configured_tenant_slug_length(),
            selected_handle_length: None,
            locale_length: Some(locale.chars().count()),
            currency_code_length: None,
            region_id_length: None,
            price_list_id_length: None,
            channel_id_length: None,
            channel_slug_length: None,
            quantity_present: false,
            search_length: None,
            category_id_length: None,
            sort_by_present: false,
            sort_direction_present: false,
            attribute_filter_count: 0,
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
                "product.storefront_graphql_network_unavailable",
                "Product storefront is temporarily unavailable",
                true,
            ),
            Ok(GraphqlHttpError::Http(_)) => (
                "http",
                "product.storefront_graphql_http_unavailable",
                "Product storefront is temporarily unavailable",
                true,
            ),
            Ok(GraphqlHttpError::Unauthorized) => (
                "unauthorized",
                "product.storefront_graphql_authentication_required",
                "Product storefront authentication is required",
                false,
            ),
            Ok(GraphqlHttpError::Graphql(_)) => (
                "graphql",
                "product.storefront_graphql_request_rejected",
                "Product storefront request could not be completed",
                false,
            ),
            Err(_) => (
                "unknown",
                "product.storefront_graphql_unknown_failure",
                "Product storefront request could not be completed",
                true,
            ),
        };

        if technical_failure {
            tracing::error!(
                raw_error_present,
                raw_error_length,
                parsed_error_valid,
                owner = PRODUCT_STOREFRONT_GRAPHQL_OWNER,
                owner_operation = self.owner_operation,
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
                search_present = self.search_length.is_some(),
                search_length = ?self.search_length,
                category_id_present = self.category_id_length.is_some(),
                category_id_length = ?self.category_id_length,
                sort_by_present = self.sort_by_present,
                sort_direction_present = self.sort_direction_present,
                attribute_filter_count = self.attribute_filter_count,
                error_kind,
                code,
                boundary = PRODUCT_STOREFRONT_GRAPHQL_BOUNDARY,
                "product storefront GraphQL transport failed"
            );
        } else {
            tracing::warn!(
                raw_error_present,
                raw_error_length,
                parsed_error_valid,
                owner = PRODUCT_STOREFRONT_GRAPHQL_OWNER,
                owner_operation = self.owner_operation,
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
                search_present = self.search_length.is_some(),
                search_length = ?self.search_length,
                category_id_present = self.category_id_length.is_some(),
                category_id_length = ?self.category_id_length,
                sort_by_present = self.sort_by_present,
                sort_direction_present = self.sort_direction_present,
                attribute_filter_count = self.attribute_filter_count,
                error_kind,
                code,
                boundary = PRODUCT_STOREFRONT_GRAPHQL_BOUNDARY,
                "product storefront GraphQL request was rejected"
            );
        }

        ApiError::Graphql(public_message.to_string())
    }
}

fn correlation_id(owner_operation: &str) -> String {
    format!(
        "product-storefront-graphql:{owner_operation}:{}",
        Uuid::new_v4()
    )
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
