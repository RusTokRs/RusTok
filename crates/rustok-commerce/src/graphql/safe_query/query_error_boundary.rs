use ::async_graphql::{Error, ErrorExtensions};
use ::rustok_fulfillment::error::FulfillmentError;
use ::rustok_order::error::OrderError;
use ::rustok_payment::error::PaymentError;

const QUERY_ERROR_BOUNDARY: &str = "commerce_graphql_query";

struct QueryDiagnosticError;

impl std::fmt::Debug for QueryDiagnosticError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("redacted")
    }
}

fn text_presence_shape(value: &str) -> &'static str {
    if value.is_empty() { "empty" } else { "present" }
}

#[derive(Clone, Debug)]
pub(crate) enum BoundaryError {
    Graphql(Error),
    Public {
        message: &'static str,
        code: &'static str,
        retryable: bool,
    },
}

pub(crate) trait QueryGraphqlMessage {
    fn into_query_boundary(self) -> BoundaryError;
}

impl BoundaryError {
    pub(crate) fn new<M>(message: M) -> Self
    where
        M: QueryGraphqlMessage,
    {
        message.into_query_boundary()
    }

    fn public(message: &'static str, code: &'static str, retryable: bool) -> Self {
        Self::Public {
            message,
            code,
            retryable,
        }
    }
}

impl QueryGraphqlMessage for String {
    fn into_query_boundary(self) -> BoundaryError {
        let message_presence = text_presence_shape(&self);
        let message_len = self.len();
        let error = QueryDiagnosticError;
        tracing::error!(
            error = ?error,
            source_owner = "commerce_graphql_query.dynamic_message",
            error_kind = "dynamic_message",
            message_presence,
            message_len,
            public_code = "COMMERCE_QUERY_OPERATION_FAILED",
            retryable = false,
            boundary = QUERY_ERROR_BOUNDARY,
            "commerce GraphQL query dynamic error was redacted"
        );
        BoundaryError::public(
            "Commerce query could not be completed safely",
            "COMMERCE_QUERY_OPERATION_FAILED",
            false,
        )
    }
}

impl QueryGraphqlMessage for &str {
    fn into_query_boundary(self) -> BoundaryError {
        BoundaryError::Graphql(Error::new(self))
    }
}

impl QueryGraphqlMessage for BoundaryError {
    fn into_query_boundary(self) -> BoundaryError {
        self
    }
}

impl From<Error> for BoundaryError {
    fn from(error: Error) -> Self {
        Self::Graphql(error)
    }
}

impl From<String> for BoundaryError {
    fn from(message: String) -> Self {
        message.into_query_boundary()
    }
}

impl From<sea_orm::DbErr> for BoundaryError {
    fn from(_error: sea_orm::DbErr) -> Self {
        let error = QueryDiagnosticError;
        tracing::error!(
            error = ?error,
            owner = "sea_orm",
            error_kind = "database",
            public_code = "COMMERCE_QUERY_TEMPORARILY_UNAVAILABLE",
            retryable = true,
            boundary = QUERY_ERROR_BOUNDARY,
            "commerce GraphQL query database operation failed"
        );
        Self::public(
            "Commerce data is temporarily unavailable",
            "COMMERCE_QUERY_TEMPORARILY_UNAVAILABLE",
            true,
        )
    }
}

impl From<rustok_product::CommerceError> for BoundaryError {
    fn from(error: rustok_product::CommerceError) -> Self {
        Self::Graphql(super::super::map_product_service_error(
            error,
            "commerce_query",
        ))
    }
}

impl From<crate::CommerceError> for BoundaryError {
    fn from(error: crate::CommerceError) -> Self {
        let (message, code, retryable, error_kind) = match &error {
            crate::CommerceError::Database(_) => (
                "Commerce data is temporarily unavailable",
                "COMMERCE_QUERY_TEMPORARILY_UNAVAILABLE",
                true,
                "database",
            ),
            _ => (
                "Commerce query could not be completed safely",
                "COMMERCE_QUERY_OPERATION_FAILED",
                false,
                "commerce",
            ),
        };
        let error = QueryDiagnosticError;
        tracing::error!(
            error = ?error,
            owner = "rustok_commerce",
            error_kind,
            public_code = code,
            retryable,
            boundary = QUERY_ERROR_BOUNDARY,
            "commerce GraphQL owner operation failed"
        );
        Self::public(message, code, retryable)
    }
}

impl From<FulfillmentError> for BoundaryError {
    fn from(error: FulfillmentError) -> Self {
        let (message, code, retryable, error_kind) = match &error {
            FulfillmentError::Validation(_) => (
                "Fulfillment query is invalid",
                "FULFILLMENT_REQUEST_INVALID",
                false,
                "validation",
            ),
            FulfillmentError::ShippingOptionNotFound(_)
            | FulfillmentError::FulfillmentNotFound(_) => (
                "Fulfillment resource was not found",
                "FULFILLMENT_RESOURCE_NOT_FOUND",
                false,
                "not_found",
            ),
            FulfillmentError::InvalidTransition { .. } => (
                "Fulfillment state conflicts with this query",
                "FULFILLMENT_STATE_CONFLICT",
                false,
                "invalid_transition",
            ),
            FulfillmentError::Database(_) => (
                "Fulfillment data is temporarily unavailable",
                "FULFILLMENT_TEMPORARILY_UNAVAILABLE",
                true,
                "database",
            ),
        };
        let error = QueryDiagnosticError;
        tracing::error!(
            error = ?error,
            owner = "rustok_fulfillment",
            error_kind,
            public_code = code,
            retryable,
            boundary = QUERY_ERROR_BOUNDARY,
            "commerce GraphQL fulfillment query failed"
        );
        Self::public(message, code, retryable)
    }
}

impl From<OrderError> for BoundaryError {
    fn from(error: OrderError) -> Self {
        let (message, code, retryable, error_kind) = match &error {
            OrderError::Validation(_) => (
                "Order query is invalid",
                "ORDER_REQUEST_INVALID",
                false,
                "validation",
            ),
            OrderError::OrderNotFound(_)
            | OrderError::OrderReturnNotFound(_)
            | OrderError::OrderChangeNotFound(_) => (
                "Order resource was not found",
                "ORDER_RESOURCE_NOT_FOUND",
                false,
                "not_found",
            ),
            OrderError::InvalidTransition { .. } => (
                "Order state conflicts with this query",
                "ORDER_STATE_CONFLICT",
                false,
                "invalid_transition",
            ),
            OrderError::Database(_) => (
                "Order data is temporarily unavailable",
                "ORDER_TEMPORARILY_UNAVAILABLE",
                true,
                "database",
            ),
            OrderError::Core(_) => (
                "Order query could not be completed safely",
                "ORDER_OPERATION_FAILED",
                false,
                "core",
            ),
        };
        let error = QueryDiagnosticError;
        tracing::error!(
            error = ?error,
            owner = "rustok_order",
            error_kind,
            public_code = code,
            retryable,
            boundary = QUERY_ERROR_BOUNDARY,
            "commerce GraphQL order query failed"
        );
        Self::public(message, code, retryable)
    }
}

impl From<PaymentError> for BoundaryError {
    fn from(error: PaymentError) -> Self {
        let (message, code, retryable, error_kind) = match &error {
            PaymentError::Validation(_) => (
                "Payment query is invalid",
                "PAYMENT_REQUEST_INVALID",
                false,
                "validation",
            ),
            PaymentError::PaymentCollectionNotFound(_)
            | PaymentError::PaymentNotFound(_)
            | PaymentError::RefundNotFound(_) => (
                "Payment resource was not found",
                "PAYMENT_RESOURCE_NOT_FOUND",
                false,
                "not_found",
            ),
            PaymentError::InvalidTransition { .. } | PaymentError::ProviderRejected { .. } => (
                "Payment state conflicts with this query",
                "PAYMENT_STATE_CONFLICT",
                false,
                "state_conflict",
            ),
            PaymentError::ProviderUnavailable { .. } | PaymentError::Database(_) => (
                "Payment data is temporarily unavailable",
                "PAYMENT_TEMPORARILY_UNAVAILABLE",
                true,
                "temporarily_unavailable",
            ),
            PaymentError::ProviderInvalidResponse { .. }
            | PaymentError::ProviderOutcomeUnknown { .. } => (
                "Payment state requires reconciliation",
                "PAYMENT_RECONCILIATION_REQUIRED",
                false,
                "reconciliation_required",
            ),
            PaymentError::ProviderConfiguration { .. } => (
                "Payment provider configuration is invalid",
                "PAYMENT_CONFIGURATION_ERROR",
                false,
                "configuration",
            ),
        };
        let error = QueryDiagnosticError;
        tracing::error!(
            error = ?error,
            owner = "rustok_payment",
            error_kind,
            public_code = code,
            retryable,
            boundary = QUERY_ERROR_BOUNDARY,
            "commerce GraphQL payment query failed"
        );
        Self::public(message, code, retryable)
    }
}

impl From<BoundaryError> for Error {
    fn from(error: BoundaryError) -> Self {
        match error {
            BoundaryError::Graphql(error) => error,
            BoundaryError::Public {
                message,
                code,
                retryable,
            } => Error::new(message).extend_with(|_, extensions| {
                extensions.set("code", code);
                extensions.set("retryable", retryable);
            }),
        }
    }
}
