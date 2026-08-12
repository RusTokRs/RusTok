use std::sync::Arc;

use ::rustok_api::{PortContext, PortError, PortErrorKind};
pub(crate) use ::rustok_customer::CustomerUserProjectionRequest;
use ::rustok_customer::{CustomerReadPort, CustomerResponse};
use ::sea_orm::DatabaseConnection;

use super::super::query_error_boundary::{BoundaryError, QueryGraphqlMessage};

const GRAPHQL_QUERY_CUSTOMER_BOUNDARY: &str = "commerce_graphql_query_customer";
const CUSTOMER_BY_USER_NOT_FOUND_CODE: &str = "customer.customer_by_user_not_found";
const CUSTOMER_OTHER_CODE: &str = "customer.query_failure";

struct CustomerQueryDiagnosticError;

impl std::fmt::Debug for CustomerQueryDiagnosticError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("redacted")
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct CustomerQueryCode {
    identity_missing: bool,
}

impl CustomerQueryCode {
    pub(crate) fn as_str(&self) -> &'static str {
        if self.identity_missing {
            CUSTOMER_BY_USER_NOT_FOUND_CODE
        } else {
            CUSTOMER_OTHER_CODE
        }
    }
}

impl PartialEq<&str> for CustomerQueryCode {
    fn eq(&self, other: &&str) -> bool {
        self.as_str() == *other
    }
}

pub(crate) struct CustomerGraphqlMessage {
    error: PortError,
}

impl QueryGraphqlMessage for CustomerGraphqlMessage {
    fn into_query_boundary(self) -> BoundaryError {
        let (message, code, retryable, error_kind, technical) = match &self.error.kind {
            PortErrorKind::Validation => (
                "Customer query is invalid",
                "CUSTOMER_REQUEST_INVALID",
                false,
                "validation",
                false,
            ),
            PortErrorKind::NotFound => (
                "Customer data was not found",
                "CUSTOMER_RESOURCE_NOT_FOUND",
                false,
                "not_found",
                false,
            ),
            PortErrorKind::Conflict => (
                "Customer state conflicts with this query",
                "CUSTOMER_STATE_CONFLICT",
                false,
                "conflict",
                false,
            ),
            PortErrorKind::Forbidden => (
                "Customer query is not permitted",
                "CUSTOMER_ACCESS_DENIED",
                false,
                "forbidden",
                false,
            ),
            PortErrorKind::Unavailable | PortErrorKind::Timeout => (
                "Customer data is temporarily unavailable",
                "CUSTOMER_TEMPORARILY_UNAVAILABLE",
                true,
                "unavailable",
                true,
            ),
            PortErrorKind::InvariantViolation => (
                "Customer query could not be completed safely",
                "CUSTOMER_OPERATION_FAILED",
                false,
                "invariant",
                true,
            ),
        };
        let owner_message_present = !self.error.message.is_empty();
        let owner_message_length = self.error.message.chars().count();
        let diagnostic_error = CustomerQueryDiagnosticError;
        if technical {
            tracing::error!(
                error = ?diagnostic_error,
                owner = "rustok_customer",
                error_kind,
                owner_code = %self.error.code,
                owner_message_present,
                owner_message_length,
                owner_retryable = self.error.retryable,
                public_code = code,
                retryable,
                boundary = GRAPHQL_QUERY_CUSTOMER_BOUNDARY,
                "commerce GraphQL customer query failed"
            );
        } else {
            tracing::warn!(
                error = ?diagnostic_error,
                owner = "rustok_customer",
                error_kind,
                owner_code = %self.error.code,
                owner_message_present,
                owner_message_length,
                owner_retryable = self.error.retryable,
                public_code = code,
                retryable,
                boundary = GRAPHQL_QUERY_CUSTOMER_BOUNDARY,
                "commerce GraphQL customer query was rejected"
            );
        }
        BoundaryError::Public {
            message,
            code,
            retryable,
        }
    }
}

pub(crate) struct CustomerQueryPortError {
    pub(crate) code: CustomerQueryCode,
    pub(crate) message: CustomerGraphqlMessage,
}

impl From<PortError> for CustomerQueryPortError {
    fn from(error: PortError) -> Self {
        let identity_missing = matches!(&error.kind, PortErrorKind::NotFound);
        Self {
            code: CustomerQueryCode { identity_missing },
            message: CustomerGraphqlMessage { error },
        }
    }
}

/// Compatibility facade for the unchanged Commerce query source.
///
/// The legacy resolver may inspect `code` only to retain customer-identity
/// absence semantics. The shim derives that decision from `PortErrorKind`, while
/// every remaining failure keeps the complete typed error for the transport mapper.
pub(crate) struct CustomerQueryReadPort {
    inner: Arc<dyn CustomerReadPort>,
}

pub(crate) fn in_process_customer_read_port(db: DatabaseConnection) -> CustomerQueryReadPort {
    CustomerQueryReadPort {
        inner: ::rustok_customer::in_process_customer_read_port(db),
    }
}

impl CustomerQueryReadPort {
    pub(crate) async fn read_customer_projection_by_user(
        &self,
        context: PortContext,
        request: CustomerUserProjectionRequest,
    ) -> Result<CustomerResponse, CustomerQueryPortError> {
        self.inner
            .read_customer_projection_by_user(context, request)
            .await
            .map_err(Into::into)
    }
}
