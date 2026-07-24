mod cart_context_boundary {
    use ::async_graphql::{Error, ErrorExtensions};

    use crate::StoreContextError;

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

    fn public_graphql_error(message: &'static str, code: &'static str, retryable: bool) -> Error {
        Error::new(message).extend_with(|_, extensions| {
            extensions.set("code", code);
            extensions.set("retryable", retryable);
        })
    }

    fn store_context_error_envelope(
        error: &StoreContextError,
    ) -> (&'static str, &'static str, bool, &'static str) {
        match error {
            StoreContextError::TenantNotFound(_) => (
                "Store context was not found",
                "STORE_CONTEXT_NOT_FOUND",
                false,
                "tenant_not_found",
            ),
            StoreContextError::Validation(_) | StoreContextError::CurrencyRegionMismatch { .. } => {
                (
                    "Store context request is invalid",
                    "STORE_CONTEXT_REQUEST_INVALID",
                    false,
                    "validation",
                )
            }
            StoreContextError::RegionBoundary { .. } => (
                "Store context could not be resolved safely",
                "STORE_CONTEXT_RESOLUTION_FAILED",
                false,
                "region_boundary",
            ),
            StoreContextError::Database(_) => (
                "Store context is temporarily unavailable",
                "STORE_CONTEXT_TEMPORARILY_UNAVAILABLE",
                true,
                "database",
            ),
        }
    }

    impl From<StoreContextError> for BoundaryError {
        fn from(error: StoreContextError) -> Self {
            let (message, code, retryable, error_kind) = store_context_error_envelope(&error);
            tracing::error!(
                error = ?error,
                owner = "rustok_commerce.store_context",
                error_kind,
                public_code = code,
                retryable,
                operation = "resolve_store_context",
                boundary = "commerce_graphql_cart",
                "commerce GraphQL cart store context resolution failed"
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
    pub use ::async_graphql::{Context, Error, MaybeUndefined, Object};

    pub type Result<T> = std::result::Result<T, super::cart_context_boundary::BoundaryError>;
}

use self::async_graphql_shim as async_graphql;

include!("cart.rs");
