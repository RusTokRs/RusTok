use std::str::FromStr;

use rustok_graphql::GraphqlHttpError;

use super::native_server_adapter::ApiError;

const COMMERCE_ADMIN_GRAPHQL_CONSUMER: &str = "rustok_commerce.admin_graphql_transport";
const COMMERCE_ADMIN_GRAPHQL_BOUNDARY: &str = "commerce_admin_graphql_transport";

pub(super) fn graphql_correlation_id(operation: &'static str) -> String {
    format!(
        "commerce-admin-graphql:{operation}:{}",
        uuid::Uuid::new_v4()
    )
}

pub(super) fn map_graphql_error(
    error: ApiError,
    operation: &'static str,
    correlation_id: &str,
    tenant_id: Option<&str>,
    tenant_slug_length: Option<usize>,
) -> ApiError {
    let ApiError::Graphql(raw_error) = error else {
        return error;
    };

    let parsed = GraphqlHttpError::from_str(raw_error.as_str());
    let (public_code, public_message, error_kind, severe) = match &parsed {
        Ok(GraphqlHttpError::Unauthorized) => (
            "commerce.admin_graphql_authentication_required",
            "Commerce admin authentication is required",
            "unauthorized",
            false,
        ),
        Ok(GraphqlHttpError::Network) => (
            "commerce.admin_graphql_network_unavailable",
            "Commerce admin service is temporarily unavailable",
            "network",
            true,
        ),
        Ok(GraphqlHttpError::Http(_)) => (
            "commerce.admin_graphql_http_unavailable",
            "Commerce admin service is temporarily unavailable",
            "http",
            true,
        ),
        Ok(GraphqlHttpError::Graphql(_)) => (
            "commerce.admin_graphql_request_rejected",
            "Commerce admin request could not be completed",
            "graphql",
            false,
        ),
        Err(_) => (
            "commerce.admin_graphql_unknown_failure",
            "Commerce admin request could not be completed",
            "unknown",
            true,
        ),
    };

    if severe {
        tracing::error!(
            error = %raw_error,
            parsed_error = ?parsed,
            consumer = COMMERCE_ADMIN_GRAPHQL_CONSUMER,
            operation,
            correlation_id,
            tenant_id,
            tenant_slug_present = tenant_slug_length.is_some(),
            tenant_slug_length,
            error_kind,
            public_code,
            boundary = COMMERCE_ADMIN_GRAPHQL_BOUNDARY,
            "commerce admin GraphQL transport failed"
        );
    } else {
        tracing::warn!(
            error = %raw_error,
            parsed_error = ?parsed,
            consumer = COMMERCE_ADMIN_GRAPHQL_CONSUMER,
            operation,
            correlation_id,
            tenant_id,
            tenant_slug_present = tenant_slug_length.is_some(),
            tenant_slug_length,
            error_kind,
            public_code,
            boundary = COMMERCE_ADMIN_GRAPHQL_BOUNDARY,
            "commerce admin GraphQL request was rejected"
        );
    }

    ApiError::Graphql(public_message.to_string())
}
