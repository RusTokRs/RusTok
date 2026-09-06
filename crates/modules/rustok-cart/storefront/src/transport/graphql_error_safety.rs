use std::str::FromStr;

use rustok_graphql::GraphqlHttpError;
use uuid::Uuid;

use crate::core::{
    CartFetchRequest, CartLineItemDecrementRequest, CartLineItemMutationRequest,
    CartLineItemQuantityCommand,
};

use super::native_server_adapter::ApiError;

const CART_STOREFRONT_GRAPHQL_OWNER: &str = "rustok_cart.storefront";
const CART_STOREFRONT_GRAPHQL_BOUNDARY: &str = "cart_storefront_graphql_transport";

pub(super) struct GraphqlCallContext {
    owner_operation: &'static str,
    correlation_id: String,
    tenant_slug_length: Option<usize>,
    selected_cart_id_length: Option<usize>,
    locale_length: Option<usize>,
    cart_id_length: Option<usize>,
    line_item_id_length: Option<usize>,
    command_kind: Option<&'static str>,
}

impl GraphqlCallContext {
    pub(super) fn fetch_cart(request: &CartFetchRequest) -> Self {
        Self::new(
            "fetch_cart",
            request
                .selected_cart_id
                .as_deref()
                .map(|value| value.chars().count()),
            request.locale.as_deref().map(|value| value.chars().count()),
            None,
            None,
            None,
        )
    }

    pub(super) fn decrement_line_item(request: &CartLineItemDecrementRequest) -> Self {
        let command_kind = match request.command {
            CartLineItemQuantityCommand::Remove => "remove",
            CartLineItemQuantityCommand::Update { .. } => "update",
        };
        Self::new(
            "decrement_line_item",
            None,
            None,
            Some(request.cart_id.chars().count()),
            Some(request.line_item_id.chars().count()),
            Some(command_kind),
        )
    }

    pub(super) fn remove_line_item(request: &CartLineItemMutationRequest) -> Self {
        Self::new(
            "remove_line_item",
            None,
            None,
            Some(request.cart_id.chars().count()),
            Some(request.line_item_id.chars().count()),
            Some("remove"),
        )
    }

    fn new(
        owner_operation: &'static str,
        selected_cart_id_length: Option<usize>,
        locale_length: Option<usize>,
        cart_id_length: Option<usize>,
        line_item_id_length: Option<usize>,
        command_kind: Option<&'static str>,
    ) -> Self {
        Self {
            owner_operation,
            correlation_id: format!(
                "cart-storefront-graphql:{owner_operation}:{}",
                Uuid::new_v4()
            ),
            tenant_slug_length: configured_tenant_slug_length(),
            selected_cart_id_length,
            locale_length,
            cart_id_length,
            line_item_id_length,
            command_kind,
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
                "cart.storefront_graphql_network_unavailable",
                "Cart storefront is temporarily unavailable",
                true,
            ),
            Ok(GraphqlHttpError::Http(_)) => (
                "http",
                "cart.storefront_graphql_http_unavailable",
                "Cart storefront is temporarily unavailable",
                true,
            ),
            Ok(GraphqlHttpError::Unauthorized) => (
                "unauthorized",
                "cart.storefront_graphql_authentication_required",
                "Cart authentication is required",
                false,
            ),
            Ok(GraphqlHttpError::Graphql(_)) => (
                "graphql",
                "cart.storefront_graphql_request_rejected",
                "Cart request could not be completed",
                false,
            ),
            Err(_) => (
                "unknown",
                "cart.storefront_graphql_unknown_failure",
                "Cart request could not be completed",
                true,
            ),
        };

        if technical_failure {
            tracing::error!(
                raw_error_present,
                raw_error_length,
                parsed_error_valid,
                owner = CART_STOREFRONT_GRAPHQL_OWNER,
                owner_operation = self.owner_operation,
                correlation_id = %self.correlation_id,
                tenant_slug_configured = self.tenant_slug_length.is_some(),
                tenant_slug_length = ?self.tenant_slug_length,
                selected_cart_id_present = self.selected_cart_id_length.is_some(),
                selected_cart_id_length = ?self.selected_cart_id_length,
                locale_present = self.locale_length.is_some(),
                locale_length = ?self.locale_length,
                cart_id_length = ?self.cart_id_length,
                line_item_id_length = ?self.line_item_id_length,
                command_kind = ?self.command_kind,
                error_kind,
                code,
                boundary = CART_STOREFRONT_GRAPHQL_BOUNDARY,
                "cart storefront GraphQL transport failed"
            );
        } else {
            tracing::warn!(
                raw_error_present,
                raw_error_length,
                parsed_error_valid,
                owner = CART_STOREFRONT_GRAPHQL_OWNER,
                owner_operation = self.owner_operation,
                correlation_id = %self.correlation_id,
                tenant_slug_configured = self.tenant_slug_length.is_some(),
                tenant_slug_length = ?self.tenant_slug_length,
                selected_cart_id_present = self.selected_cart_id_length.is_some(),
                selected_cart_id_length = ?self.selected_cart_id_length,
                locale_present = self.locale_length.is_some(),
                locale_length = ?self.locale_length,
                cart_id_length = ?self.cart_id_length,
                line_item_id_length = ?self.line_item_id_length,
                command_kind = ?self.command_kind,
                error_kind,
                code,
                boundary = CART_STOREFRONT_GRAPHQL_BOUNDARY,
                "cart storefront GraphQL request was rejected"
            );
        }

        ApiError::Graphql(public_message.to_string())
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
