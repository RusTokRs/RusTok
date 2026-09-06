mod async_graphql_shim {
    pub use ::async_graphql::{Context, Object};

    pub type Error = super::super::query_error_boundary::BoundaryError;
    pub type FieldError = super::super::query_error_boundary::BoundaryError;
    pub type Result<T> = std::result::Result<T, super::super::query_error_boundary::BoundaryError>;
}

use self::async_graphql_shim as async_graphql;

mod rustok_api_shim {
    pub use ::rustok_api::{
        AuthContext, Permission, PortActor, PortContext, PortError, PortErrorKind, RequestContext,
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
                    <::async_graphql::FieldError as ::rustok_api::graphql::GraphQLError>::not_found(
                        message,
                    ),
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

#[path = "source/rustok_cart_shim.rs"]
mod rustok_cart_shim;
#[path = "source/rustok_channel_shim.rs"]
mod rustok_channel_shim;
#[path = "source/rustok_customer_shim.rs"]
mod rustok_customer_shim;
#[path = "source/rustok_fulfillment_shim.rs"]
mod rustok_fulfillment_shim;
#[path = "source/rustok_order_shim.rs"]
mod rustok_order_shim;
#[path = "source/rustok_payment_shim.rs"]
mod rustok_payment_shim;
#[path = "source/rustok_pricing_shim.rs"]
mod rustok_pricing_shim;

use self::rustok_api_shim as rustok_api;
use self::rustok_cart_shim as rustok_cart;
use self::rustok_channel_shim as rustok_channel;
use self::rustok_customer_shim as rustok_customer;
use self::rustok_fulfillment_shim as rustok_fulfillment;
use self::rustok_order_shim as rustok_order;
use self::rustok_payment_shim as rustok_payment;
use self::rustok_pricing_shim as rustok_pricing;

// The unchanged compatibility resolver formats the Region owner code and message
// before constructing a GraphQL error. Intercept only that exact source expression
// inside the safe-query include so the complete typed PortError reaches the
// transport mapper. Every other format invocation keeps standard Rust behavior.
macro_rules! format {
    ("{}: {}", $error:ident.code, $error_dup:ident.message) => {
        super::query_error_boundary::RegionGraphqlMessage::new($error)
    };
    ($($tokens:tt)*) => {
        ::std::format!($($tokens)*)
    };
}

include!("../query.rs");
