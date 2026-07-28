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
            let (message, code, retryable, error_kind) = match error {
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
            tracing::error!(
                error = ?error,
                error_kind,
                public_code = code,
                retryable,
                boundary = "commerce_graphql_query",
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
    error: rustok_product::CommerceError,
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

            #[allow(dead_code)]
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

    mod rustok_fulfillment_shim {
        use std::sync::Arc;

        use ::rustok_api::{PortActor, PortContext, PortError, PortErrorKind};
        use ::rustok_fulfillment::{
            FulfillmentError, FulfillmentResponse, FulfillmentResult,
            ListAllShippingOptionProjectionsRequest, ListFulfillmentsInput,
            ListShippingOptionProjectionsRequest, ReadShippingOptionProjectionRequest,
            ShippingOptionAdminReadPort, ShippingOptionReadPort, ShippingOptionResponse,
            in_process_shipping_option_admin_read_port, in_process_shipping_option_read_port,
        };
        use ::sea_orm::{DatabaseConnection, DbErr};
        use ::uuid::Uuid;

        use super::super::query_error_boundary::BoundaryError;

        pub mod error {
            pub use ::rustok_fulfillment::error::*;
        }

        const GRAPHQL_QUERY_FULFILLMENT_BOUNDARY: &str =
            "commerce_graphql_query_fulfillment_facade";

        pub(crate) struct ShippingOptionAdminQueryError(BoundaryError);

        impl ShippingOptionAdminQueryError {
            #[allow(clippy::inherent_to_string, clippy::wrong_self_convention)]
            pub(crate) fn to_string(self) -> BoundaryError {
                self.0
            }
        }

        pub struct FulfillmentService {
            inner: ::rustok_fulfillment::FulfillmentService,
            shipping_option_reads: Arc<dyn ShippingOptionReadPort>,
            shipping_option_admin_reads: Arc<dyn ShippingOptionAdminReadPort>,
        }

        impl FulfillmentService {
            pub fn new(db: DatabaseConnection) -> Self {
                Self {
                    inner: ::rustok_fulfillment::FulfillmentService::new(db.clone()),
                    shipping_option_reads: in_process_shipping_option_read_port(db.clone()),
                    shipping_option_admin_reads: in_process_shipping_option_admin_read_port(db),
                }
            }

            pub async fn get_shipping_option(
                &self,
                tenant_id: Uuid,
                id: Uuid,
                requested_locale: Option<&str>,
                tenant_default_locale: Option<&str>,
            ) -> FulfillmentResult<ShippingOptionResponse> {
                let context = shipping_option_query_context(
                    tenant_id,
                    "shipping_option",
                    Some(id),
                    requested_locale,
                    tenant_default_locale,
                );
                self.shipping_option_reads
                    .read_shipping_option_projection(
                        context.clone(),
                        ReadShippingOptionProjectionRequest {
                            shipping_option_id: id,
                            requested_locale: requested_locale.map(str::to_owned),
                            tenant_default_locale: tenant_default_locale.map(str::to_owned),
                        },
                    )
                    .await
                    .map_err(|error| {
                        map_shipping_option_lookup_port_error(
                            error,
                            &context,
                            "shipping_option",
                            "read_shipping_option_projection",
                            id,
                            requested_locale,
                            tenant_default_locale,
                        )
                    })
            }

            pub async fn list_shipping_options(
                &self,
                tenant_id: Uuid,
                requested_locale: Option<&str>,
                tenant_default_locale: Option<&str>,
            ) -> Result<Vec<ShippingOptionResponse>, BoundaryError> {
                let context = shipping_option_query_context(
                    tenant_id,
                    "storefront_shipping_options",
                    None,
                    requested_locale,
                    tenant_default_locale,
                );
                self.shipping_option_reads
                    .list_shipping_option_projections(
                        context.clone(),
                        ListShippingOptionProjectionsRequest {
                            requested_locale: requested_locale.map(str::to_owned),
                            tenant_default_locale: tenant_default_locale.map(str::to_owned),
                        },
                    )
                    .await
                    .map_err(|error| {
                        map_shipping_option_port_error(
                            error,
                            &context,
                            "storefront_shipping_options",
                            "list_shipping_option_projections",
                            None,
                            requested_locale,
                            tenant_default_locale,
                        )
                    })
            }

            pub async fn list_all_shipping_options(
                &self,
                tenant_id: Uuid,
                requested_locale: Option<&str>,
                tenant_default_locale: Option<&str>,
            ) -> Result<Vec<ShippingOptionResponse>, ShippingOptionAdminQueryError> {
                let context = shipping_option_query_context(
                    tenant_id,
                    "shipping_options",
                    None,
                    requested_locale,
                    tenant_default_locale,
                );
                self.shipping_option_admin_reads
                    .list_all_shipping_option_projections(
                        context.clone(),
                        ListAllShippingOptionProjectionsRequest {
                            requested_locale: requested_locale.map(str::to_owned),
                            tenant_default_locale: tenant_default_locale.map(str::to_owned),
                        },
                    )
                    .await
                    .map_err(|error| {
                        ShippingOptionAdminQueryError(map_shipping_option_port_error(
                            error,
                            &context,
                            "shipping_options",
                            "list_all_shipping_option_projections",
                            None,
                            requested_locale,
                            tenant_default_locale,
                        ))
                    })
            }

            pub async fn get_fulfillment(
                &self,
                tenant_id: Uuid,
                id: Uuid,
            ) -> FulfillmentResult<FulfillmentResponse> {
                self.inner
                    .get_fulfillment(tenant_id, id)
                    .await
                    .map_err(|error| {
                        log_fulfillment_query_error(
                            &error,
                            tenant_id,
                            "fulfillment",
                            "get_fulfillment",
                            None,
                            None,
                            None,
                        );
                        error
                    })
            }

            pub async fn list_fulfillments(
                &self,
                tenant_id: Uuid,
                input: ListFulfillmentsInput,
            ) -> FulfillmentResult<(Vec<FulfillmentResponse>, u64)> {
                self.inner
                    .list_fulfillments(tenant_id, input)
                    .await
                    .map_err(|error| {
                        log_fulfillment_query_error(
                            &error,
                            tenant_id,
                            "fulfillments",
                            "list_fulfillments",
                            None,
                            None,
                            None,
                        );
                        error
                    })
            }

            pub async fn find_by_order(
                &self,
                tenant_id: Uuid,
                order_id: Uuid,
            ) -> FulfillmentResult<Option<FulfillmentResponse>> {
                self.inner
                    .find_by_order(tenant_id, order_id)
                    .await
                    .map_err(|error| {
                        log_fulfillment_query_error(
                            &error,
                            tenant_id,
                            "order",
                            "find_by_order",
                            Some(order_id),
                            None,
                            None,
                        );
                        error
                    })
            }
        }

        fn shipping_option_query_context(
            tenant_id: Uuid,
            query_field: &'static str,
            shipping_option_id: Option<Uuid>,
            requested_locale: Option<&str>,
            tenant_default_locale: Option<&str>,
        ) -> PortContext {
            let locale = requested_locale.or(tenant_default_locale).unwrap_or("en");
            let resource = shipping_option_id
                .map(|id| id.to_string())
                .unwrap_or_else(|| tenant_id.to_string());
            PortContext::new(
                tenant_id.to_string(),
                PortActor::service("rustok-commerce.graphql-query-shipping-options"),
                locale,
                format!("graphql-fulfillment:{query_field}:{resource}"),
            )
            .with_deadline(std::time::Duration::from_secs(2))
        }

        #[allow(clippy::too_many_arguments)]
        fn map_shipping_option_lookup_port_error(
            error: PortError,
            context: &PortContext,
            query_field: &'static str,
            operation: &'static str,
            shipping_option_id: Uuid,
            requested_locale: Option<&str>,
            tenant_default_locale: Option<&str>,
        ) -> FulfillmentError {
            let error_kind = port_error_kind_name(&error.kind);
            let technical = is_technical_port_error(&error.kind);
            log_shipping_option_port_error(
                &error,
                context,
                query_field,
                operation,
                Some(shipping_option_id),
                requested_locale,
                tenant_default_locale,
                error_kind,
                if matches!(&error.kind, PortErrorKind::NotFound) {
                    "OPTIONAL_NONE"
                } else {
                    "COMMERCE_QUERY_OPERATION_FAILED"
                },
                false,
                technical,
            );

            match error.kind {
                PortErrorKind::NotFound => {
                    FulfillmentError::ShippingOptionNotFound(shipping_option_id)
                }
                PortErrorKind::Conflict => FulfillmentError::InvalidTransition {
                    from: "current".to_string(),
                    to: "query".to_string(),
                },
                PortErrorKind::Unavailable | PortErrorKind::Timeout => FulfillmentError::Database(
                    DbErr::Custom("fulfillment storage is temporarily unavailable".to_string()),
                ),
                PortErrorKind::Validation => FulfillmentError::Validation(
                    "fulfillment request is invalid".to_string(),
                ),
                PortErrorKind::Forbidden => FulfillmentError::Validation(
                    "fulfillment query is not permitted".to_string(),
                ),
                PortErrorKind::InvariantViolation => FulfillmentError::Validation(
                    "fulfillment query could not be completed safely".to_string(),
                ),
            }
        }

        #[allow(clippy::too_many_arguments)]
        fn map_shipping_option_port_error(
            error: PortError,
            context: &PortContext,
            query_field: &'static str,
            operation: &'static str,
            shipping_option_id: Option<Uuid>,
            requested_locale: Option<&str>,
            tenant_default_locale: Option<&str>,
        ) -> BoundaryError {
            let (message, code, retryable) = match &error.kind {
                PortErrorKind::Validation => (
                    "Fulfillment query is invalid",
                    "FULFILLMENT_REQUEST_INVALID",
                    false,
                ),
                PortErrorKind::NotFound => (
                    "Fulfillment resource was not found",
                    "FULFILLMENT_RESOURCE_NOT_FOUND",
                    false,
                ),
                PortErrorKind::Conflict => (
                    "Fulfillment state conflicts with this query",
                    "FULFILLMENT_STATE_CONFLICT",
                    false,
                ),
                PortErrorKind::Unavailable | PortErrorKind::Timeout => (
                    "Fulfillment data is temporarily unavailable",
                    "FULFILLMENT_TEMPORARILY_UNAVAILABLE",
                    true,
                ),
                PortErrorKind::Forbidden => (
                    "Fulfillment query is not permitted",
                    "FULFILLMENT_ACCESS_DENIED",
                    false,
                ),
                PortErrorKind::InvariantViolation => (
                    "Fulfillment query could not be completed safely",
                    "FULFILLMENT_OPERATION_FAILED",
                    false,
                ),
            };
            let error_kind = port_error_kind_name(&error.kind);
            let technical = is_technical_port_error(&error.kind);
            log_shipping_option_port_error(
                &error,
                context,
                query_field,
                operation,
                shipping_option_id,
                requested_locale,
                tenant_default_locale,
                error_kind,
                code,
                retryable,
                technical,
            );

            BoundaryError::Public {
                message,
                code,
                retryable,
            }
        }

        fn port_error_kind_name(kind: &PortErrorKind) -> &'static str {
            match kind {
                PortErrorKind::Validation => "validation",
                PortErrorKind::NotFound => "not_found",
                PortErrorKind::Conflict => "conflict",
                PortErrorKind::Forbidden => "forbidden",
                PortErrorKind::Unavailable | PortErrorKind::Timeout => "unavailable",
                PortErrorKind::InvariantViolation => "invariant",
            }
        }

        fn is_technical_port_error(kind: &PortErrorKind) -> bool {
            matches!(
                kind,
                PortErrorKind::Unavailable
                    | PortErrorKind::Timeout
                    | PortErrorKind::InvariantViolation
            )
        }

        #[allow(clippy::too_many_arguments)]
        fn log_shipping_option_port_error(
            error: &PortError,
            context: &PortContext,
            query_field: &'static str,
            operation: &'static str,
            shipping_option_id: Option<Uuid>,
            requested_locale: Option<&str>,
            tenant_default_locale: Option<&str>,
            error_kind: &'static str,
            public_code: &'static str,
            public_retryable: bool,
            technical: bool,
        ) {
            if technical {
                tracing::error!(
                    error = ?error,
                    owner = "rustok_fulfillment",
                    correlation_id = %context.correlation_id,
                    tenant_id = %context.tenant_id,
                    actor = ?context.actor,
                    context_locale_length = context.locale.len(),
                    deadline_ms = ?context.deadline_ms,
                    query_field,
                    operation,
                    shipping_option_id = ?shipping_option_id,
                    requested_locale_length = requested_locale.map(str::len),
                    tenant_default_locale_length = tenant_default_locale.map(str::len),
                    error_kind,
                    owner_code = %error.code,
                    owner_kind = ?error.kind,
                    owner_retryable = error.retryable,
                    public_code,
                    public_retryable,
                    boundary = GRAPHQL_QUERY_FULFILLMENT_BOUNDARY,
                    "commerce GraphQL query shipping-option owner read failed"
                );
            } else {
                tracing::warn!(
                    owner = "rustok_fulfillment",
                    correlation_id = %context.correlation_id,
                    tenant_id = %context.tenant_id,
                    actor = ?context.actor,
                    context_locale_length = context.locale.len(),
                    deadline_ms = ?context.deadline_ms,
                    query_field,
                    operation,
                    shipping_option_id = ?shipping_option_id,
                    requested_locale_length = requested_locale.map(str::len),
                    tenant_default_locale_length = tenant_default_locale.map(str::len),
                    error_kind,
                    owner_code = %error.code,
                    owner_kind = ?error.kind,
                    owner_retryable = error.retryable,
                    public_code,
                    public_retryable,
                    boundary = GRAPHQL_QUERY_FULFILLMENT_BOUNDARY,
                    "commerce GraphQL query shipping-option owner read was rejected"
                );
            }
        }

        #[allow(clippy::too_many_arguments)]
        fn log_fulfillment_query_error(
            error: &FulfillmentError,
            tenant_id: Uuid,
            query_field: &'static str,
            operation: &'static str,
            order_id: Option<Uuid>,
            requested_locale: Option<&str>,
            tenant_default_locale: Option<&str>,
        ) {
            let (owner_code, owner_kind, owner_retryable) = match error {
                FulfillmentError::Validation(_) =>
                    ("fulfillment.validation", "validation", false),
                FulfillmentError::ShippingOptionNotFound(_) => (
                    "fulfillment.shipping_option_not_found",
                    "not_found",
                    false,
                ),
                FulfillmentError::FulfillmentNotFound(_) => (
                    "fulfillment.fulfillment_not_found",
                    "not_found",
                    false,
                ),
                FulfillmentError::InvalidTransition { .. } => (
                    "fulfillment.invalid_transition",
                    "conflict",
                    false,
                ),
                FulfillmentError::Database(_) => (
                    "fulfillment.database_unavailable",
                    "unavailable",
                    true,
                ),
            };

            match error {
                FulfillmentError::Database(_) => tracing::error!(
                    error = ?error,
                    owner = "rustok_fulfillment",
                    tenant_id = %tenant_id,
                    query_field,
                    operation,
                    order_id = ?order_id,
                    requested_locale = ?requested_locale,
                    tenant_default_locale = ?tenant_default_locale,
                    owner_code,
                    owner_kind,
                    owner_retryable,
                    boundary = GRAPHQL_QUERY_FULFILLMENT_BOUNDARY,
                    "commerce GraphQL query fulfillment owner read failed"
                ),
                _ => tracing::warn!(
                    error = ?error,
                    owner = "rustok_fulfillment",
                    tenant_id = %tenant_id,
                    query_field,
                    operation,
                    order_id = ?order_id,
                    requested_locale = ?requested_locale,
                    tenant_default_locale = ?tenant_default_locale,
                    owner_code,
                    owner_kind,
                    owner_retryable,
                    boundary = GRAPHQL_QUERY_FULFILLMENT_BOUNDARY,
                    "commerce GraphQL query fulfillment owner read was rejected"
                ),
            }
        }
    }

    use self::rustok_api_shim as rustok_api;
    use self::rustok_fulfillment_shim as rustok_fulfillment;

    include!("query.rs");
}

pub use source::CommerceQuery;
