mod query_error_boundary {
    use ::async_graphql::{Error, ErrorExtensions};
    use ::rustok_fulfillment::error::FulfillmentError;
    use ::rustok_order::error::OrderError;
    use ::rustok_payment::error::PaymentError;

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

        fn public(
            message: &'static str,
            code: &'static str,
            retryable: bool,
        ) -> Self {
            Self::Public {
                message,
                code,
                retryable,
            }
        }
    }

    impl QueryGraphqlMessage for String {
        fn into_query_boundary(self) -> BoundaryError {
            tracing::error!(
                error_message = %self,
                public_code = "COMMERCE_QUERY_OPERATION_FAILED",
                retryable = false,
                boundary = "commerce_graphql_query",
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
        fn from(error: sea_orm::DbErr) -> Self {
            tracing::error!(
                error = ?error,
                owner = "sea_orm",
                error_kind = "database",
                public_code = "COMMERCE_QUERY_TEMPORARILY_UNAVAILABLE",
                retryable = true,
                boundary = "commerce_graphql_query",
                "commerce GraphQL query database operation failed"
            );
            Self::public(
                "Commerce data is temporarily unavailable",
                "COMMERCE_QUERY_TEMPORARILY_UNAVAILABLE",
                true,
            )
        }
    }

    impl From<crate::CommerceError> for BoundaryError {
        fn from(error: crate::CommerceError) -> Self {
            Self::Graphql(super::super::map_product_service_error(
                error,
                "commerce_query",
            ))
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
            tracing::error!(
                error = ?error,
                owner = "rustok_fulfillment",
                error_kind,
                public_code = code,
                retryable,
                boundary = "commerce_graphql_query",
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
            tracing::error!(
                error = ?error,
                owner = "rustok_order",
                error_kind,
                public_code = code,
                retryable,
                boundary = "commerce_graphql_query",
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
                PaymentError::InvalidTransition { .. }
                | PaymentError::ProviderRejected { .. } => (
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
            tracing::error!(
                error = ?error,
                owner = "rustok_payment",
                error_kind,
                public_code = code,
                retryable,
                boundary = "commerce_graphql_query",
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
}

pub(crate) const MODULE_SLUG: &str = super::MODULE_SLUG;
pub(crate) const PRODUCT_MODULE_SLUG: &str = super::PRODUCT_MODULE_SLUG;

pub(crate) mod types {
    pub(crate) use super::super::types::*;
}

pub(crate) fn map_product_service_error(
    error: rustok_commerce_foundation::CommerceError,
    operation: &'static str,
) -> query_error_boundary::BoundaryError {
    super::map_product_service_error(error, operation).into()
}

pub(crate) fn product_query_tenant(
    ctx: &::async_graphql::Context<'_>,
    requested_tenant_id: uuid::Uuid,
) -> Result<uuid::Uuid, query_error_boundary::BoundaryError> {
    super::product_query_tenant(ctx, requested_tenant_id).map_err(Into::into)
}

pub(crate) fn require_commerce_permission(
    ctx: &::async_graphql::Context<'_>,
    permissions: &[::rustok_api::Permission],
    message: &str,
) -> Result<::rustok_api::AuthContext, query_error_boundary::BoundaryError> {
    super::require_commerce_permission(ctx, permissions, message).map_err(Into::into)
}

pub(crate) async fn require_storefront_channel_enabled(
    ctx: &::async_graphql::Context<'_>,
) -> Result<(), query_error_boundary::BoundaryError> {
    super::require_storefront_channel_enabled(ctx)
        .await
        .map_err(Into::into)
}

mod source {
    mod async_graphql_shim {
        pub use ::async_graphql::{Context, Object};

        pub type Error = super::super::query_error_boundary::BoundaryError;
        pub type FieldError = super::super::query_error_boundary::BoundaryError;
        pub type Result<T> =
            std::result::Result<T, super::super::query_error_boundary::BoundaryError>;
    }

    use self::async_graphql_shim as async_graphql;

    mod rustok_api_shim {
        pub use ::rustok_api::{
            AuthContext, Permission, PortActor, PortContext, PortErrorKind, RequestContext,
            TenantContext, locale_tags_match,
        };

        pub mod graphql {
            use super::super::super::query_error_boundary::BoundaryError;

            pub trait GraphQLError {
                fn unauthenticated() -> BoundaryError;
                fn permission_denied(message: &str) -> BoundaryError;
                fn internal_error(message: &str) -> BoundaryError;
                fn bad_user_input(message: &str) -> BoundaryError;
                fn not_found(message: &str) -> BoundaryError;
            }

            impl GraphQLError for BoundaryError {
                fn unauthenticated() -> BoundaryError {
                    BoundaryError::from(
                        <::async_graphql::FieldError as ::rustok_api::graphql::GraphQLError>::unauthenticated(),
                    )
                }

                fn permission_denied(message: &str) -> BoundaryError {
                    BoundaryError::from(
                        <::async_graphql::FieldError as ::rustok_api::graphql::GraphQLError>::permission_denied(message),
                    )
                }

                fn internal_error(message: &str) -> BoundaryError {
                    BoundaryError::from(
                        <::async_graphql::FieldError as ::rustok_api::graphql::GraphQLError>::internal_error(message),
                    )
                }

                fn bad_user_input(message: &str) -> BoundaryError {
                    BoundaryError::from(
                        <::async_graphql::FieldError as ::rustok_api::graphql::GraphQLError>::bad_user_input(message),
                    )
                }

                fn not_found(message: &str) -> BoundaryError {
                    BoundaryError::from(
                        <::async_graphql::FieldError as ::rustok_api::graphql::GraphQLError>::not_found(message),
                    )
                }
            }

            pub async fn require_module_enabled(
                ctx: &::async_graphql::Context<'_>,
                module_slug: &str,
            ) -> Result<(), BoundaryError> {
                ::rustok_api::graphql::require_module_enabled(ctx, module_slug)
                    .await
                    .map_err(Into::into)
            }
        }
    }

    use self::rustok_api_shim as rustok_api;

    include!("query.rs");
}

pub use source::CommerceQuery;
