use std::str::FromStr;

use rustok_graphql::GraphqlHttpError;

use super::native_server_adapter::ApiError;

const COMMERCE_ADMIN_GRAPHQL_CONSUMER: &str = "rustok_commerce.admin_graphql_transport";
const COMMERCE_ADMIN_GRAPHQL_BOUNDARY: &str = "commerce_admin_graphql_transport";

struct CommerceAdminGraphqlErrorFacts {
    error_payload_present: bool,
    error_payload_length: usize,
    parse_succeeded: bool,
    detail_present: bool,
    detail_length: usize,
}

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
    let error_facts = commerce_admin_graphql_error_facts(raw_error.as_str(), &parsed);
    let tenant_id_length = tenant_id.map(|value| value.chars().count());
    let tenant_uuid = tenant_id.and_then(|value| uuid::Uuid::parse_str(value).ok());
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
            error_payload_present = error_facts.error_payload_present,
            error_payload_length = error_facts.error_payload_length,
            parse_succeeded = error_facts.parse_succeeded,
            error_detail_present = error_facts.detail_present,
            error_detail_length = error_facts.detail_length,
            consumer = COMMERCE_ADMIN_GRAPHQL_CONSUMER,
            operation,
            correlation_id,
            tenant_id_present = tenant_id_length.is_some(),
            tenant_id_length,
            tenant_id_uuid_valid = tenant_uuid.is_some(),
            tenant_id_uuid_non_nil = tenant_uuid.as_ref().is_some_and(|value| !value.is_nil()),
            tenant_slug_present = tenant_slug_length.is_some(),
            tenant_slug_length,
            error_kind,
            public_code,
            boundary = COMMERCE_ADMIN_GRAPHQL_BOUNDARY,
            "commerce admin GraphQL transport failed"
        );
    } else {
        tracing::warn!(
            error_payload_present = error_facts.error_payload_present,
            error_payload_length = error_facts.error_payload_length,
            parse_succeeded = error_facts.parse_succeeded,
            error_detail_present = error_facts.detail_present,
            error_detail_length = error_facts.detail_length,
            consumer = COMMERCE_ADMIN_GRAPHQL_CONSUMER,
            operation,
            correlation_id,
            tenant_id_present = tenant_id_length.is_some(),
            tenant_id_length,
            tenant_id_uuid_valid = tenant_uuid.is_some(),
            tenant_id_uuid_non_nil = tenant_uuid.as_ref().is_some_and(|value| !value.is_nil()),
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

fn commerce_admin_graphql_error_facts(
    raw_error: &str,
    parsed: &Result<GraphqlHttpError, String>,
) -> CommerceAdminGraphqlErrorFacts {
    let detail = match parsed {
        Ok(GraphqlHttpError::Graphql(message)) | Ok(GraphqlHttpError::Http(message)) => {
            Some(message.as_str())
        }
        Ok(GraphqlHttpError::Network) | Ok(GraphqlHttpError::Unauthorized) | Err(_) => None,
    };

    CommerceAdminGraphqlErrorFacts {
        error_payload_present: !raw_error.trim().is_empty(),
        error_payload_length: raw_error.chars().count(),
        parse_succeeded: parsed.is_ok(),
        detail_present: detail.is_some_and(|value| !value.trim().is_empty()),
        detail_length: detail.map_or(0, |value| value.chars().count()),
    }
}
