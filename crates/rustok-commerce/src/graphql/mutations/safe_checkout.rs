mod checkout_boundary {
    use ::async_graphql::{Error, ErrorExtensions};
    use ::rustok_api::{PortContext, PortError, PortErrorKind};

    use crate::CommerceError;

    const CHECKOUT_ERROR_BOUNDARY: &str = "commerce_graphql_checkout";

    struct CheckoutServiceDiagnosticError;

    impl std::fmt::Debug for CheckoutServiceDiagnosticError {
        fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            formatter.write_str("redacted")
        }
    }

    #[derive(Clone)]
    pub(crate) enum BoundaryError {
        Graphql(Error),
        Public {
            message: String,
            code: &'static str,
            retryable: bool,
        },
    }

    impl From<Error> for BoundaryError {
        fn from(error: Error) -> Self {
            Self::Graphql(error)
        }
    }

    fn public_graphql_error(message: impl Into<String>, code: &'static str, retryable: bool) -> Error {
        Error::new(message).extend_with(|_, extensions| {
            extensions.set("code", code);
            extensions.set("retryable", retryable);
        })
    }

    fn commerce_error_envelope(
        error: &CommerceError,
    ) -> (String, &'static str, bool, &'static str) {
        match error {
            CommerceError::Validation(detail) => (
                if detail.is_empty() {
                    "Shipping profile request is invalid".to_string()
                } else {
                    detail.clone()
                },
                "SHIPPING_PROFILE_REQUEST_INVALID",
                false,
                "validation",
            ),
            CommerceError::InvalidPrice(_)
            | CommerceError::InvalidOptionCombination
            | CommerceError::NoVariants => (
                "Shipping profile request is invalid".to_string(),
                "SHIPPING_PROFILE_REQUEST_INVALID",
                false,
                "validation",
            ),
            CommerceError::ShippingProfileNotFound(_) => (
                "Shipping profile was not found".to_string(),
                "SHIPPING_PROFILE_NOT_FOUND",
                false,
                "not_found",
            ),
            CommerceError::DuplicateShippingProfileSlug(_) => (
                "Shipping profile conflicts with the current state".to_string(),
                "SHIPPING_PROFILE_STATE_CONFLICT",
                false,
                "conflict",
            ),
            CommerceError::Database(_) => (
                "Shipping profile service is temporarily unavailable".to_string(),
                "SHIPPING_PROFILE_TEMPORARILY_UNAVAILABLE",
                true,
                "database",
            ),
            CommerceError::ProductNotFound(_)
            | CommerceError::VariantNotFound(_)
            | CommerceError::DuplicateHandle { .. }
            | CommerceError::DuplicateSku(_)
            | CommerceError::InsufficientInventory { .. }
            | CommerceError::CannotDeletePublished
            | CommerceError::Rich(_)
            | CommerceError::Core(_) => (
                "Shipping profile operation could not be completed safely".to_string(),
                "SHIPPING_PROFILE_OPERATION_FAILED",
                false,
                "unexpected_owner_error",
            ),
        }
    }

    fn shipping_option_port_error_envelope(
        error: &PortError,
    ) -> (String, &'static str, bool, &'static str) {
        match &error.kind {
            PortErrorKind::Validation => (
                if error.message.is_empty() {
                    "Shipping option request is invalid".to_string()
                } else {
                    error.message.clone()
                },
                "SHIPPING_OPTION_REQUEST_INVALID",
                false,
                "validation",
            ),
            PortErrorKind::NotFound if error.code == "fulfillment.shipping_option_not_found" => (
                "Shipping option was not found".to_string(),
                "SHIPPING_OPTION_NOT_FOUND",
                false,
                "not_found",
            ),
            PortErrorKind::Conflict => (
                "Shipping option operation conflicts with the current state".to_string(),
                "SHIPPING_OPTION_STATE_CONFLICT",
                false,
                "conflict",
            ),
            PortErrorKind::Unavailable | PortErrorKind::Timeout => (
                "Shipping option service is temporarily unavailable".to_string(),
                "SHIPPING_OPTION_TEMPORARILY_UNAVAILABLE",
                true,
                "temporarily_unavailable",
            ),
            PortErrorKind::NotFound
            | PortErrorKind::Forbidden
            | PortErrorKind::InvariantViolation => (
                "Shipping option operation could not be completed safely".to_string(),
                "SHIPPING_OPTION_OPERATION_FAILED",
                false,
                "unexpected_owner_error",
            ),
        }
    }

    impl From<CommerceError> for BoundaryError {
        fn from(error: CommerceError) -> Self {
            let (message, code, retryable, error_kind) = commerce_error_envelope(&error);
            let diagnostic_error = CheckoutServiceDiagnosticError;
            tracing::error!(
                error = ?diagnostic_error,
                owner = "rustok_commerce",
                error_kind,
                public_code = code,
                retryable,
                boundary = CHECKOUT_ERROR_BOUNDARY,
                "commerce GraphQL checkout shipping profile operation failed"
            );
            Self::Public {
                message,
                code,
                retryable,
            }
        }
    }

    pub(crate) fn shipping_option_port_error(
        context: &PortContext,
        owner_operation: &'static str,
        error: PortError,
    ) -> BoundaryError {
        let (message, code, retryable, error_kind) = shipping_option_port_error_envelope(&error);
        let diagnostic_error = CheckoutServiceDiagnosticError;
        tracing::error!(
            error = ?diagnostic_error,
            owner = "rustok_fulfillment.shipping_option_admin_command",
            owner_operation,
            correlation_id = %context.correlation_id,
            tenant_id_present = !context.tenant_id.is_empty(),
            actor_id_present = !context.actor.id.is_empty(),
            channel_present = context.channel.is_some(),
            locale_length = context.locale.chars().count(),
            deadline_ms = ?context.deadline_ms,
            owner_error_kind = ?error.kind,
            owner_code_length = error.code.chars().count(),
            error_kind,
            public_code = code,
            retryable,
            boundary = CHECKOUT_ERROR_BOUNDARY,
            "commerce GraphQL checkout shipping option owner command failed"
        );
        BoundaryError::Public {
            message,
            code,
            retryable,
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
                } => public_graphql_error(message, code, retryable),
            }
        }
    }
}

mod async_graphql_shim {
    pub use ::async_graphql::{Context, Error, ErrorExtensions, Object};

    pub type Result<T> = std::result::Result<T, super::checkout_boundary::BoundaryError>;
}

use self::async_graphql_shim as async_graphql;

include!("checkout.rs");
