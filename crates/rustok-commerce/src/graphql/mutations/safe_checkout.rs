mod checkout_boundary {
    use ::async_graphql::{Error, ErrorExtensions};
    use ::rustok_fulfillment::error::FulfillmentError;

    use crate::CommerceError;

    #[derive(Clone)]
    pub(crate) enum BoundaryError {
        Graphql(Error),
        Public {
            message: &'static str,
            code: &'static str,
            retryable: bool,
        },
    }

    impl From<Error> for BoundaryError {
        fn from(error: Error) -> Self {
            Self::Graphql(error)
        }
    }

    fn public_graphql_error(
        message: &'static str,
        code: &'static str,
        retryable: bool,
    ) -> Error {
        Error::new(message).extend_with(|_, extensions| {
            extensions.set("code", code);
            extensions.set("retryable", retryable);
        })
    }

    fn commerce_error_envelope(
        error: &CommerceError,
    ) -> (&'static str, &'static str, bool, &'static str) {
        match error {
            CommerceError::Validation(_)
            | CommerceError::InvalidPrice(_)
            | CommerceError::InvalidOptionCombination
            | CommerceError::NoVariants => (
                "Shipping profile request is invalid",
                "SHIPPING_PROFILE_REQUEST_INVALID",
                false,
                "validation",
            ),
            CommerceError::ShippingProfileNotFound(_) => (
                "Shipping profile was not found",
                "SHIPPING_PROFILE_NOT_FOUND",
                false,
                "not_found",
            ),
            CommerceError::DuplicateShippingProfileSlug(_) => (
                "Shipping profile conflicts with the current state",
                "SHIPPING_PROFILE_STATE_CONFLICT",
                false,
                "conflict",
            ),
            CommerceError::Database(_) => (
                "Shipping profile service is temporarily unavailable",
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
                "Shipping profile operation could not be completed safely",
                "SHIPPING_PROFILE_OPERATION_FAILED",
                false,
                "unexpected_owner_error",
            ),
        }
    }

    fn fulfillment_error_envelope(
        error: &FulfillmentError,
    ) -> (&'static str, &'static str, bool, &'static str) {
        match error {
            FulfillmentError::Validation(_) => (
                "Shipping option request is invalid",
                "SHIPPING_OPTION_REQUEST_INVALID",
                false,
                "validation",
            ),
            FulfillmentError::ShippingOptionNotFound(_) => (
                "Shipping option was not found",
                "SHIPPING_OPTION_NOT_FOUND",
                false,
                "not_found",
            ),
            FulfillmentError::InvalidTransition { .. } => (
                "Shipping option operation conflicts with the current state",
                "SHIPPING_OPTION_STATE_CONFLICT",
                false,
                "conflict",
            ),
            FulfillmentError::Database(_) => (
                "Shipping option service is temporarily unavailable",
                "SHIPPING_OPTION_TEMPORARILY_UNAVAILABLE",
                true,
                "database",
            ),
            FulfillmentError::FulfillmentNotFound(_) => (
                "Shipping option operation could not be completed safely",
                "SHIPPING_OPTION_OPERATION_FAILED",
                false,
                "unexpected_owner_error",
            ),
        }
    }

    impl From<CommerceError> for BoundaryError {
        fn from(error: CommerceError) -> Self {
            let (message, code, retryable, error_kind) = commerce_error_envelope(&error);
            tracing::error!(
                error = ?error,
                owner = "rustok_commerce",
                error_kind,
                public_code = code,
                retryable,
                boundary = "commerce_graphql_checkout",
                "commerce GraphQL checkout shipping profile operation failed"
            );
            Self::Public {
                message,
                code,
                retryable,
            }
        }
    }

    impl From<FulfillmentError> for BoundaryError {
        fn from(error: FulfillmentError) -> Self {
            let (message, code, retryable, error_kind) = fulfillment_error_envelope(&error);
            tracing::error!(
                error = ?error,
                owner = "rustok_fulfillment",
                error_kind,
                public_code = code,
                retryable,
                boundary = "commerce_graphql_checkout",
                "commerce GraphQL checkout shipping option operation failed"
            );
            Self::Public {
                message,
                code,
                retryable,
            }
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
